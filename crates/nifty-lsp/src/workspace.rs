//! The open documents and the projects they belong to. A document inside a project folder is
//! analysed with every other `.nf` file of that project, open or not; a document outside one
//! is a project of its own. Open documents' text wins over what is saved on disk.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use lsp_types::{PublishDiagnosticsParams, Uri};
use nifty_syntax::FileId;
use nifty_syntax::project::{self, root_of};
use url::Url;

use crate::document::Encoding;
use crate::features::diagnostics;
use crate::project::{Cursor, Project, Source};

#[derive(Debug)]
struct OpenDocument {
    text: String,
    version: i32,
    /// Where the document is saved; `None` for one that isn't a file, such as an untitled
    /// buffer.
    path: Option<PathBuf>,
}

/// What changed after a document was opened, edited or closed.
#[derive(Debug, Default)]
pub(crate) struct Refreshed {
    /// The diagnostics of every file whose project was analysed again, and an empty list for
    /// every file no longer in an analysed project.
    pub diagnostics: Vec<PublishDiagnosticsParams>,
    /// Files of the project that couldn't be read, which are left out of the analysis.
    pub warnings: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Workspace {
    open: HashMap<Uri, OpenDocument>,
    projects: Vec<Project>,
}

impl Workspace {
    pub(crate) fn update(
        &mut self,
        uri: &Uri,
        text: String,
        version: i32,
        encoding: Encoding,
    ) -> Refreshed {
        let path = path_of(uri);
        self.open.insert(
            uri.clone(),
            OpenDocument {
                text,
                version,
                path,
            },
        );
        self.refresh(uri, encoding)
    }

    pub(crate) fn close(&mut self, uri: &Uri, encoding: Encoding) -> Refreshed {
        self.open.remove(uri);
        self.refresh(uri, encoding)
    }

    /// The project file a request points into, and the byte offset it points at.
    pub(crate) fn locate(
        &self,
        uri: &Uri,
        position: lsp_types::Position,
        encoding: Encoding,
    ) -> Option<Cursor<'_>> {
        let (project, file) = self.file(uri)?;
        let offset = project.document(file).offset(position, encoding);
        Some(Cursor {
            project,
            file,
            offset,
        })
    }

    /// The project holding `uri`, and which of its files it is.
    pub(crate) fn file(&self, uri: &Uri) -> Option<(&Project, FileId)> {
        self.projects
            .iter()
            .find_map(|project| Some((project, project.file_of(uri)?)))
    }

    /// Analyses again the project `uri` belongs to. When `uri` was just closed, the project
    /// stays only while another of its files is open.
    fn refresh(&mut self, uri: &Uri, encoding: Encoding) -> Refreshed {
        let old = self
            .projects
            .iter()
            .position(|project| project.file_of(uri).is_some())
            .map(|index| self.projects.swap_remove(index));
        let still_open = if self.open.contains_key(uri) {
            Some(uri.clone())
        } else {
            old.as_ref().and_then(|project| {
                project
                    .files()
                    .map(|(_, uri)| uri)
                    .find(|uri| self.open.contains_key(uri))
                    .cloned()
            })
        };

        let mut refreshed = Refreshed::default();
        let new = still_open.map(|uri| Project::new(self.sources(&uri, &mut refreshed.warnings)));
        if let Some(old) = &old {
            for (_, uri) in old.files() {
                if new.as_ref().is_none_or(|new| new.file_of(uri).is_none()) {
                    refreshed.diagnostics.push(self.publish(uri, Vec::new()));
                }
            }
        }
        if let Some(new) = new {
            for (file, uri) in new.files() {
                let found = diagnostics::diagnostics(&new, file, encoding);
                refreshed.diagnostics.push(self.publish(uri, found));
            }
            self.projects.push(new);
        }
        refreshed
    }

    /// The files of the project the open document `uri` belongs to: the entry file first,
    /// with the text of open documents and of the rest as saved.
    fn sources(&self, uri: &Uri, warnings: &mut Vec<String>) -> Vec<Source> {
        let alone = |document: &OpenDocument| {
            vec![Source {
                uri: uri.clone(),
                text: document.text.clone(),
            }]
        };
        let Some(document) = self.open.get(uri) else {
            return Vec::new();
        };
        let Some(root) = document.path.as_deref().and_then(root_of) else {
            return alone(document);
        };
        let listed = match project::source_files(root) {
            Ok(listed) => listed,
            Err(error) => {
                warnings.push(format!(
                    "can't list the files of {}: {error}",
                    root.display()
                ));
                return alone(document);
            }
        };
        let mut sources: Vec<Source> = listed
            .iter()
            .filter_map(|relative| self.source(&root.join(relative), warnings))
            .collect();
        if !sources.iter().any(|source| source.uri == *uri) {
            sources.extend(alone(document));
        }
        sources
    }

    /// The source saved at `path`, or its open document's text when it is open.
    fn source(&self, path: &Path, warnings: &mut Vec<String>) -> Option<Source> {
        let open = self
            .open
            .iter()
            .find(|(_, document)| document.path.as_deref() == Some(path));
        if let Some((uri, document)) = open {
            return Some(Source {
                uri: uri.clone(),
                text: document.text.clone(),
            });
        }
        let Some(uri) = uri_of(path) else {
            warnings.push(format!("can't address {} as a URI", path.display()));
            return None;
        };
        match fs::read_to_string(path) {
            Ok(text) => Some(Source { uri, text }),
            Err(error) => {
                warnings.push(format!("can't read {}: {error}", path.display()));
                None
            }
        }
    }

    fn publish(
        &self,
        uri: &Uri,
        diagnostics: Vec<lsp_types::Diagnostic>,
    ) -> PublishDiagnosticsParams {
        PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: self.open.get(uri).map(|document| document.version),
        }
    }
}

/// The file a `file:` URI names; other schemes have none.
fn path_of(uri: &Uri) -> Option<PathBuf> {
    Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

fn uri_of(path: &Path) -> Option<Uri> {
    Url::from_file_path(path).ok()?.as_str().parse().ok()
}
