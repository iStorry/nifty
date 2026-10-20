//! Choosing a downloaded file's name and creating it without replacing anything.

use std::borrow::Cow;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;
use ureq::http::Uri;

/// The name a file gets when its address doesn't end in one, such as `https://example.com/`.
const FALLBACK_FILE_NAME: &str = "download";

/// Characters some file systems refuse in names, replaced by [`NAME_REPLACEMENT`].
const UNSAFE_NAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];
const NAME_REPLACEMENT: char = '_';
/// Names Windows keeps for devices, with or without an extension, such as `nul.txt`.
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// The last part of the address's path, decoded and made safe to use as a file name.
pub(crate) fn file_name(uri: &Uri) -> String {
    let last = uri.path().rsplit('/').next().unwrap_or_default();
    let decoded = percent_decode_str(last)
        .decode_utf8()
        .unwrap_or(Cow::Borrowed(last));
    let safe: String = decoded
        .chars()
        .map(|c| {
            if UNSAFE_NAME_CHARS.contains(&c) || c.is_control() {
                NAME_REPLACEMENT
            } else {
                c
            }
        })
        .collect();
    // Windows drops trailing dots and spaces, so `a.` and `a` would be the same file.
    let safe = safe.trim_end_matches(['.', ' ']);
    if safe.is_empty() {
        return FALLBACK_FILE_NAME.to_owned();
    }
    let stem = safe.split('.').next().unwrap_or_default();
    if RESERVED_NAMES
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(stem))
    {
        format!("{NAME_REPLACEMENT}{safe}")
    } else {
        safe.to_owned()
    }
}

/// Added to a file's name while it downloads, so a file left behind, as when the app
/// quits mid-download on macOS, reads as unfinished.
const PARTIAL_SUFFIX: &str = ".part";

/// A file being downloaded: `name.part` in `folder`, which becomes `name`, or `name (1)`
/// and so on, once it is saved.
#[derive(Debug)]
pub(crate) struct Partial {
    folder: PathBuf,
    name: String,
    /// The copy number its name was free at, which finishing tries first.
    copy: usize,
    path: PathBuf,
}

impl Partial {
    /// Creates the partial file for `name` in `folder`, at the first copy number whose
    /// finished name and partial name are both free, the way browsers number copies, so a
    /// download never replaces an existing file.
    pub(crate) fn create(folder: &Path, name: &str) -> io::Result<(Self, File)> {
        let mut copy = 0;
        loop {
            let finished = folder.join(numbered(name, copy));
            let path = partial_path(&finished);
            if !finished.exists()
                && let Some(file) = create_new(&path)?
            {
                let partial = Self {
                    folder: folder.to_owned(),
                    name: name.to_owned(),
                    copy,
                    path,
                };
                return Ok((partial, file));
            }
            copy += 1;
        }
    }

    /// Where it is being written.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Gives the file its finished name, the first one free from its own copy number on,
    /// since another file may have taken that name while it downloaded.
    pub(crate) fn finish(&self) -> io::Result<PathBuf> {
        let mut copy = self.copy;
        loop {
            let finished = self.folder.join(numbered(&self.name, copy));
            // Claiming the name first means renaming over it replaces only this placeholder.
            if create_new(&finished)?.is_some() {
                fs::rename(&self.path, &finished)?;
                return Ok(finished);
            }
            copy += 1;
        }
    }
}

/// `name`, or for copies after the first, `name (copy)` before its extension.
fn numbered(name: &str, copy: usize) -> String {
    if copy == 0 {
        return name.to_owned();
    }
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    };
    format!("{stem} ({copy}){extension}")
}

fn partial_path(finished: &Path) -> PathBuf {
    let mut path = finished.as_os_str().to_owned();
    path.push(PARTIAL_SUFFIX);
    PathBuf::from(path)
}

/// Creates `path` for writing; `None` when something is there already.
fn create_new(path: &Path) -> io::Result<Option<File>> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => Ok(Some(file)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_of(address: &str) -> String {
        file_name(&address.parse().expect("a valid test address"))
    }

    #[test]
    fn names_the_file_after_the_end_of_the_address() {
        assert_eq!(
            name_of("https://example.com/files/report.pdf"),
            "report.pdf"
        );
        assert_eq!(name_of("https://example.com/a.zip?version=2"), "a.zip");
        assert_eq!(name_of("https://example.com/"), FALLBACK_FILE_NAME);
        assert_eq!(name_of("https://example.com"), FALLBACK_FILE_NAME);
        assert_eq!(name_of("https://example.com/.."), FALLBACK_FILE_NAME);
        assert_eq!(name_of("https://example.com/a:b"), "a_b");
        assert_eq!(name_of("https://example.com/my%20file.pdf"), "my file.pdf");
        assert_eq!(name_of("https://example.com/caf%C3%A9.txt"), "café.txt");
        assert_eq!(name_of("https://example.com/bad%FF.txt"), "bad%FF.txt");
        assert_eq!(name_of("https://example.com/100%25.txt"), "100%.txt");
    }

    #[test]
    fn decoded_separators_never_leave_the_folder() {
        assert_eq!(name_of("https://example.com/..%2F..%2Fetc"), ".._.._etc");
        assert_eq!(name_of("https://example.com/a%5Cb.txt"), "a_b.txt");
        assert_eq!(name_of("https://example.com/%2E%2E"), FALLBACK_FILE_NAME);
        assert_eq!(name_of("https://example.com/a%00b"), "a_b");
    }

    #[test]
    fn avoids_names_windows_reserves_or_changes() {
        assert_eq!(name_of("https://example.com/nul"), "_nul");
        assert_eq!(name_of("https://example.com/CON.txt"), "_CON.txt");
        assert_eq!(name_of("https://example.com/com1.tar.gz"), "_com1.tar.gz");
        assert_eq!(name_of("https://example.com/console.txt"), "console.txt");
        assert_eq!(name_of("https://example.com/notes.."), "notes");
        assert_eq!(name_of("https://example.com/n%75l.txt"), "_nul.txt");
        assert_eq!(name_of("https://example.com/notes%2E%20"), "notes");
    }

    fn file_names(folder: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(folder)
            .expect("read the scratch folder")
            .map(|entry| {
                entry
                    .expect("read a folder entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    fn scratch(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!("nf-download-test-{name}"));
        if folder.exists() {
            fs::remove_dir_all(&folder).expect("remove stale scratch folder");
        }
        fs::create_dir_all(&folder).expect("create scratch folder");
        folder
    }

    #[test]
    fn never_replaces_an_existing_file() {
        let folder = scratch("unique-names");
        fs::write(folder.join("notes.txt"), "mine").expect("write a file");
        let (first, _) = Partial::create(&folder, "notes.txt").expect("create a partial file");
        let (second, _) = Partial::create(&folder, "notes.txt").expect("create a partial file");
        assert_eq!(
            file_names(&folder),
            ["notes (1).txt.part", "notes (2).txt.part", "notes.txt"]
        );

        fs::write(folder.join("notes (2).txt"), "taken meanwhile").expect("write a file");
        assert_eq!(first.finish().ok(), Some(folder.join("notes (1).txt")));
        assert_eq!(second.finish().ok(), Some(folder.join("notes (3).txt")));
        assert_eq!(
            file_names(&folder),
            [
                "notes (1).txt",
                "notes (2).txt",
                "notes (3).txt",
                "notes.txt"
            ]
        );
        assert_eq!(
            fs::read_to_string(folder.join("notes (2).txt"))
                .ok()
                .as_deref(),
            Some("taken meanwhile")
        );
        fs::remove_dir_all(&folder).expect("remove scratch folder");
    }
}
