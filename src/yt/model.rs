// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration: Option<u64>,
    pub views: Option<u64>,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    entries: Vec<RawEntry>,
}

#[derive(Deserialize)]
struct RawEntry {
    id: Option<String>,
    title: Option<String>,
    channel: Option<String>,
    uploader: Option<String>,
    duration: Option<f64>,
    view_count: Option<u64>,
}

// ids end up in a URL handed to mpv and curl: only accept the 11-char YouTube alphabet.
pub fn is_valid_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn parse_search(json: &str) -> Result<Vec<Video>> {
    let resp: SearchResponse = serde_json::from_str(json).context("unexpected yt-dlp output")?;
    Ok(resp
        .entries
        .into_iter()
        .filter_map(|e| {
            let id = e.id.filter(|id| is_valid_id(id))?;
            Some(Video {
                id,
                title: e.title.unwrap_or_else(|| "(untitled)".to_string()),
                channel: e.channel.or(e.uploader).unwrap_or_default(),
                duration: e.duration.map(|d| d as u64),
                views: e.view_count,
            })
        })
        .collect())
}

pub fn fmt_duration(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn fmt_views(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1}K", n as f64 / 1e3),
        1_000_000..1_000_000_000 => format!("{:.1}M", n as f64 / 1e6),
        _ => format!("{:.1}B", n as f64 / 1e9),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_and_drops_bad_ids() {
        let json = r#"{"entries":[
            {"id":"rFZHOHl-L8A","title":"lofi","channel":"Lofi Girl","duration":null,"view_count":12},
            {"id":"UC123","title":"a channel"},
            {"id":"abcdefghijk","title":"x","uploader":"Up","duration":65.0}
        ]}"#;
        let v = parse_search(json).expect("valid json");
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].channel, "Lofi Girl");
        assert_eq!(v[1].channel, "Up");
        assert_eq!(v[1].duration, Some(65));
    }

    #[test]
    fn formats() {
        assert_eq!(fmt_duration(65), "1:05");
        assert_eq!(fmt_duration(3725), "1:02:05");
        assert_eq!(fmt_views(999), "999");
        assert_eq!(fmt_views(1_500), "1.5K");
        assert_eq!(fmt_views(2_000_000), "2.0M");
    }
}
