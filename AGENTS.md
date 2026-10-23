# AGENTS.md

ytglide: Rust TUI (Ratatui + ratatui-image) to search YouTube and play with mpv. Thin layer over `yt-dlp` (search), `curl` (thumbnails) and `mpv` (playback). No API key, no account. Linux only (uses Unix sockets).

## Commands

```sh
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings   # CI uses -D warnings
shellcheck scripts/package.sh
scripts/package.sh                           # release tarballs into dist/ (gitignored)
```

CI (`.github/workflows/ci.yml`) runs fmt, build, test, clippy and shellcheck. Run them all before committing.

## Layout

```
src/main.rs     terminal setup, event loop
src/config.rs   config.json load/create (OnceLock, `config::get()`)
src/history.rs  search + play history files next to config.json
src/yt/         backend: commands.rs (subprocesses, mpv IPC), model.rs (Video, parsing, id validation)
src/app/        state (mod.rs), key handling (input.rs), events from threads (events.rs)
src/ui/         rendering only: search box, results, preview, status bar
```

## Rules

- **Never block the UI thread.** Subprocess work (yt-dlp, curl, mpv IPC) runs on plain threads and reports back through the `YtEvent` channel. No async runtime; do not add one.
- **Validate video ids** with `is_valid_id` before they reach a URL, curl or mpv. Never build a command through a shell: use `Command` with separate args.
- **Command strings from config** (`yt_dlp_cmd`, `mpv_cmd`, `curl_cmd`) go through `command()`: the first word is the program, the rest are args (e.g. `flatpak run ...`). Reuse it, do not spawn directly.
- **mpv socket** lives at `$XDG_RUNTIME_DIR/ytglide-mpv.sock`. If `XDG_RUNTIME_DIR` is unset, pause is disabled on purpose. Do not fall back to `/tmp`.
- **Stopping mpv** goes over IPC (needed for Flatpak mpv, where killing the child does not stop playback). Keep that path working.
- **Colors:** ANSI only, no background set, so the terminal theme applies. No RGB.
- **Layout:** under 90 columns the UI is single-pane, `p` toggles the preview.
- **Config:** a new key needs a `Default` value and a `README.md` table row. `#[serde(default)]` keeps old config files working.
- **Dependencies:** keep them minimal and `default-features = false` where possible. Ask before adding one.
- **Style:** KISS. Match the surrounding code. Errors via `anyhow` with `.context(...)`. Every source file starts with the copyright + `SPDX-License-Identifier: GPL-3.0-or-later` header.

## Testing

Unit tests live next to the code (`#[cfg(test)]`) and cover parsing (`model.rs`), formatting helpers and key handling (`input.rs`). The UI has no automated tests: run it to check rendering changes. Add a test for new parsing or input logic.

## Git and releases

- Branches: work on `dev` or `feat/*`, merge to `main` through PRs.
- Conventional Commits (`feat:`, `fix:`, `chore:`).
- Release: bump `version` in `Cargo.toml`, merge to `main`, push a `v*` tag. `release.yml` builds and publishes the binaries.
- PRs: target `dev` (and `dev` -> `main` for releases). Write them with the `pr` skill (`.claude/skills/pr/SKILL.md`), which fills `.github/pull_request_template.md`.
- Don't commit `target/` or `dist/`.
- Don't change `LICENSE` or the disclaimer in `README.md`.
