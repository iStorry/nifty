//! Choosing a downloaded file's name and creating it without replacing anything.

use std::borrow::Cow;
use std::fs::{File, OpenOptions};
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

/// Creates `name` in `folder`, or `name (1)`, `name (2)` and so on when it is taken, the way
/// browsers do, so a download never replaces an existing file.
pub(crate) fn create_unique(folder: &Path, name: &str) -> io::Result<(PathBuf, File)> {
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    };
    let mut copy = 0;
    loop {
        let candidate = if copy == 0 {
            folder.join(name)
        } else {
            folder.join(format!("{stem} ({copy}){extension}"))
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => copy += 1,
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

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

    #[test]
    fn never_replaces_an_existing_file() {
        let folder = std::env::temp_dir().join("nf-download-test-unique-names");
        if folder.exists() {
            fs::remove_dir_all(&folder).expect("remove stale scratch folder");
        }
        fs::create_dir_all(&folder).expect("create scratch folder");
        let names: Vec<_> = (0..3)
            .map(|_| {
                let (path, _) = create_unique(&folder, "notes.txt").expect("create a file");
                path.file_name()
                    .expect("a created file has a name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, ["notes.txt", "notes (1).txt", "notes (2).txt"]);
        fs::remove_dir_all(&folder).expect("remove scratch folder");
    }
}
