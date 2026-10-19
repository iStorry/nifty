//! `nf create <name>`: start a new app from the starter template.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::ExitCode;

use crate::error::CliError;
use crate::{CommandResult, project};

const TEMPLATE: &str = include_str!("../templates/main.nf");
const TITLE_PLACEHOLDER: &str = "{{title}}";

pub(crate) fn run(name: &str) -> CommandResult {
    validate_name(name)?;
    let dir = Path::new(name);
    fs::create_dir(dir).map_err(|error| match error.kind() {
        ErrorKind::AlreadyExists => CliError::new(format!("{name} already exists")),
        _ => CliError::new(format!("can't create {name}: {error}")),
    })?;

    let entry = dir.join(project::ENTRY_FILE);
    fs::write(&entry, starter_source(name))
        .map_err(|error| CliError::new(format!("can't write {}: {error}", entry.display())))?;

    println!(
        "Created {name}/{}\n\nRun it with:\n  nf run {name}",
        project::ENTRY_FILE
    );
    Ok(ExitCode::SUCCESS)
}

/// Names become folder names and window titles, so they stay simple and portable.
fn validate_name(name: &str) -> Result<(), CliError> {
    let mut chars = name.chars();
    let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let rest_is_valid = chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if starts_with_letter && rest_is_valid {
        Ok(())
    } else {
        Err(CliError::new(format!(
            "`{name}` isn't a valid app name; start with a letter and use only letters, digits, `-` and `_`"
        )))
    }
}

fn starter_source(name: &str) -> String {
    TEMPLATE.replace(TITLE_PLACEHOLDER, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_simple_names() {
        for name in ["todo", "Todo2", "my-app", "my_app"] {
            assert!(validate_name(name).is_ok(), "{name} should be valid");
        }
    }

    #[test]
    fn rejects_names_that_are_not_portable_folder_names() {
        for name in ["", "2do", "-app", "my app", "a/b", "../x", "app.nf", "café"] {
            assert!(validate_name(name).is_err(), "{name:?} should be rejected");
        }
    }

    #[test]
    fn starter_source_is_a_valid_program_titled_after_the_app() {
        let source = starter_source("todo");
        assert!(source.contains(r#"window("todo","#));
        assert_eq!(nifty_syntax::parse(&source).diagnostics, []);
    }
}
