// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use super::model::{Video, is_valid_id, parse_search};
use anyhow::{Context, Result, bail};
use image::DynamicImage;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

// "yt-dlp" or e.g. "flatpak run io.github.yt_dlp" from config: first word is the program.
fn command(line: &str) -> Command {
    let mut parts = line.split_whitespace();
    let mut cmd = Command::new(parts.next().unwrap_or(line));
    cmd.args(parts);
    cmd
}

pub fn search(query: &str, limit: usize) -> Result<Vec<Video>> {
    let out = command(&crate::config::get().yt_dlp_cmd)
        .arg(format!("ytsearch{limit}:{query}"))
        .args(["--flat-playlist", "-J", "--no-warnings"])
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
    if let Some(sock) = socket_path() {
        // A killed mpv leaves its socket behind; a stale file would make the next bind fail.
        if let Some(parent) = sock.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::remove_file(&sock);
        cmd.arg(format!("--input-ipc-server={}", sock.display()));
    }
    cmd.arg(format!("https://www.youtube.com/watch?v={id}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("could not run mpv")
}
