use std::ops::Range;

/// One source file of a program. Files are numbered in the order they are checked, and
/// the first, `FileId::default()`, is the project's entry file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(usize);

impl FileId {
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A byte range in the text of one source file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub file: FileId,
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(file: FileId, start: usize, end: usize) -> Self {
        Self { file, start, end }
    }

    /// The span covering `self` through `other`, which is in the same file.
    #[must_use]
    pub const fn to(self, other: Self) -> Self {
        Self::new(self.file, self.start, other.end)
    }

    #[must_use]
    pub const fn range(self) -> Range<usize> {
        self.start..self.end
    }
}
