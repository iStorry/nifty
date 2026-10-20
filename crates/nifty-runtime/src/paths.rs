//! Paths as apps write them, shared by downloads and file dialogs.

use std::fmt;
use std::path::PathBuf;

/// `path` with a leading `~` standing for the user's home folder.
///
/// # Errors
/// When `path` starts with `~` and the home folder can't be found.
pub fn expand_home(path: &str) -> Result<PathBuf, NoHomeFolder> {
    let Some(rest) = path.strip_prefix('~') else {
        return Ok(PathBuf::from(path));
    };
    if !rest.is_empty() && !rest.starts_with(std::path::is_separator) {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::home_dir().ok_or(NoHomeFolder)?;
    Ok(home.join(rest.trim_start_matches(std::path::is_separator)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoHomeFolder;

impl fmt::Display for NoHomeFolder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("couldn't find your home folder for `~`")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
