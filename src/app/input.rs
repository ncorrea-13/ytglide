// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{App, Focus};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match self.focus {
            Focus::Search => self.handle_search_key(key),
            Focus::List => self.handle_list_key(key),
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => self.submit_search(),
            KeyCode::Esc | KeyCode::Down | KeyCode::Tab => {
                if self.results.is_empty() {
                    if key.code == KeyCode::Esc {
                        self.should_quit = true;
                    }
                } else {
                    self.focus = Focus::List;
                }
            }
            KeyCode::Left => {
                if let Some(c) = self.query[..self.cursor].chars().next_back() {
                    self.cursor -= c.len_utf8();
                }
            }
            KeyCode::Right => {
                if let Some(c) = self.query[self.cursor..].chars().next() {
                    self.cursor += c.len_utf8();
                }
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.query.len(),
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.query.len(),
            KeyCode::Backspace => {
                if let Some(c) = self.query[..self.cursor].chars().next_back() {
                    self.cursor -= c.len_utf8();
                    self.query.remove(self.cursor);
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.query.len() {
                    self.query.remove(self.cursor);
                }
            }
            KeyCode::Char('w') if ctrl => {
                let head = self.query[..self.cursor].trim_end();
                let start = head.rfind(' ').map_or(0, |i| i + 1);
                self.query.drain(start..self.cursor);
                self.cursor = start;
            }
            KeyCode::Char('u') if ctrl => {
                self.query.clear();
                self.cursor = 0;
            }
            KeyCode::Char(c) if !ctrl => {
                self.query.insert(self.cursor, c);
                self.cursor += c.len_utf8();
            }
            _ => {}
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('g') | KeyCode::Home => self.select_edge(false),
            KeyCode::Char('G') | KeyCode::End => self.select_edge(true),
            KeyCode::Char('/') | KeyCode::Char('i') | KeyCode::Tab => self.focus = Focus::Search,
            KeyCode::Enter => self.play_selected(false),
            KeyCode::Char('a') => self.play_selected(true),
            KeyCode::Char(' ') => self.toggle_pause(),
            KeyCode::Char('s') => {
                if self.stop_player() {
                    self.set_status("stopped");
                }
            }
            KeyCode::Char('p') => self.show_preview = !self.show_preview,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_image::picker::Picker;

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn typing_edits_query_and_esc_quits_without_results() {
        let mut app = App::new(Picker::halfblocks());
        press(&mut app, KeyCode::Char('l'));
        press(&mut app, KeyCode::Char('o'));
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.query, "l");
        press(&mut app, KeyCode::Esc);
        assert!(app.should_quit);
    }

    #[test]
    fn cursor_edits_in_place_with_multibyte() {
        let mut app = App::new(Picker::halfblocks());
        for c in "añb".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Left);
        press(&mut app, KeyCode::Backspace);
        assert_eq!(app.query, "ab");
        press(&mut app, KeyCode::Char('x'));
        assert_eq!(app.query, "axb");
        press(&mut app, KeyCode::Home);
        press(&mut app, KeyCode::Delete);
        assert_eq!(app.query, "xb");
    }

    #[test]
    fn ctrl_w_deletes_previous_word() {
        let mut app = App::new(Picker::halfblocks());
        for c in "lofi hip hop".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(app.query, "lofi hip ");
        app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(app.query, "lofi ");
    }

    #[test]
    fn list_navigation_is_clamped() {
        let mut app = App::new(Picker::halfblocks());
        app.focus = Focus::List;
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.list.selected(), None);
    }
}
