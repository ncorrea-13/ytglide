// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{App, Focus, Thumb};
use crate::yt;
use image::DynamicImage;

pub enum YtEvent {
    Searched {
        seq: u64,
        query: String,
        end: usize,
        more: bool,
        result: anyhow::Result<Vec<yt::Video>>,
    },
    Thumbnail {
        id: String,
        result: anyhow::Result<DynamicImage>,
    },
    Resolved {
        seq: u64,
        video: yt::Video,
        audio_only: bool,
        result: anyhow::Result<yt::Stream>,
    },
}

impl App {
    pub(super) fn apply_event(&mut self, event: YtEvent) {
        match event {
            YtEvent::Searched {
                seq,
                query,
                end,
                more,
                result,
            } => {
                // A newer search was fired while this one ran: drop the stale answer.
                if seq != self.search_seq {
                    return;
                }
                self.loading = false;
                match result {
                    // The ranking shifts between calls, so skip videos we already show.
                    Ok(videos) if more => {
                        self.fetched = end;
                        let new: Vec<_> = videos
                            .into_iter()
                            .filter(|v| !self.results.iter().any(|r| r.id == v.id))
                            .collect();
                        if new.is_empty() {
                            self.exhausted = true;
                            self.set_status("no more results");
                        } else {
                            self.results.extend(new);
                        }
                    }
                    Ok(videos) if videos.is_empty() => self.set_error("no results"),
                    Ok(videos) => {
                        self.results = videos;
                        self.history_view = false;
                        self.last_query = Some(query);
                        self.fetched = end;
                        self.exhausted = false;
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
            YtEvent::Resolved {
                seq,
                video,
                audio_only,
                result,
            } => {
                // The player was stopped or replaced while yt-dlp worked: drop the stale answer.
                if seq != self.play_seq {
                    return;
                }
                self.resolving = false;
                let child = match result {
                    Ok(stream) => yt::play_stream(&stream, audio_only),
                    // Could not resolve here: let mpv try with its own yt-dlp hook.
                    Err(_) => yt::play(&video.id, audio_only),
                };
                self.started(&video, child);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui_image::picker::Picker;

    fn video(id: &str) -> yt::Video {
        yt::Video {
            id: id.into(),
            title: id.into(),
            channel: String::new(),
            duration: None,
            views: None,
        }
    }

    fn answer(app: &mut App, seq: u64, ids: &[&str]) {
        app.apply_event(YtEvent::Searched {
            seq,
            query: "q".into(),
            end: 40,
            more: true,
            result: Ok(ids.iter().map(|id| video(id)).collect()),
        });
    }

    #[test]
    fn load_more_appends_new_videos_and_keeps_the_selection() {
        let mut app = App::new(Picker::halfblocks());
        app.results = vec![video("aaaaaaaaaaa"), video("bbbbbbbbbbb")];
        app.list.select(Some(1));
        let seq = app.search_seq;
        answer(&mut app, seq, &["bbbbbbbbbbb", "ccccccccccc"]);
        let ids: Vec<_> = app.results.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["aaaaaaaaaaa", "bbbbbbbbbbb", "ccccccccccc"]);
        assert_eq!(app.list.selected(), Some(1));
        assert_eq!(app.fetched, 40);
        assert!(!app.exhausted);
    }

    #[test]
    fn load_more_marks_the_end_when_nothing_is_new() {
        let mut app = App::new(Picker::halfblocks());
        app.results = vec![video("aaaaaaaaaaa")];
        let seq = app.search_seq;
        answer(&mut app, seq, &["aaaaaaaaaaa"]);
        assert_eq!(app.results.len(), 1);
        assert!(app.exhausted);
    }

    #[test]
    fn stale_resolve_does_not_start_the_player() {
        let mut app = App::new(Picker::halfblocks());
        app.play_seq = 2;
        app.resolving = true;
        app.apply_event(YtEvent::Resolved {
            seq: 1,
            video: video("aaaaaaaaaaa"),
            audio_only: true,
            result: Err(anyhow::anyhow!("late")),
        });
        assert!(app.playing.is_none());
        assert!(app.resolving);
    }

    #[test]
    fn stop_cancels_a_pending_resolve() {
        let mut app = App::new(Picker::halfblocks());
        app.resolving = true;
        let seq = app.play_seq;
        assert!(app.stop_player());
        assert!(!app.resolving);
        assert_ne!(app.play_seq, seq);
    }

    #[test]
    fn stale_answers_are_dropped() {
        let mut app = App::new(Picker::halfblocks());
        app.search_seq = 2;
        answer(&mut app, 1, &["aaaaaaaaaaa"]);
        assert!(app.results.is_empty());
    }
}
