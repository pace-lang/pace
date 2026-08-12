/// Represents a line and column in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Location {
    pub line: u32,
    pub column: u32,
}

impl Location {
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }
}

/// Represents a contiguous span of characters in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub file_id: u32,
    pub start: usize,
    pub end: usize,
    pub start_loc: Location,
    pub end_loc: Location,
}

impl Span {
    pub fn new(file_id: u32, start: usize, end: usize, start_loc: Location, end_loc: Location) -> Self {
        Self {
            file_id,
            start,
            end,
            start_loc,
            end_loc,
        }
    }
}
