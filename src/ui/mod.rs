// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

mod preview;
mod results;

use crate::app::{App, Focus};
use crate::yt;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Flex, Layout, Margin, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, List, ListItem, Paragraph},
};

// ANSI colors and DIM on purpose: no background of our own, everything follows the terminal theme.
pub const ACCENT: Style = Style::new().fg(Color::Cyan);
pub const MUTED: Style = Style::new().add_modifier(Modifier::DIM);
pub const ERROR: Style = Style::new().fg(Color::Red);
pub const HISTORY: Style = Style::new().fg(Color::Yellow);
pub const HELP: Style = Style::new().fg(Color::Magenta);
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
    match app.focus {
        Focus::History => draw_history(frame, app),
        Focus::Help => draw_help(frame),
        _ => {}
    }
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let [row] = Layout::horizontal([Constraint::Length(w.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [popup] = Layout::vertical([Constraint::Length(h.min(area.height))])
        .flex(Flex::Center)
        .areas(row);
    popup
}

// Keep in sync with the keybinding tables in README.md.
const KEYS: &[(&str, &str)] = &[
    ("Search box", ""),
    ("enter", "search"),
    ("←/→  home/end", "move the cursor"),
    ("ctrl+w  ctrl+u", "delete word / clear"),
    ("↑ ctrl+p  ctrl+n", "previous / next search"),
    ("ctrl+r", "searches popup"),
    ("ctrl+o", "watched videos"),
    ("tab  esc", "go to the results"),
    ("Results", ""),
    ("j/k  ↑/↓  g/G", "move / first / last"),
    ("enter  a", "play video / audio only"),
    ("space  s", "pause / stop"),
    ("H", "watched videos"),
    ("p", "toggle preview (narrow)"),
    ("/  i  tab", "back to the search box"),
    ("q  esc", "quit"),
    ("Anywhere", ""),
    ("F1  ?", "this help"),
    ("ctrl+c", "quit"),
];

fn draw_help(frame: &mut Frame) {
    let popup = centered(frame.area(), 52, KEYS.len() as u16 + 2);
    let block = panel("Keybindings", Some(HELP));
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    let lines: Vec<Line> = KEYS
        .iter()
        .map(|(key, action)| {
            if action.is_empty() {
                Line::styled(format!(" {key}"), BOLD)
            } else {
                Line::from(vec![
                    Span::styled(format!("  {key:<18}"), HELP.patch(BOLD)),
                    Span::raw(*action),
                ])
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_history(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let h = (app.searches.len() as u16 + 2).clamp(3, area.height.saturating_sub(4).max(3));
    let popup = centered(area, 60, h);
    let block = panel("History", Some(HISTORY));
    let inner = block.inner(popup);
    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);
    let width = (inner.width as usize).saturating_sub(2);
    let items: Vec<ListItem> = app
        .searches
        .iter()
        .rev()
        .map(|q| ListItem::new(format!(" {}", fit(q, width))))
        .collect();
    let bar = Style::new().add_modifier(Modifier::REVERSED);
    frame.render_stateful_widget(
        List::new(items).highlight_style(bar),
        inner,
        &mut app.hist_list,
    );
}

// Each mode owns one color: the badge, the focused panel and the key hints all follow it.
fn mode_style(focus: Focus) -> Style {
    match focus {
        Focus::Search => ACCENT,
        Focus::List => OK,
        Focus::History => HISTORY,
        Focus::Help => HELP,
    }
}

pub(super) fn panel(title: &str, accent: Option<Style>) -> Block<'_> {
    let (border, title_style) = match accent {
        Some(a) => (a, a.patch(BOLD)),
        None => (MUTED, MUTED),
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
    let block = panel("Search", focused.then_some(ACCENT));
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

    let accent = mode_style(app.focus);
    let mode = match app.focus {
        Focus::Search => " SEARCH ",
        Focus::List => " BROWSE ",
        Focus::History => " HISTORY ",
        Focus::Help => " HELP ",
    };
    let version = format!("ytglide v{}", env!("CARGO_PKG_VERSION"));
    let mut spans = vec![
        Span::styled(
            mode,
            accent.add_modifier(Modifier::REVERSED | Modifier::BOLD),
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
        let room =
            (info.width as usize).saturating_sub(mode.len() + label.len() + version.len() + 8);
        spans.push(Span::styled(label, style));
        spans.push(Span::styled(format!("  {}", fit(title, room)), BOLD));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), info);
    frame.render_widget(
        Paragraph::new(Span::styled(format!("{version} "), accent)).alignment(Alignment::Right),
        info,
    );

    let mut hints: Vec<(&str, &str)> = match app.focus {
        Focus::Search => vec![
            ("enter", "search"),
            ("ctrl+r", "searches"),
            ("ctrl+o", "watched"),
        ],
        Focus::History => vec![("j/k", "move"), ("enter", "pick"), ("esc", "close")],
        Focus::Help => vec![("any key", "close")],
        Focus::List => vec![
            ("j/k", "move"),
            ("enter", "play"),
            ("a", "audio"),
            ("H", "watched"),
        ],
    };
    if app.focus == Focus::Search && !app.results.is_empty() {
        hints.push(("esc", "results"));
    }
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
    if app.focus != Focus::Help {
        hints.push(("F1", "help"));
    }
    let mut spans = vec![Span::raw(" ")];
    for (key, action) in hints {
        spans.push(Span::styled(key, accent.patch(BOLD)));
        spans.push(Span::styled(format!(" {action}   "), MUTED));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), keys);
}
