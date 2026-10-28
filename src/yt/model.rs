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

#[derive(Debug, PartialEq)]
pub struct Stream {
    // The first one is the main file; the rest (audio of a video+audio selection) are side tracks.
    pub urls: Vec<String>,
    pub user_agent: Option<String>,
}

pub fn parse_stream(json: &str) -> Result<Stream> {
    let v: serde_json::Value = serde_json::from_str(json).context("unexpected yt-dlp output")?;
    // Merged selections list their streams in requested_formats; a single format sits at the top level.
    let formats = match v["requested_formats"].as_array() {
        Some(f) if !f.is_empty() => f.as_slice(),
        _ => std::slice::from_ref(&v),
    };
    // URLs become mpv arguments: refuse anything that is not plain https.
    let urls = formats
        .iter()
        .map(|f| f["url"].as_str().filter(|u| u.starts_with("https://")))
        .map(|u| u.map(String::from))
        .collect::<Option<Vec<_>>>()
        .context("yt-dlp returned no playable stream")?;
    let user_agent = formats[0]["http_headers"]["User-Agent"]
        .as_str()
        .or(v["http_headers"]["User-Agent"].as_str())
        .map(String::from);
    Ok(Stream { urls, user_agent })
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
    fn stream_from_merged_formats_keeps_video_first() {
        let json = r#"{"http_headers":{"User-Agent":"top"},"requested_formats":[
            {"url":"https://v.example/video","http_headers":{"User-Agent":"UA"}},
            {"url":"https://a.example/audio"}
        ]}"#;
        let s = parse_stream(json).expect("valid json");
        assert_eq!(
            s.urls,
            ["https://v.example/video", "https://a.example/audio"]
        );
        assert_eq!(s.user_agent.as_deref(), Some("UA"));
    }

    #[test]
    fn stream_from_a_single_format() {
        let json = r#"{"url":"https://a.example/audio","http_headers":{"User-Agent":"UA"}}"#;
        let s = parse_stream(json).expect("valid json");
        assert_eq!(s.urls, ["https://a.example/audio"]);
        assert_eq!(s.user_agent.as_deref(), Some("UA"));
    }

    #[test]
    fn stream_rejects_urls_that_are_not_https() {
        assert!(parse_stream(r#"{"url":"--script=evil"}"#).is_err());
        assert!(
            parse_stream(r#"{"requested_formats":[{"url":"https://ok"},{"url":"file:///x"}]}"#)
                .is_err()
        );
        assert!(parse_stream(r#"{}"#).is_err());
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
