#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// カーソルの位置 0-indexed
pub struct Cursor {
    pub row: usize,
    pub column: usize,
}

