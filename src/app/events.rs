// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{App, Focus, Thumb};
use crate::yt;
use image::DynamicImage;

pub enum YtEvent {
    Searched {
        seq: u64,
        result: anyhow::Result<Vec<yt::Video>>,
    },
    Thumbnail {
        id: String,
        result: anyhow::Result<DynamicImage>,
    },
}

impl App {
    pub(super) fn apply_event(&mut self, event: YtEvent) {
        match event {
            YtEvent::Searched { seq, result } => {
                // A newer search was fired while this one ran: drop the stale answer.
                if seq != self.search_seq {
                    return;
                }
                self.loading = false;
                match result {
                    Ok(videos) if videos.is_empty() => self.set_error("no results"),
                    Ok(videos) => {
                        self.results = videos;
                        self.history_view = false;
                        self.thumbs.clear();
                        self.list.select(Some(0));
                        self.focus = Focus::List;
                        self.request_thumbnail();
                    }
                    Err(e) => self.set_error(e.to_string()),
                }
            }
            YtEvent::Thumbnail { id, result } => {
                let Some(slot) = self.thumbs.get_mut(&id) else {
                    return;
                };
                *slot = match result {
                    Ok(img) => Thumb::Ready(Box::new(self.picker.new_resize_protocol(img))),
                    Err(_) => Thumb::Failed,
                };
            }
        }
    }
}
