/// Point in the source text (line and column)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub row: usize,
    pub column: usize,
}

impl Point {
    pub fn new(row: usize, column: usize) -> Self {
        Point { row, column }
    }
}
