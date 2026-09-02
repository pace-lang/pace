use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn merge(&self, other: &Span) -> Span {
        if self.start == 0 && self.end == 0 {
            return *other;
        }
        if other.start == 0 && other.end == 0 {
            return *self;
        }
        let start = self.start.min(other.start);
        let end = self.end.max(other.end);
        Span::new(start, end)
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}..{})", self.start, self.end)
    }
}

impl From<Span> for (usize, usize) {
    fn from(span: Span) -> Self {
        (span.start, span.len())
    }
}

impl From<(usize, usize)> for Span {
    fn from(tuple: (usize, usize)) -> Self {
        Span::new(tuple.0, tuple.0 + tuple.1)
    }
}

impl From<Span> for miette::SourceSpan {
    fn from(span: Span) -> Self {
        miette::SourceSpan::new(span.start.into(), span.len())
    }
}
