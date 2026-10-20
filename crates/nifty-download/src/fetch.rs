//! The worker side of a download: fetching over HTTP and writing the file, on its own thread.

use std::borrow::Cow;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use percent_encoding::percent_decode_str;
use ureq::http::Uri;
use ureq::tls::{RootCerts, TlsConfig, TlsProvider};
use ureq::{Agent, ResponseExt};

/// How long connecting, and then waiting for the server to start answering, may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

const CHUNK_SIZE: usize = 64 * 1024;

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

/// How far a download has got, sent to the UI thread.
#[derive(Debug, PartialEq, Eq)]
pub enum Report {
    /// The percentage received, when the server said how large the file is.
    Progress(i64),
    Saved(PathBuf),
    Failed(String),
}

/// The UI thread is gone, so nobody is waiting for this download any more.
#[derive(Debug)]
pub(crate) struct Abandoned;

#[derive(Debug)]
enum DownloadError {
    Http(ureq::Error),
    File(io::Error),
    NoHomeFolder,
    Abandoned,
    /// The download failed and its partly written file couldn't be removed either.
    LeftBehind {
        error: Box<DownloadError>,
        path: PathBuf,
    },
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(error) => write!(f, "{error}"),
            Self::File(error) => write!(f, "{error}"),
            Self::NoHomeFolder => f.write_str("couldn't find your home folder for `~`"),
            Self::Abandoned => f.write_str("the app stopped waiting for it"),
            Self::LeftBehind { error, path } => {
                write!(
                    f,
                    "{error}; the partial file {} is left behind",
                    path.display()
                )
            }
        }
    }
}

impl From<ureq::Error> for DownloadError {
    fn from(error: ureq::Error) -> Self {
        Self::Http(error)
    }
}

impl From<io::Error> for DownloadError {
    fn from(error: io::Error) -> Self {
        Self::File(error)
    }
}

impl From<Abandoned> for DownloadError {
    fn from(Abandoned: Abandoned) -> Self {
        Self::Abandoned
    }
}

/// Downloads `url` into `folder`, sending each step to `report`, and finally whether the
/// file was saved.
///
/// # Errors
/// [`Abandoned`] once `report` finds nobody listening, which stops the download early.
pub(crate) fn run(
    url: &str,
    folder: &str,
    report: &impl Fn(Report) -> Result<(), Abandoned>,
) -> Result<(), Abandoned> {
    let outcome = match fetch(url, folder, report) {
        Ok(path) => Report::Saved(path),
        Err(DownloadError::Abandoned) => return Err(Abandoned),
        Err(error) => Report::Failed(error.to_string()),
    };
    report(outcome)
}

fn fetch(
    url: &str,
    folder: &str,
    report: &impl Fn(Report) -> Result<(), Abandoned>,
) -> Result<PathBuf, DownloadError> {
    let folder = expand_home(folder)?;
    let response = agent().get(url).call()?;
    let name = file_name(response.get_uri());
    let total = response.body().content_length();
    let body = response.into_body().into_reader();

    fs::create_dir_all(&folder)?;
    let (path, file) = create_unique(&folder, &name)?;
    match save(body, file, total, report) {
        Ok(()) => Ok(path),
        Err(error) => match fs::remove_file(&path) {
            Ok(()) => Err(error),
            Err(_) => Err(DownloadError::LeftBehind {
                error: Box::new(error),
                path,
            }),
        },
    }
}

fn agent() -> Agent {
    let tls = TlsConfig::builder()
        .provider(TlsProvider::NativeTls)
        .root_certs(RootCerts::PlatformVerifier)
        .build();
    Agent::config_builder()
        .tls_config(tls)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_recv_response(Some(RESPONSE_TIMEOUT))
        .build()
        .into()
}

/// Copies `body` into `file`, reporting each new whole percentage when `total` is known.
fn save(
    mut body: impl Read,
    mut file: File,
    total: Option<u64>,
    report: &impl Fn(Report) -> Result<(), Abandoned>,
) -> Result<(), DownloadError> {
    let mut buffer = vec![0; CHUNK_SIZE];
    let mut received: u64 = 0;
    let mut reported = None;
    loop {
        let read = body.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])?;
        received += read as u64;
        if let Some(percent) = total.map(|total| percent(received, total))
            && reported != Some(percent)
        {
            report(Report::Progress(percent))?;
            reported = Some(percent);
        }
    }
    file.sync_all()?;
    Ok(())
}

fn percent(received: u64, total: u64) -> i64 {
    if total == 0 {
        return 100;
    }
    let percent = received.min(total) * 100 / total;
    i64::try_from(percent).expect("a percentage of at most 100 fits an i64")
}

/// `folder` with a leading `~` standing for the user's home folder.
fn expand_home(folder: &str) -> Result<PathBuf, DownloadError> {
    let Some(rest) = folder.strip_prefix('~') else {
        return Ok(PathBuf::from(folder));
    };
    if !rest.is_empty() && !rest.starts_with(std::path::is_separator) {
        return Ok(PathBuf::from(folder));
    }
    let home = std::env::home_dir().ok_or(DownloadError::NoHomeFolder)?;
    Ok(home.join(rest.trim_start_matches(std::path::is_separator)))
}

/// The last part of the address's path, decoded and made safe to use as a file name.
fn file_name(uri: &Uri) -> String {
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
fn create_unique(folder: &Path, name: &str) -> Result<(PathBuf, File), DownloadError> {
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
            Err(error) => return Err(error.into()),
        }
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

    #[test]
    fn counts_whole_percentages() {
        assert_eq!(percent(0, 200), 0);
        assert_eq!(percent(199, 200), 99);
        assert_eq!(percent(200, 200), 100);
        assert_eq!(percent(300, 200), 100);
        assert_eq!(percent(0, 0), 100);
    }

    #[test]
    fn a_leading_tilde_means_the_home_folder() {
        let home = std::env::home_dir().expect("tests run with a home folder");
        assert_eq!(expand_home("~").ok(), Some(home.clone()));
        assert_eq!(
            expand_home("~/Downloads").ok(),
            Some(home.join("Downloads"))
        );
        assert_eq!(expand_home("~other").ok(), Some(PathBuf::from("~other")));
        assert_eq!(expand_home("/tmp").ok(), Some(PathBuf::from("/tmp")));
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
