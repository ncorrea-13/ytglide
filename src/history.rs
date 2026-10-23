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
    push_search(list, query, cap);
    write("searches", &list.join("\n"))
}

// Moves the query to the end (most recent) and drops the oldest beyond `cap`.
fn push_search(list: &mut Vec<String>, query: &str, cap: usize) {
    list.retain(|q| q != query);
    list.push(query.to_string());
    list.drain(..list.len().saturating_sub(cap));
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
    push_play(&mut list, video, cap);
    write("plays.jsonl", &to_lines(&list)?)
}

// Puts the video first (most recent); a replay replaces its older entry.
fn push_play(list: &mut Vec<Video>, video: &Video, cap: usize) {
    list.retain(|v| v.id != video.id);
    list.insert(0, video.clone());
    list.truncate(cap);
}

fn to_lines(list: &[Video]) -> Result<String> {
    let lines = list
        .iter()
        .map(serde_json::to_string)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(lines.join("\n"))
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

    fn video(id: &str, title: &str) -> Video {
        Video {
            id: id.into(),
            title: title.into(),
            channel: String::new(),
            duration: Some(65),
            views: None,
        }
    }

    #[test]
    fn push_search_moves_repeats_to_the_end_and_caps() {
        let mut list: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        push_search(&mut list, "a", 3);
        assert_eq!(list, ["b", "c", "a"]);
        push_search(&mut list, "d", 3);
        assert_eq!(list, ["c", "a", "d"]);
    }

    #[test]
    fn push_play_puts_replays_first_and_caps() {
        let mut list = vec![video("aaaaaaaaaaa", "old"), video("bbbbbbbbbbb", "b")];
        push_play(&mut list, &video("bbbbbbbbbbb", "new"), 2);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "new");
        push_play(&mut list, &video("ccccccccccc", "c"), 2);
        let ids: Vec<_> = list.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(ids, ["ccccccccccc", "bbbbbbbbbbb"]);
    }

    #[test]
    fn plays_survive_a_write_and_read() {
        let list = vec![video("aaaaaaaaaaa", "ñandú"), video("bbbbbbbbbbb", "b")];
        let text = to_lines(&list).expect("serializable");
        assert_eq!(parse_plays(&text), list);
    }
}
