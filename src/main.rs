extern crate clap;
extern crate termion;
extern crate unicode_width;

use clap::{App, Arg};
use std::cmp::{max, min};
use std::fs;
use std::io::{self, stdin, stdout, Write};
use std::path;
use termion::clear;
use termion::cursor;
use termion::event::{Event, Key};
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;
use unicode_width::UnicodeWidthChar;

use syntect::easy::HighlightLines;
use syntect::parsing::SyntaxSet;
use syntect::highlighting::{ThemeSet, Style};
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// カーソルの位置 0-indexed
struct Cursor {
    row: usize,
    column: usize,
}

// エディタの内部状態
struct Crab {
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
    fn open(&mut self, path: &path::Path) {
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

    fn terminal_size() -> (usize, usize) {
        let (cols, rows) = termion::terminal_size().unwrap();
        (rows as usize, cols as usize)
    }

    // 描画処理
    fn draw<T: Write>(&self, out: &mut T) -> Result<(), io::Error>  {
        // 画面サイズ(文字数)
        let (rows, cols) = Self::terminal_size();

        write!(out, "{}", clear::All)?;
        write!(out, "{}", cursor::Goto(1, 1))?;

        // 画面上の行、列
        let mut row = 0;
        let mut col = 0;


        let mut display_cursor: Option<(usize, usize)> = None;

        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let syntax = ps.find_syntax_by_extension("rs").unwrap();
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);

        'outer: for i in self.row_offset..self.buffer.len() {
            let mut s = String::new();
            for j in &self.buffer[i] {
                let ele = j.clone();
                s.push(ele)
            }
            println!("バッファ: {:?}, ストリング: {:?}", self.buffer[i], s);
            let mut escaped = String::new();
            for line in LinesWithEndings::from(&s) {
                let ranges: Vec<(Style, &str)> = h.highlight_line(line, &ps).unwrap();
                escaped = as_24_bit_terminal_escaped(&ranges[..], true);
            }
            let vec: Vec<char> = escaped.chars().collect();
            for j in 0..=vec.len() {
                if self.cursor == (Cursor { row: i, column: j }) {
                    // 画面上のカーソルの位置がわかった
                    display_cursor = Some((row, col));
                }

                if let Some(c) = vec.get(j) {
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
            write!(out, "{}", cursor::Goto(c as u16 + 1, r as u16 + 1))?;
        }

        out.flush()?;
        Ok(())
    }

    fn cursor_up(&mut self) {
        if self.cursor.row > 0 {
            self.cursor.row -= 1;
            self.cursor.column = min(self.buffer[self.cursor.row].len(), self.cursor.column);
        }
        self.scroll();
    }
    fn cursor_down(&mut self) {
        if self.cursor.row + 1 < self.buffer.len() {
            self.cursor.row += 1;
            self.cursor.column = min(self.cursor.column, self.buffer[self.cursor.row].len());
        }
        self.scroll();
    }
    fn cursor_left(&mut self) {
        if self.cursor.column >= 1 {
            self.cursor.column -= 1;
        }
    }
    fn cursor_right(&mut self) {
        self.cursor.column = min(self.cursor.column + 1, self.buffer[self.cursor.row].len());
    }

    fn scroll(&mut self) {
        let (rows, _) = Self::terminal_size();
        self.row_offset = min(self.row_offset, self.cursor.row);
        if self.cursor.row + 1 >= rows {
            self.row_offset = max(self.row_offset, self.cursor.row + 1 - rows);
        }
    }

    fn insert(&mut self, c: char) {
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

    fn back_space(&mut self) {
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

    fn save(&self) {
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


fn main() {
    // Clap
    let matches = App::new("Crab")
        .about("A text editor")
        .bin_name("Crab")
        .arg(Arg::with_name("file").required(true))
        .get_matches();

    let file_path: Option<&String> = matches.get_one::<String>("file");
    //matches.value_of_os("file");

    let mut state = Crab::default();

    if let Some(file_path) = file_path {
        state.open(path::Path::new(&file_path));
    }

    let stdin = stdin();
    let mut stdout = stdout().into_raw_mode().unwrap().into_alternate_screen().unwrap();

    state.draw(&mut stdout).unwrap();

    for evt in stdin.events() {
        match evt.unwrap() {
            Event::Key(Key::Ctrl('c')) => {
                return;
            }
            Event::Key(Key::Ctrl('s')) => {
                state.save();
            }
            Event::Key(Key::Up) => {
                state.cursor_up();
            }
            Event::Key(Key::Down) => {
                state.cursor_down();
            }
            Event::Key(Key::Left) => {
                state.cursor_left();
            }
            Event::Key(Key::Right) => {
                state.cursor_right();
            }
            Event::Key(Key::Char(c)) => {
                // 文字入力
                state.insert(c);
            }
            Event::Key(Key::Backspace) => {
                // バックスペースキー
                state.back_space();
            }
            _ => {}
        }
        state.draw(&mut stdout).unwrap();
    }
}
