//! File dialogs: `NSOpenPanel` and `NSSavePanel`, as a sheet on the key window, or a
//! window of their own when none of the app's windows is key.

use std::cell::Cell;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{NSApplication, NSModalResponse, NSModalResponseOK, NSOpenPanel, NSSavePanel};
use objc2_foundation::{NSArray, NSString, NSURL};
use objc2_uniform_type_identifiers::UTType;

use nifty_runtime::{DialogHandler, DialogKind, FileDialog, Picked};

/// The panel a dialog shows; open panels are save panels with more settings.
enum Panel {
    Open(Retained<NSOpenPanel>),
    Save(Retained<NSSavePanel>),
}

impl Panel {
    fn new(mtm: MainThreadMarker, kind: DialogKind) -> Self {
        if kind == DialogKind::SavePath {
            return Self::Save(NSSavePanel::savePanel(mtm));
        }
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseFiles(kind != DialogKind::Folder);
        panel.setCanChooseDirectories(kind == DialogKind::Folder);
        panel.setAllowsMultipleSelection(kind.picks_many());
        Self::Open(panel)
    }

    fn save_panel(&self) -> &NSSavePanel {
        match self {
            Self::Open(panel) => panel,
            Self::Save(panel) => panel,
        }
    }

    /// The paths the user chose, once they pressed the panel's default button.
    fn chosen(&self) -> Result<Vec<String>, String> {
        let urls = match self {
            Self::Open(panel) => panel.URLs().to_vec(),
            Self::Save(panel) => panel.URL().into_iter().collect(),
        };
        if urls.is_empty() {
            return Err("macOS said a file was chosen but didn't say which".to_owned());
        }
        urls.iter().map(|url| path(url)).collect()
    }
}

/// Shows `dialog`, passing how it closed to `on_close` once the user closes it.
pub(crate) fn show(
    mtm: MainThreadMarker,
    app: &NSApplication,
    dialog: &FileDialog,
    on_close: DialogHandler,
) {
    let panel = Panel::new(mtm, dialog.kind);
    if let Err(reason) = configure(panel.save_panel(), dialog) {
        on_close(Picked::Failed(reason));
        return;
    }
    let pending = Cell::new(Some(on_close));
    let shown = panel.save_panel().retain();
    let handler = RcBlock::new(move |response: NSModalResponse| {
        let picked = if response == NSModalResponseOK {
            panel.chosen().map_or_else(Picked::Failed, Picked::Chosen)
        } else {
            Picked::Cancelled
        };
        if let Some(on_close) = pending.take() {
            on_close(picked);
        }
    });
    match app.keyWindow() {
        Some(window) => shown.beginSheetModalForWindow_completionHandler(&window, &handler),
        None => shown.beginWithCompletionHandler(&handler),
    }
}

fn configure(panel: &NSSavePanel, dialog: &FileDialog) -> Result<(), String> {
    panel.setCanCreateDirectories(true);
    if !dialog.title.is_empty() {
        // Panels show their message inside them; their title bar is hidden as a sheet.
        panel.setMessage(Some(&NSString::from_str(&dialog.title)));
    }
    if !dialog.types.is_empty() {
        let types = dialog
            .types
            .iter()
            .map(|extension| {
                UTType::typeWithFilenameExtension(&NSString::from_str(extension))
                    .ok_or_else(|| format!("macOS doesn't know the file type `{extension}`"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        panel.setAllowedContentTypes(&NSArray::from_retained_slice(&types));
    }
    if let Some(folder) = &dialog.folder {
        let folder = NSString::from_str(&folder.to_string_lossy());
        panel.setDirectoryURL(Some(&NSURL::fileURLWithPath_isDirectory(&folder, true)));
    }
    if !dialog.name.is_empty() {
        panel.setNameFieldStringValue(&NSString::from_str(&dialog.name));
    }
    Ok(())
}

fn path(url: &NSURL) -> Result<String, String> {
    url.path()
        .map(|path| path.to_string())
        .ok_or_else(|| "macOS chose a location that isn't a file path".to_owned())
}
