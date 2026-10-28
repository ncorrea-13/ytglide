// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use super::model::{Stream, Video, is_valid_id, parse_search, parse_stream};
use anyhow::{Context, Result, bail};
use image::DynamicImage;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

const WARM_TRIES: usize = 6;

// "yt-dlp" or e.g. "flatpak run io.github.yt_dlp" from config: first word is the program.
fn command(line: &str) -> Command {
    let mut parts = line.split_whitespace();
    let mut cmd = Command::new(parts.next().unwrap_or(line));
    cmd.args(parts);
    cmd
}

// 1-based, inclusive: yt-dlp pages through the results but only returns start..=end.
pub fn search(query: &str, start: usize, end: usize) -> Result<Vec<Video>> {
    let out = command(&crate::config::get().yt_dlp_cmd)
        .arg(format!("ytsearch{end}:{query}"))
        .args(["--flat-playlist", "-J", "--no-warnings"])
        .args(["-I", &format!("{start}:{end}")])
        .stdin(Stdio::null())
        .output()
        .context("could not run yt-dlp")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("yt-dlp: {}", err.lines().last().unwrap_or("failed").trim());
    }
    parse_search(&String::from_utf8_lossy(&out.stdout))
}

pub fn fetch_thumbnail(id: &str) -> Result<DynamicImage> {
    if !is_valid_id(id) {
        bail!("invalid video id");
    }
    let out = command(&crate::config::get().curl_cmd)
        .args(["-fsSL", "--max-time", "10"])
        .arg(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"))
        .stdin(Stdio::null())
        .output()
        .context("could not run curl")?;
    if !out.status.success() {
        bail!("thumbnail download failed");
    }
    image::load_from_memory(&out.stdout).context("could not decode thumbnail")
}

// XDG_RUNTIME_DIR is per-user and 0700; without it pausing is disabled rather than using a shared /tmp path.
fn socket_path() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?);
    // Flatpak mpv only shares $XDG_RUNTIME_DIR/app/<id> with the host; any other path stays inside the sandbox.
    if crate::config::get().mpv_cmd.contains("io.mpv.Mpv") {
        return Some(dir.join("app/io.mpv.Mpv/ytglide-mpv.sock"));
    }
    Some(dir.join("ytglide-mpv.sock"))
}

pub fn toggle_pause() -> Result<bool> {
    let path = socket_path().context("XDG_RUNTIME_DIR is not set")?;
    let mut stream = UnixStream::connect(path).context("player not ready")?;
    let timeout = Some(Duration::from_millis(500));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    stream.write_all(
        b"{\"command\":[\"cycle\",\"pause\"]}\n{\"command\":[\"get_property\",\"pause\"],\"request_id\":2}\n",
    )?;
    for line in BufReader::new(stream).lines() {
        let reply: serde_json::Value = serde_json::from_str(&line?)?;
        if reply["request_id"] == 2 {
            return reply["data"].as_bool().context("mpv sent no pause state");
        }
    }
    bail!("player closed")
}

// Killing `flatpak run` leaves the sandboxed mpv alive, so ask mpv itself to exit.
pub fn quit() -> Result<()> {
    let path = socket_path().context("XDG_RUNTIME_DIR is not set")?;
    let mut stream = UnixStream::connect(path).context("player not ready")?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    stream.write_all(b"{\"command\":[\"quit\"]}\n")?;
    Ok(())
}

pub fn resolve(id: &str, audio_only: bool) -> Result<Stream> {
    if !is_valid_id(id) {
        bail!("invalid video id");
    }
    let cfg = crate::config::get();
    let format = if audio_only {
        "bestaudio/best"
    } else {
        cfg.ytdl_format.as_str()
    };
    let out = command(&cfg.yt_dlp_cmd)
        .args(["-J", "--no-playlist", "--no-warnings", "-f", format])
        .arg(format!("https://www.youtube.com/watch?v={id}"))
        .stdin(Stdio::null())
        .output()
        .context("could not run yt-dlp")?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("yt-dlp: {}", err.lines().last().unwrap_or("failed").trim());
    }
    let stream = parse_stream(&String::from_utf8_lossy(&out.stdout))?;
    let agent = stream.user_agent.as_deref().unwrap_or("Mozilla/5.0");
    for url in &stream.urls {
        warm(url, agent);
    }
    Ok(stream)
}

// A fresh googlevideo URL answers 403 to its first request or two. yt-dlp's downloader retries so it never
// notices, but ffmpeg opens once and gives up: hit the URL until it answers before handing it to mpv.
fn warm(url: &str, user_agent: &str) {
    for attempt in 0..WARM_TRIES {
        if attempt > 0 {
            thread::sleep(Duration::from_secs(1));
        }
        let out = command(&crate::config::get().curl_cmd)
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}"])
            .args(["--max-time", "10", "-r", "0-0", "-A", user_agent])
            .arg(url)
            .stdin(Stdio::null())
            .output();
        if matches!(out, Ok(o) if matches!(o.stdout.as_slice(), b"200" | b"206")) {
            return;
        }
    }
}

// A killed mpv leaves its socket behind; a stale file would make the next bind fail.
fn bind_socket(cmd: &mut Command) {
    if let Some(sock) = socket_path() {
        if let Some(parent) = sock.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&sock);
        cmd.arg(format!("--input-ipc-server={}", sock.display()));
    }
}

fn launch(mut cmd: Command) -> Result<Child> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("could not run mpv")
}

pub fn play_stream(stream: &Stream, audio_only: bool) -> Result<Child> {
    let (main, sides) = stream.urls.split_first().context("no stream to play")?;
    let mut cmd = command(&crate::config::get().mpv_cmd);
    cmd.arg("--ytdl=no");
    if let Some(agent) = &stream.user_agent {
        cmd.arg(format!("--user-agent={agent}"));
    }
    if audio_only {
        cmd.arg("--no-video");
    } else {
        cmd.arg("--force-window=immediate");
    }
    bind_socket(&mut cmd);
    for url in sides {
        cmd.arg(format!("--audio-file={url}"));
    }
    cmd.arg(main);
    launch(cmd)
}

// The caller owns the child so playback can be stopped from the UI.
pub fn play(id: &str, audio_only: bool) -> Result<Child> {
    if !is_valid_id(id) {
        bail!("invalid video id");
    }
    let mut cmd = command(&crate::config::get().mpv_cmd);
    // Without a cap mpv grabs the best stream (4K/AV1), which stutters and drifts A/V on most machines.
    if audio_only {
        cmd.args(["--no-video", "--ytdl-format=bestaudio/best"]);
    } else {
        cmd.arg("--force-window=immediate");
        cmd.arg(format!(
            "--ytdl-format={}",
            crate::config::get().ytdl_format
        ));
    }
    bind_socket(&mut cmd);
    cmd.arg(format!("https://www.youtube.com/watch?v={id}"));
    launch(cmd)
}
