// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::config;
use crate::yt::{Video, is_valid_id};
use anyhow::{Context, Result};
use std::path::PathBuf;

fn file(name: &str) -> PathBuf {
    config::config_dir().join(name)
}

fn read(name: &str) -> String {
    std::fs::read_to_string(file(name)).unwrap_or_default()
}

fn write(name: &str, text: &str) -> Result<()> {
    let path = file(name);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).context("could not create the config directory")?;
    }
    std::fs::write(&path, text).with_context(|| format!("could not write {}", path.display()))
}

// Oldest first, one query per line.
pub fn load_searches() -> Vec<String> {
    if config::get().history_size == 0 {
        return Vec::new();
    }
    read("searches")
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

pub fn record_search(list: &mut Vec<String>, query: &str) -> Result<()> {
    let cap = config::get().history_size;
    if cap == 0 {
        return Ok(());
    }
    list.retain(|q| q != query);
    list.push(query.to_string());
    list.drain(..list.len().saturating_sub(cap));
    write("searches", &list.join("\n"))
}

// Newest first, one JSON video per line. The file is user-editable input: ids are re-validated.
pub fn load_plays() -> Vec<Video> {
    parse_plays(&read("plays.jsonl"))
}

fn parse_plays(text: &str) -> Vec<Video> {
    text.lines()
        .filter_map(|l| serde_json::from_str::<Video>(l).ok())
        .filter(|v| is_valid_id(&v.id))
        .collect()
}

pub fn record_play(video: &Video) -> Result<()> {
    let cap = config::get().history_size;
    if cap == 0 {
        return Ok(());
    }
    let mut list = load_plays();
    list.retain(|v| v.id != video.id);
    list.insert(0, video.clone());
    list.truncate(cap);
    let lines = list
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?;
    write("plays.jsonl", &lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plays_skips_garbage_and_bad_ids() {
        let text = concat!(
            r#"{"id":"rFZHOHl-L8A","title":"lofi","channel":"c","duration":null,"views":null}"#,
            "\nnot json\n",
            r#"{"id":"../etc/passwd","title":"x","channel":"c","duration":null,"views":null}"#,
        );
        let plays = parse_plays(text);
        assert_eq!(plays.len(), 1);
        assert_eq!(plays[0].id, "rFZHOHl-L8A");
    }
}
