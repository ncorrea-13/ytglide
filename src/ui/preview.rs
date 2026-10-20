// Copyright (c) 2026 Nicolás Correa
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{MUTED, meta, panel};
use crate::app::{App, Thumb};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Paragraph, Wrap},
};
use ratatui_image::StatefulImage;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = panel("Preview", false);
    let inner = block.inner(area).inner(Margin::new(1, 0));
    frame.render_widget(block, area);

    let Some(video) = app.selected().cloned() else {
        return;
    };
    let [image, details] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).areas(inner);

    match app.thumbs.get_mut(&video.id) {
        Some(Thumb::Ready(protocol)) => {
            frame.render_stateful_widget(StatefulImage::default(), image, protocol.as_mut());
        }
        Some(Thumb::Failed) => {
            frame.render_widget(Paragraph::new(Line::styled("(no thumbnail)", MUTED)), image);
        }
        _ => {
            let text = format!("{} loading thumbnail…", app.spinner());
            frame.render_widget(Paragraph::new(Line::styled(text, MUTED)), image);
        }
    }

    let lines = vec![
        Line::styled(
            video.title.clone(),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Line::styled(meta(&video), MUTED),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), details);
}
