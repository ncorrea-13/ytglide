// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{MUTED, fit, meta, panel};
use crate::app::{App, Focus};
use crate::yt;
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::List;
    let title = format!("Results {}", app.results.len());
    let mut block = panel(&title, focused);
    if let Some(i) = app.list.selected() {
        block = block.title_bottom(
            Line::styled(format!(" {}/{} ", i + 1, app.results.len()), MUTED).right_aligned(),
        );
    }
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.results.is_empty() {
        let text = if app.loading {
            format!("{} searching…", app.spinner())
        } else {
            "type a query and press enter".to_string()
        };
        frame.render_widget(
            Paragraph::new(Line::styled(text, MUTED)),
            inner.inner(Margin::new(1, 0)),
        );
        return;
    }

    let selected = app.list.selected();
    // One column of padding each side keeps text off the border and the selection bar.
    let width = (inner.width as usize).saturating_sub(2);
    let bold = Style::new().add_modifier(Modifier::BOLD);

    // Only the selected row spends two lines; the rest stay compact so more results fit.
    let items: Vec<ListItem> = app
        .results
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if Some(i) == selected {
                return ListItem::new(vec![
                    Line::from(vec![
                        Span::raw(" "),
                        Span::styled(fit(&v.title, width), bold),
                    ]),
                    Line::from(vec![
                        Span::raw(" "),
                        Span::styled(fit(&meta(v), width), MUTED),
                    ]),
                ]);
            }
            let duration = v.duration.map(yt::fmt_duration).unwrap_or_default();
            let title = fit(&v.title, width.saturating_sub(duration.len() + 1));
            let pad = width.saturating_sub(Line::from(title.as_str()).width() + duration.len());
            ListItem::new(Line::from(vec![
                Span::raw(" "),
                Span::raw(title),
                Span::raw(" ".repeat(pad)),
                Span::styled(duration, MUTED),
            ]))
        })
        .collect();

    // REVERSED paints the whole row as a bar using the terminal's own colors.
    let bar = if focused {
        Style::new().add_modifier(Modifier::REVERSED)
    } else {
        MUTED
    };
    frame.render_stateful_widget(List::new(items).highlight_style(bar), inner, &mut app.list);

    if app.results.len() > inner.height as usize {
        let mut state = ScrollbarState::new(app.results.len()).position(selected.unwrap_or(0));
        let track = area.inner(Margin::new(0, 1));
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            track,
            &mut state,
        );
    }
}
