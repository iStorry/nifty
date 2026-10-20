//! File dialogs: the system's Common Item Dialog, modal to the app's active window. It runs
//! its own message loop until the user closes it, so posted wakes keep being handled.

use windows::Win32::Foundation::{ERROR_CANCELLED, ERROR_NO_UNICODE_TRANSLATION};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow;
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FILEOPENDIALOGOPTIONS, FOS_ALLOWMULTISELECT, FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS,
    FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItem,
    SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::{Error, HSTRING, PCWSTR, Result};

use nifty_runtime::{DialogKind, FileDialog, Picked};

/// COM and OLE, which the dialogs, images and drag and drop are built on, set up on the UI
/// thread for as long as the backend lives. OLE enters a single-threaded apartment.
#[derive(Debug)]
pub(crate) struct ComApartment(());

impl ComApartment {
    pub(crate) fn enter() -> Result<Self> {
        // SAFETY: the backend is created on the UI thread, which no other code has entered
        // into a multithreaded apartment.
        unsafe { OleInitialize(None) }?;
        Ok(Self(()))
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        // SAFETY: balances the successful `OleInitialize` in `enter`, on the same thread.
        unsafe { OleUninitialize() };
    }
}

/// Shows `dialog` and waits until the user closes it.
pub(crate) fn show(dialog: &FileDialog) -> Picked {
    match pick(dialog) {
        Ok(Some(paths)) => Picked::Chosen(paths),
        Ok(None) => Picked::Cancelled,
        Err(error) => Picked::Failed(error.to_string()),
    }
}

/// The paths the user chose, or `None` when they cancelled.
fn pick(dialog: &FileDialog) -> Result<Option<Vec<String>>> {
    let options = match dialog.kind {
        DialogKind::File | DialogKind::SavePath => FILEOPENDIALOGOPTIONS(0),
        DialogKind::Files => FOS_ALLOWMULTISELECT,
        DialogKind::Folder => FOS_PICKFOLDERS,
    };
    if dialog.kind == DialogKind::SavePath {
        // SAFETY: COM is initialized on this thread by the backend's `ComApartment`.
        let save: IFileSaveDialog =
            unsafe { CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)? };
        configure(&save, dialog, options)?;
        if !run(&save)? {
            return Ok(None);
        }
        // SAFETY: the dialog closed with a choice, so it has a result.
        let chosen = unsafe { save.GetResult()? };
        return Ok(Some(vec![path(&chosen)?]));
    }
    // SAFETY: COM is initialized on this thread by the backend's `ComApartment`.
    let open: IFileOpenDialog =
        unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)? };
    configure(&open, dialog, options)?;
    if !run(&open)? {
        return Ok(None);
    }
    // SAFETY: the dialog closed with a choice, so it has results.
    let results = unsafe { open.GetResults()? };
    // SAFETY: `results` is the array the dialog just returned.
    let count = unsafe { results.GetCount()? };
    (0..count)
        // SAFETY: every index below `count` names an item of `results`.
        .map(|index| path(&unsafe { results.GetItemAt(index)? }))
        .collect::<Result<_>>()
        .map(Some)
}

fn configure(
    native: &IFileDialog,
    dialog: &FileDialog,
    options: FILEOPENDIALOGOPTIONS,
) -> Result<()> {
    // SAFETY: each call configures a live dialog that isn't showing yet, with strings that
    // outlive the call.
    unsafe {
        native.SetOptions(native.GetOptions()? | FOS_FORCEFILESYSTEM | options)?;
        if !dialog.title.is_empty() {
            native.SetTitle(&HSTRING::from(dialog.title.as_str()))?;
        }
        if !dialog.types.is_empty() {
            let patterns: Vec<String> = dialog
                .types
                .iter()
                .map(|extension| format!("*.{extension}"))
                .collect();
            let name = HSTRING::from(patterns.join(", "));
            let spec = HSTRING::from(patterns.join(";"));
            native.SetFileTypes(&[COMDLG_FILTERSPEC {
                pszName: PCWSTR(name.as_ptr()),
                pszSpec: PCWSTR(spec.as_ptr()),
            }])?;
            // Saving appends the first type when the user types a name without one.
            native.SetDefaultExtension(&HSTRING::from(dialog.types[0].as_str()))?;
        }
        if let Some(folder) = &dialog.folder {
            let folder: IShellItem =
                SHCreateItemFromParsingName(&HSTRING::from(folder.as_path()), None)?;
            native.SetFolder(&folder)?;
        }
        if !dialog.name.is_empty() {
            native.SetFileName(&HSTRING::from(dialog.name.as_str()))?;
        }
    }
    Ok(())
}

/// Shows the dialog modal to the app's active window: `true` once the user chose, `false`
/// when they cancelled.
fn run(native: &IFileDialog) -> Result<bool> {
    // SAFETY: called on the UI thread, which owns the app's windows.
    let owner = unsafe { GetActiveWindow() };
    // SAFETY: the dialog is configured, and `owner` is this thread's window or null.
    match unsafe { native.Show(Some(owner)) } {
        Ok(()) => Ok(true),
        Err(error) if error.code() == ERROR_CANCELLED.to_hresult() => Ok(false),
        Err(error) => Err(error),
    }
}

/// The file system path of an item the dialog returned.
fn path(item: &IShellItem) -> Result<String> {
    // SAFETY: `FOS_FORCEFILESYSTEM` makes every item the dialog returns a file system item.
    let name = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH)? };
    // SAFETY: `name` is a NUL-terminated string the shell allocated for this call.
    let path = unsafe { name.to_string() };
    // SAFETY: the shell allocated `name` with the COM allocator and nothing else frees it.
    unsafe { CoTaskMemFree(Some(name.0.cast_const().cast())) };
    path.map_err(|_| {
        Error::new(
            ERROR_NO_UNICODE_TRANSLATION.to_hresult(),
            "the chosen path isn't valid Unicode",
        )
    })
}
