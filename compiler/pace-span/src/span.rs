use miette::SourceSpan;
use serde::{Deserialize, Serialize};

/// A unique identifier for a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileId(pub u32);

impl FileId {
    pub const DUMMY: FileId = FileId(0);
}

#[derive(Debug, Default)]
pub struct SourceMap {
    files: Vec<(String, String)>,
}

impl SourceMap {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn add_file(&mut self, path: String, source: String) -> FileId {
        let id = self.files.len() as u32;
        self.files.push((path, source));
        FileId(id)
    }

    pub fn get_file(&self, id: FileId) -> Option<&(String, String)> {
        self.files.get(id.0 as usize)
    }

    pub fn get_source(&self, id: FileId) -> Option<&str> {
        self.files.get(id.0 as usize).map(|(_, s)| s.as_str())
    }

    pub fn get_path(&self, id: FileId) -> Option<&str> {
        self.files.get(id.0 as usize).map(|(p, _)| p.as_str())
    }

    pub fn get_file_id(&self, path: &str) -> Option<FileId> {
        self.files
            .iter()
            .position(|(p, _)| p == path)
            .map(|i| FileId(i as u32))
    }
}

/// A range of bytes in the source code, tagged with the file it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub file_id: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const DUMMY: Span = Span {
        file_id: FileId::DUMMY,
        start: 0,
        end: 0,
    };

    pub fn new(file_id: FileId, start: u32, end: u32) -> Self {
        Self {
            file_id,
            start,
            end,
        }
    }

    /// Merges two spans into a single span that encompasses both.
    /// Panics if the spans belong to different files.
    pub fn merge(self, other: Self) -> Self {
        assert_eq!(
            self.file_id, other.file_id,
            "Cannot merge spans from different files"
        );
        Self {
            file_id: self.file_id,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl From<Span> for SourceSpan {
    fn from(span: Span) -> Self {
        SourceSpan::new(
            (span.start as usize).into(),
            (span.end - span.start) as usize,
        )
    }
}

impl From<(FileId, std::ops::Range<usize>)> for Span {
    fn from((file_id, range): (FileId, std::ops::Range<usize>)) -> Self {
        Span::new(file_id, range.start as u32, range.end as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_map() {
        let mut sm = SourceMap::new();
        let file1 = sm.add_file("file1.pace".to_string(), "let x = 5;".to_string());
        let file2 = sm.add_file("file2.pace".to_string(), "let y = 10;".to_string());

        assert_eq!(file1, FileId(0));
        assert_eq!(file2, FileId(1));

        assert_eq!(sm.get_source(file1), Some("let x = 5;"));
        assert_eq!(sm.get_path(file2), Some("file2.pace"));
        assert_eq!(sm.get_file_id("file1.pace"), Some(file1));
        assert_eq!(sm.get_file_id("missing.pace"), None);
    }

    #[test]
    fn test_span_merge() {
        let span1 = Span::new(FileId(1), 5, 10);
        let span2 = Span::new(FileId(1), 15, 20);
        let merged = span1.merge(span2);

        assert_eq!(merged.file_id, FileId(1));
        assert_eq!(merged.start, 5);
        assert_eq!(merged.end, 20);
    }

    #[test]
    #[should_panic(expected = "Cannot merge spans from different files")]
    fn test_span_merge_panic() {
        let span1 = Span::new(FileId(1), 5, 10);
        let span2 = Span::new(FileId(2), 15, 20);
        span1.merge(span2);
    }
}
