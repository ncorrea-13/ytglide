// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

mod events;
mod input;

use crate::config;
use crate::yt;
use events::YtEvent;
use ratatui::widgets::ListState;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::collections::HashMap;
use std::process::Child;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Search,
    List,
}

pub enum Thumb {
    Loading,
    Ready(Box<StatefulProtocol>),
    Failed,
}

pub struct Status {
    pub text: String,
    pub is_error: bool,
    shown_at: Instant,
}

pub struct App {
    pub focus: Focus,
    pub query: String,
    pub cursor: usize,
    pub results: Vec<yt::Video>,
    pub list: ListState,
    pub thumbs: HashMap<String, Thumb>,
    pub loading: bool,
    pub status: Option<Status>,
    pub show_preview: bool,
    pub playing: Option<(Child, String)>,
    pub paused: bool,
    pub should_quit: bool,
    tick_count: usize,
    search_seq: u64,
    picker: Picker,
    tx: Sender<YtEvent>,
    rx: Receiver<YtEvent>,
}

impl App {
    pub fn new(picker: Picker) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            focus: Focus::Search,
            query: String::new(),
            cursor: 0,
            results: Vec::new(),
            list: ListState::default(),
            thumbs: HashMap::new(),
            loading: false,
            status: None,
            show_preview: false,
            playing: None,
            paused: false,
            should_quit: false,
            tick_count: 0,
            search_seq: 0,
            picker,
            tx,
            rx,
        }
    }

    // Blocking work runs on a thread; the UI thread only ever reads the channel.
    fn spawn(&self, f: impl FnOnce() -> YtEvent + Send + 'static) {
        let tx = self.tx.clone();
        thread::spawn(move || {
            let _ = tx.send(f());
        });
    }

    pub fn spinner(&self) -> &'static str {
        SPINNER[self.tick_count % SPINNER.len()]
    }

    pub fn selected(&self) -> Option<&yt::Video> {
        self.list.selected().and_then(|i| self.results.get(i))
    }

    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            is_error: false,
            shown_at: Instant::now(),
        });
    }

    pub fn set_error(&mut self, text: impl Into<String>) {
        self.status = Some(Status {
            text: text.into(),
            is_error: true,
            shown_at: Instant::now(),
        });
    }

    pub fn on_tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        while let Ok(event) = self.rx.try_recv() {
            self.apply_event(event);
        }
        if let Some((child, _)) = &mut self.playing
            && !matches!(child.try_wait(), Ok(None))
        {
            self.playing = None;
        }
        if let Some(s) = &self.status
            && s.shown_at.elapsed() > Duration::from_secs(4)
        {
            self.status = None;
        }
    }

    fn submit_search(&mut self) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.search_seq += 1;
        let seq = self.search_seq;
        self.loading = true;
        self.spawn(move || YtEvent::Searched {
            seq,
            result: yt::search(&query, config::get().results),
        });
    }

    fn request_thumbnail(&mut self) {
        let Some(id) = self.selected().map(|v| v.id.clone()) else {
            return;
        };
        if self.thumbs.contains_key(&id) {
            return;
        }
        self.thumbs.insert(id.clone(), Thumb::Loading);
        self.spawn(move || YtEvent::Thumbnail {
            result: yt::fetch_thumbnail(&id),
            id,
        });
    }

    fn move_selection(&mut self, delta: isize) {
        if self.results.is_empty() {
            return;
        }
        let last = self.results.len() - 1;
        let cur = self.list.selected().unwrap_or(0) as isize;
        self.list.select(Some(
            cur.saturating_add(delta).clamp(0, last as isize) as usize
        ));
        self.request_thumbnail();
    }

    fn select_edge(&mut self, last: bool) {
        if self.results.is_empty() {
            return;
        }
        self.list
            .select(Some(if last { self.results.len() - 1 } else { 0 }));
        self.request_thumbnail();
    }

    fn play_selected(&mut self, audio_only: bool) {
        let Some(v) = self.selected() else { return };
        let (id, title) = (v.id.clone(), v.title.clone());
        self.stop_player();
        match yt::play(&id, audio_only) {
            Ok(child) => {
                self.playing = Some((child, title));
                self.paused = false;
            }
            Err(e) => self.set_error(e.to_string()),
        }
    }

    fn toggle_pause(&mut self) {
        if self.playing.is_none() {
            return;
        }
        match yt::toggle_pause() {
            Ok(paused) => self.paused = paused,
            Err(e) => self.set_error(e.to_string()),
        }
    }

    pub fn stop_player(&mut self) -> bool {
        let Some((mut child, _)) = self.playing.take() else {
            return false;
        };
        let _ = yt::quit();
        let _ = child.kill();
        let _ = child.wait();
        true
    }
}
