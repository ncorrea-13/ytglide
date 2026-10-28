// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

mod events;
mod input;

use crate::config;
use crate::history;
use crate::yt;
use events::YtEvent;
use ratatui::widgets::ListState;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::collections::HashMap;
use std::process::Child;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

// Stops endless scrolling from paging through the whole of YouTube.
const MAX_RESULTS: usize = 100;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Clone, Copy, PartialEq)]
pub enum Focus {
    Search,
    List,
    History,
    Help,
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
    pub searches: Vec<String>,
    pub hist_list: ListState,
    hist_pos: usize,
    last_query: Option<String>,
    fetched: usize,
    exhausted: bool,
    help_from: Focus,
    history_from: Focus,
    pub history_view: bool,
    pub results: Vec<yt::Video>,
    pub list: ListState,
    pub thumbs: HashMap<String, Thumb>,
    pub loading: bool,
    pub status: Option<Status>,
    pub show_preview: bool,
    pub playing: Option<(Child, String)>,
    pub resolving: bool,
    pub paused: bool,
    pub should_quit: bool,
    tick_count: usize,
    search_seq: u64,
    play_seq: u64,
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
            searches: Vec::new(),
            hist_list: ListState::default(),
            hist_pos: 0,
            last_query: None,
            fetched: 0,
            exhausted: false,
            help_from: Focus::Search,
            history_from: Focus::Search,
            history_view: false,
            results: Vec::new(),
            list: ListState::default(),
            thumbs: HashMap::new(),
            loading: false,
            status: None,
            show_preview: false,
            playing: None,
            resolving: false,
            paused: false,
            should_quit: false,
            tick_count: 0,
            search_seq: 0,
            play_seq: 0,
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

    pub fn load_history(&mut self) {
        self.searches = history::load_searches();
        self.hist_pos = self.searches.len();
    }

    fn recall(&mut self, pos: usize) {
        self.hist_pos = pos;
        self.query = self.searches.get(pos).cloned().unwrap_or_default();
        self.cursor = self.query.len();
    }

    fn toggle_help(&mut self) {
        if self.focus == Focus::Help {
            self.focus = self.help_from;
        } else {
            self.help_from = self.focus;
            self.focus = Focus::Help;
        }
    }

    fn open_history(&mut self) {
        if self.searches.is_empty() {
            self.set_status("history is empty");
            return;
        }
        self.hist_list.select(Some(0));
        self.history_from = self.focus;
        self.focus = Focus::History;
    }

    // The popup lists newest first, so row i is searches[len - 1 - i].
    fn pick_history(&mut self) {
        if let Some(i) = self.hist_list.selected() {
            self.recall(self.searches.len() - 1 - i);
        }
        self.focus = Focus::Search;
    }

    fn history_older(&mut self) {
        if self.hist_pos > 0 {
            self.recall(self.hist_pos - 1);
        }
    }

    fn history_newer(&mut self) {
        if self.hist_pos < self.searches.len() {
            self.recall(self.hist_pos + 1);
        }
    }

    fn show_history(&mut self) {
        let plays = history::load_plays();
        if plays.is_empty() {
            self.set_status("history is empty");
            return;
        }
        // Drop any search still in flight so it cannot replace the history view.
        self.search_seq += 1;
        self.loading = false;
        self.focus = Focus::List;
        self.results = plays;
        self.history_view = true;
        self.thumbs.clear();
        self.list.select(Some(0));
        self.request_thumbnail();
    }

    fn submit_search(&mut self) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            return;
        }
        if let Err(e) = history::record_search(&mut self.searches, &query) {
            self.set_error(e.to_string());
        }
        self.hist_pos = self.searches.len();
        self.start_search(query, 1, config::get().results, false);
    }

    fn start_search(&mut self, query: String, start: usize, end: usize, more: bool) {
        self.search_seq += 1;
        let seq = self.search_seq;
        self.loading = true;
        self.spawn(move || YtEvent::Searched {
            seq,
            result: yt::search(&query, start, end),
            query,
            end,
            more,
        });
    }

    // Fetches only the next page; the rows already on screen (and the selection) stay untouched.
    fn load_more(&mut self) {
        let Some(query) = self.last_query.clone() else {
            return;
        };
        if self.loading || self.history_view || self.exhausted || self.fetched >= MAX_RESULTS {
            return;
        }
        let end = (self.fetched + config::get().results).min(MAX_RESULTS);
        self.start_search(query, self.fetched + 1, end, true);
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
        if delta > 0 && self.list.selected() == Some(last) {
            self.load_more();
        }
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
        let Some(v) = self.selected().cloned() else {
            return;
        };
        self.stop_player();
        if !config::get().warm_urls {
            self.started(&v, yt::play(&v.id, audio_only));
            return;
        }
        self.resolving = true;
        let seq = self.play_seq;
        self.spawn(move || YtEvent::Resolved {
            seq,
            result: yt::resolve(&v.id, audio_only),
            video: v,
            audio_only,
        });
    }

    fn started(&mut self, v: &yt::Video, child: anyhow::Result<Child>) {
        match child {
            Ok(child) => {
                self.playing = Some((child, v.title.clone()));
                self.paused = false;
                if let Err(e) = history::record_play(v) {
                    self.set_error(e.to_string());
                }
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
        // Bumping the sequence makes the answer of an in-flight resolve stale.
        self.play_seq += 1;
        let resolving = std::mem::take(&mut self.resolving);
        let Some((mut child, _)) = self.playing.take() else {
            return resolving;
        };
        let _ = yt::quit();
        let _ = child.kill();
        let _ = child.wait();
        true
    }
}
