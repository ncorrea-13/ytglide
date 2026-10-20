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
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char('u') if ctrl => self.query.clear(),
            KeyCode::Char(c) if !ctrl => self.query.push(c),
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
    fn list_navigation_is_clamped() {
        let mut app = App::new(Picker::halfblocks());
        app.focus = Focus::List;
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.list.selected(), None);
    }
}
