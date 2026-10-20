//! The files of one program, analysed together: a project folder's `.nf` files, or a single
//! file outside any project. A name declared in one file resolves in all of them.

use lsp_types::{Location, Uri};
use nifty_syntax::ast::{File, Item};
use nifty_syntax::{Diagnostic, FileId, Parsed, Span};

use crate::document::{Document, Encoding};
use crate::index::Index;

/// A source file's text and the address the editor knows it by.
#[derive(Debug)]
pub(crate) struct Source {
    pub uri: Uri,
    pub text: String,
}

#[derive(Debug)]
pub(crate) struct Project {
    /// File `i` is `uris[i]` and `documents[i]`.
    uris: Vec<Uri>,
    documents: Vec<Document>,
    pub index: Index,
    /// Syntax errors from every file, or type errors once they all parse.
    diagnostics: Vec<Diagnostic>,
}

impl Project {
    /// Analyses `sources` as one program; the first is the entry file.
    pub(crate) fn new(sources: Vec<Source>) -> Self {
        let (uris, documents): (Vec<Uri>, Vec<Document>) = sources
            .into_iter()
            .enumerate()
            .map(|(index, source)| (source.uri, Document::new(source.text, FileId::new(index))))
            .unzip();
        let parsed: Vec<&Parsed> = documents.iter().map(|document| &document.parsed).collect();
        let files: Vec<&File> = parsed.iter().map(|parsed| &parsed.file).collect();
        let index = Index::new(&files);
        let diagnostics = nifty_check::compile(&parsed).err().unwrap_or_default();
        Self {
            uris,
            documents,
            index,
            diagnostics,
        }
    }

    pub(crate) fn file_of(&self, uri: &Uri) -> Option<FileId> {
        self.uris
            .iter()
            .position(|known| known == uri)
            .map(FileId::new)
    }

    /// Every file with its address.
    pub(crate) fn files(&self) -> impl Iterator<Item = (FileId, &Uri)> {
        self.uris
            .iter()
            .enumerate()
            .map(|(index, uri)| (FileId::new(index), uri))
    }

    pub(crate) fn document(&self, file: FileId) -> &Document {
        &self.documents[file.index()]
    }

    /// The text of `span`, in whichever file it points into.
    pub(crate) fn text(&self, span: Span) -> &str {
        self.document(span.file).text(span)
    }

    pub(crate) fn location(&self, span: Span, encoding: Encoding) -> Location {
        Location::new(
            self.uris[span.file.index()].clone(),
            self.document(span.file).range(span, encoding),
        )
    }

    /// The errors whose main place is in `file`.
    pub(crate) fn diagnostics_in(&self, file: FileId) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(move |diagnostic| diagnostic.span.file == file)
    }

    /// The top-level items of every file.
    pub(crate) fn items(&self) -> impl Iterator<Item = &Item> {
        self.documents
            .iter()
            .flat_map(|document| &document.parsed.file.items)
    }
}

/// The place in a project a request points at: an offset in one of its files.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cursor<'p> {
    pub project: &'p Project,
    pub file: FileId,
    pub offset: usize,
}

impl<'p> Cursor<'p> {
    pub(crate) fn document(self) -> &'p Document {
        self.project.document(self.file)
    }
}

#[cfg(test)]
impl Project {
    /// A project of one file, for testing a feature on `text`.
    pub(crate) fn single(text: &str) -> Self {
        Self::new(vec![Source {
            uri: test_uri("main.nf"),
            text: text.to_owned(),
        }])
    }

    /// The cursor where `|` stands in `source`, in a project of that one file.
    pub(crate) fn with_cursor(source: &str) -> (Self, usize) {
        let offset = source
            .find('|')
            .expect("the source marks the cursor with `|`");
        (Self::single(&source.replacen('|', "", 1)), offset)
    }

    pub(crate) fn cursor(&self, offset: usize) -> Cursor<'_> {
        Cursor {
            project: self,
            file: FileId::default(),
            offset,
        }
    }
}

#[cfg(test)]
pub(crate) fn test_uri(name: &str) -> Uri {
    format!("file:///project/{name}")
        .parse()
        .expect("a valid test uri")
}
