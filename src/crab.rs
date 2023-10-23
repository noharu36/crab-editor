mod cursor;

use std::io;
use std::io::Write;
use std::{path, fs};
use std::cmp::{min, max};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use termion::{clear, cursor as termion_cursor};
use unicode_width::UnicodeWidthChar;

use self::cursor::Cursor;

// エディタの内部状態
pub struct Crab {
    // テキスト本体
    buffer: Vec<Vec<char>>,
    // 現在のカーソルの位置
    cursor: Cursor,
    row_offset: usize,
    path: Option<path::PathBuf>
}

impl Default for Crab {
    fn default() -> Self {
        Self {
            buffer: vec![Vec::new()],
            cursor: Cursor { row: 0, column: 0 },
            row_offset: 0,
            path: None
        }
    }
}

impl Crab {
    // ファイルを読み込む
    pub fn open(&mut self, path: &path::Path) {
        self.buffer = fs::read_to_string(path)
            .ok()
            .map(|s| {
                let buffer: Vec<Vec<char>> = s
                    .lines()
                    .map(|line| line.trim_end().chars().collect())
                    .collect();
                if buffer.is_empty() {
                    vec![Vec::new()]
                } else {
                    buffer
                }
            })
            .unwrap_or_else(|| vec![Vec::new()]);

        self.path = Some(path.into());
        self.cursor = Cursor { row: 0, column: 0 };
        self.row_offset = 0;
    }

    pub fn terminal_size() -> (usize, usize) {
        let (cols, rows) = termion::terminal_size().unwrap();
        (rows as usize, cols as usize)
    }

    // 描画処理
    pub fn draw<T: Write>(&self, out: &mut T) -> Result<(), io::Error>  {
        // 画面サイズ(文字数)
        let (rows, cols) = Self::terminal_size();

        write!(out, "{}", clear::All)?;
        write!(out, "{}", termion_cursor::Goto(1, 1))?;

        // 画面上の行、列
        let mut row = 0;
        let mut col = 0;


        let mut display_cursor: Option<(usize, usize)> = None;

        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let syntax = ps.find_syntax_by_extension("rs").unwrap();
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);

        'outer: for i in self.row_offset..self.buffer.len() {
            for j in 0..=self.buffer[i].len() {
                if self.cursor == (Cursor { row: i, column: j }) {
                    // 画面上のカーソルの位置がわかった
                    display_cursor = Some((row, col));
                }

                if let Some(c) = self.buffer[i].get(j) {
                    // 文字の幅を取得する
                    let width = c.width().unwrap_or(0);
                    if col + width >= cols {
                        row += 1;
                        col = 0;
                        if row >= rows {
                            break 'outer;
                        } else {
                            write!(out, "\r\n")?;
                        }
                    }
                    write!(out, "{}", c)?;
                    col += width;
                }
            }
            row += 1;
            col = 0;
            if row >= rows {
                break;
            } else {
                // 最後の行の最後では改行すると1行ずれてしまうのでこのようなコードになっている
                write!(out, "\r\n")?;
            }
        }

        if let Some((r, c)) = display_cursor {
            write!(out, "{}", termion_cursor::Goto(c as u16 + 1, r as u16 + 1))?;
        }

        out.flush()?;
        Ok(())
    }

    pub fn cursor_up(&mut self) {
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.column = min(self.buffer[self.cursor.row].len(), self.cursor.column);
        }
        self.scroll();
    }
    pub fn cursor_down(&mut self) {
        if self.cursor.row + 1 < self.buffer.len() {
            self.cursor.row += 1;
            self.cursor.column = min(self.cursor.column, self.buffer[self.cursor.row].len());
        }
        self.scroll();
    }
    pub fn cursor_left(&mut self) {
        if self.cursor.column >= 1 {
            self.cursor.column -= 1;
        }
    }
    pub fn cursor_right(&mut self) {
        self.cursor.column = min(self.cursor.column + 1, self.buffer[self.cursor.row].len());
    }

    pub fn scroll(&mut self) {
        let (rows, _) = Self::terminal_size();
        self.row_offset = min(self.row_offset, self.cursor.row);
        if self.cursor.row + 1 >= rows {
            self.row_offset = max(self.row_offset, self.cursor.row + 1 - rows);
        }
    }

    pub fn insert(&mut self, c: char) {
        if c == '\n' {
            // 改行
            let rest: Vec<char> = self.buffer[self.cursor.row]
                .drain(self.cursor.column..)
                .collect();
            self.buffer.insert(self.cursor.row + 1, rest);
            self.cursor.row += 1;
            self.cursor.column = 0;
            self.scroll();
        } else if !c.is_control() {
            self.buffer[self.cursor.row].insert(self.cursor.column, c);
            self.cursor_right();
        }
    }

    pub fn back_space(&mut self) {
        if self.cursor == (Cursor { row: 0, column: 0 }) {
            // 一番始めの位置の場合何もしない
            return;
        }

        if self.cursor.column == 0 {
            // 行の先頭
            let line = self.buffer.remove(self.cursor.row);
            self.cursor.row -= 1;
            self.cursor.column = self.buffer[self.cursor.row].len();
            self.buffer[self.cursor.row].extend(line.into_iter());
        } else {
            self.cursor_left();
            self.buffer[self.cursor.row].remove(self.cursor.column);
        }
    }

    pub fn save(&self) {
        if let Some(path) = self.path.as_ref() {
            if let Ok(mut file) = fs::File::create(path) {
                for line in &self.buffer {
                    for &c in line {
                        write!(file, "{}", c).unwrap();
                    }
                    writeln!(file).unwrap();
                }
            }
        }
    }
}