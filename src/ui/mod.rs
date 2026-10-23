// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

mod preview;
mod results;

use crate::app::{App, Focus};
use crate::yt;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph},
};

// ANSI colors and DIM on purpose: no background of our own, everything follows the terminal theme.
pub const ACCENT: Style = Style::new().fg(Color::Cyan);
pub const MUTED: Style = Style::new().add_modifier(Modifier::DIM);
pub const ERROR: Style = Style::new().fg(Color::Red);
pub const OK: Style = Style::new().fg(Color::Green);
const BOLD: Style = Style::new().add_modifier(Modifier::BOLD);

// Below this width the preview no longer fits next to the results and becomes a toggled view.
const WIDE: u16 = 90;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [search, body, status] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(2),
    ])
    .areas(frame.area());

    draw_search(frame, app, search);
    let wide = body.width >= WIDE;
    if wide {
        let [rail, preview] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .areas(body);
        results::draw(frame, app, rail);
        preview::draw(frame, app, preview);
    } else if app.show_preview {
        preview::draw(frame, app, body);
    } else {
        results::draw(frame, app, body);
    }
    draw_status(frame, app, status, wide);
}

pub(super) fn panel(title: &str, focused: bool) -> Block<'_> {
    let (border, title_style) = if focused {
        (ACCENT, ACCENT.patch(BOLD))
    } else {
        (MUTED, MUTED)
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(format!(" {title} "), title_style))
}

pub(super) fn meta(v: &yt::Video) -> String {
    let mut parts = vec![v.channel.clone()];
    parts.extend(v.duration.map(yt::fmt_duration));
    parts.extend(v.views.map(|n| format!("{} views", yt::fmt_views(n))));
    parts.retain(|p| !p.is_empty());
    parts.join(" · ")
}

// Truncates by display width so wide glyphs do not push the row past the panel.
pub(super) fn fit(s: &str, max: usize) -> String {
    if Line::from(s).width() <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut width = 0;
    for c in s.chars() {
        let cw = Span::raw(c.to_string()).width();
        if width + cw + 1 > max {
            break;
        }
        out.push(c);
        width += cw;
    }
    out.push('…');
    out
}

fn draw_search(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Search;
    let block = panel("Search", focused);
    let inner = block.inner(area).inner(Margin::new(1, 0));
    frame.render_widget(block, area);
    let text = if app.query.is_empty() && !focused {
        Span::styled("press / to search youtube", MUTED)
    } else {
        Span::raw(app.query.as_str())
    };
    frame.render_widget(Paragraph::new(Line::from(text)), inner);
    if focused {
        let x = inner.x + Line::from(&app.query[..app.cursor]).width() as u16;
        frame.set_cursor_position(Position::new(
            x.min(inner.right().saturating_sub(1)),
            inner.y,
        ));
    }
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect, wide: bool) {
    let [info, keys] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);

    let (mode, mode_style) = match app.focus {
        Focus::Search => (" SEARCH ", ACCENT),
        Focus::List => (" BROWSE ", OK),
    };
    let mut spans = vec![
        Span::styled(
            mode,
            mode_style.add_modifier(Modifier::REVERSED | Modifier::BOLD),
        ),
        Span::raw("  "),
    ];
    if app.loading {
        spans.push(Span::styled(
            format!("{} searching…", app.spinner()),
            ACCENT,
        ));
    } else if let Some(s) = &app.status {
        spans.push(Span::styled(
            s.text.as_str(),
            if s.is_error { ERROR } else { OK },
        ));
    } else if let Some((_, title)) = &app.playing {
        let (label, style) = if app.paused {
            ("paused", MUTED)
        } else {
            ("♪ playing", OK)
        };
        let room = (info.width as usize).saturating_sub(mode.len() + label.len() + 6);
        spans.push(Span::styled(label, style));
        spans.push(Span::styled(format!("  {}", fit(title, room)), BOLD));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), info);

    let mut hints: Vec<(&str, &str)> = match app.focus {
        Focus::Search => vec![
            ("enter", "search"),
            ("←/→", "move"),
            ("ctrl+w", "del word"),
            ("ctrl+u", "clear"),
            ("esc", "results"),
        ],
        Focus::List => vec![("j/k", "move"), ("enter", "play"), ("a", "audio")],
    };
    if app.focus == Focus::List {
        if app.playing.is_some() {
            hints.push(("space", "pause"));
            hints.push(("s", "stop"));
        }
        if !wide {
            hints.push(("p", "preview"));
        }
        hints.push(("/", "search"));
        hints.push(("q", "quit"));
    }
    let mut spans = vec![Span::raw(" ")];
    for (key, action) in hints {
        spans.push(Span::styled(key, ACCENT.patch(BOLD)));
        spans.push(Span::styled(format!(" {action}   "), MUTED));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), keys);
}
