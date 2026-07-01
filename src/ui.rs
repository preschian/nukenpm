//! Rendering: turns the current [`App`] state into a ratatui frame.

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::app::{App, EntryStatus};
use crate::fs_utils::{format_age, human_size};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(4), // header
        Constraint::Min(1),    // list
        Constraint::Length(3), // footer
    ])
    .split(frame.area());

    render_header(frame, app, chunks[0]);
    render_list(frame, app, chunks[1]);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let status_line = if app.scanning {
        let where_ = app
            .current_path
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        Line::from(vec![
            Span::styled(
                format!("{} scanning ", app.spinner()),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled(
                format!("({} dirs) ", app.dirs_scanned),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(truncate(&where_, 60), Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(Span::styled(
            "✓ scan complete",
            Style::default().fg(Color::Green),
        ))
    };

    let stats = Line::from(vec![
        Span::styled(
            format!("{} ", app.target),
            Style::default().fg(Color::Cyan).bold(),
        ),
        Span::raw(format!("found: {}   ", app.entries.len())),
        Span::styled(
            format!("reclaimable: {}   ", human_size(app.reclaimable())),
            Style::default().fg(Color::Magenta),
        ),
        Span::styled(
            format!("freed: {}   ", human_size(app.freed)),
            Style::default().fg(Color::Green).bold(),
        ),
        Span::styled(
            format!("sort: {}", app.sort.label()),
            Style::default().fg(Color::DarkGray),
        ),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " nukenpm ",
            Style::default().fg(Color::Red).bold(),
        ))
        .title(Line::from(format!(" {} ", app.root.display())).right_aligned());

    let paragraph = Paragraph::new(vec![stats, status_line]).block(block);
    frame.render_widget(paragraph, area);
}

fn render_list(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    if app.entries.is_empty() {
        let msg = if app.scanning {
            "Searching…"
        } else {
            "Nothing found — you're all clean! 🎉"
        };
        let placeholder = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL));
        frame.render_widget(placeholder, area);
        return;
    }

    let rows = app.entries.iter().map(|entry| {
        let rel = entry
            .path
            .strip_prefix(&app.root)
            .unwrap_or(&entry.path)
            .display()
            .to_string();

        let (tag, tag_style) = match entry.status {
            EntryStatus::Found => (human_size(entry.size), Style::default().fg(Color::Cyan)),
            EntryStatus::Deleting => (
                format!("… {}", human_size(entry.size)),
                Style::default().fg(Color::Yellow),
            ),
            EntryStatus::Deleted => (
                "deleted".to_string(),
                Style::default().fg(Color::DarkGray).crossed_out(),
            ),
            EntryStatus::Error => ("error".to_string(), Style::default().fg(Color::Red)),
        };

        let path_style = match entry.status {
            EntryStatus::Deleted => Style::default().fg(Color::DarkGray).crossed_out(),
            EntryStatus::Error => Style::default().fg(Color::Red),
            _ => Style::default(),
        };

        let path_text = match (&entry.error, entry.status) {
            (Some(err), EntryStatus::Error) => format!("{rel}  ({err})"),
            _ => rel,
        };

        Row::new(vec![
            Cell::from(tag).style(tag_style),
            Cell::from(format_age(entry.modified)).style(Style::default().fg(Color::DarkGray)),
            Cell::from(path_text).style(path_style),
        ])
    });

    let widths = [
        Constraint::Length(14),
        Constraint::Length(7),
        Constraint::Min(20),
    ];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec!["SIZE", "AGE", "PATH"])
                .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        )
        .block(Block::default().borders(Borders::ALL))
        .row_highlight_style(Style::default().bg(Color::Rgb(40, 40, 60)).bold())
        .highlight_symbol("▶ ");

    let mut state = TableState::default();
    state.select(Some(app.selected));
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_footer(frame: &mut Frame, area: ratatui::layout::Rect) {
    let keys: [(&str, &str); 5] = [
        ("↑/↓ j/k", "move"),
        ("space/del", "delete"),
        ("s", "sort"),
        ("q/esc", "quit"),
        ("", ""),
    ];
    let mut spans = Vec::new();
    for (key, desc) in keys {
        if key.is_empty() {
            continue;
        }
        spans.push(Span::styled(
            format!(" {key} "),
            Style::default().fg(Color::Black).bg(Color::Gray),
        ));
        spans.push(Span::styled(
            format!(" {desc}   "),
            Style::default().fg(Color::Gray),
        ));
    }
    let footer = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title(" keys "));
    frame.render_widget(footer, area);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let start = s.chars().count() - max + 1;
        format!("…{}", s.chars().skip(start).collect::<String>())
    }
}
