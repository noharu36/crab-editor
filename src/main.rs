extern crate clap;
extern crate termion;
extern crate unicode_width;

use clap::{App, Arg};
use editor::crab::Crab;
use std::io::{stdin, stdout};
use std::path;
use termion::event::{Event, Key};
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;

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
