//! Rendering: turns the current [`App`] state into a ratatui frame.
//!
//! The layout mirrors the `nmsweep` prototype: a single rounded panel with a
//! header (live "reclaimable" readout), an activity bar, a sortable table and a
//! keybinding footer, plus confirmation and session-summary overlays.

use std::path::Path;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
};

use crate::app::{App, EntryStatus, SortMode};
use crate::fs_utils::{format_age, format_thousands, human_size, is_stale};

const PANEL_BG: Color = Color::Rgb(0x14, 0x16, 0x1a);
const DIALOG_BG: Color = Color::Rgb(0x18, 0x1b, 0x20);
const BORDER: Color = Color::Rgb(0x2b, 0x2f, 0x36);
const FG: Color = Color::Rgb(0xe6, 0xe8, 0xec);
const SUBTLE: Color = Color::Rgb(0xc8, 0xcc, 0xd2);
const MUTED: Color = Color::Rgb(0x8b, 0x90, 0x9a);
const DIM: Color = Color::Rgb(0x5c, 0x62, 0x6c);
const ACCENT: Color = Color::Rgb(0x5f, 0xd0, 0xc5);
const ACCENT_DIM: Color = Color::Rgb(0x35, 0x6b, 0x66);
const TRACK: Color = Color::Rgb(0x24, 0x28, 0x2e);
const STALE: Color = Color::Rgb(0xe0, 0xb8, 0x77);
const DANGER: Color = Color::Rgb(0xe0, 0x8a, 0x7a);
const CURSOR_BG: Color = Color::Rgb(0x24, 0x2a, 0x32);

/// Shared column layout for the header and every row.
// The table spans the full inner width so the row highlight bleeds edge-to-edge.
// Empty gutter/gap columns re-create the side padding and inter-column spacing
// without insetting the table area, so the highlight bar still touches the
// panel borders. Because spacing lives in these columns, column_spacing is 0.
const COLS: [Constraint; 7] = [
    Constraint::Length(2),  // left gutter
    Constraint::Min(10),    // PATH
    Constraint::Length(2),  // gap
    Constraint::Length(14), // MODIFIED
    Constraint::Length(2),  // gap
    Constraint::Length(9),  // SIZE
    Constraint::Length(2),  // right gutter
];

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Fill the whole terminal with the panel background so the app reads as a
    // single dark surface regardless of the host terminal theme.
    frame.render_widget(Block::default().style(Style::default().bg(PANEL_BG)), area);

    let title_path = Span::styled(
        format!("nukenpm — {}", app.root.display()),
        Style::default().fg(MUTED),
    );
    let panel = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL_BG))
        .title(Line::from(title_path).centered());
    let inner = panel.inner(area);
    frame.render_widget(panel, area);

    // Breathing room on the sides, matching the prototype's padding.
    let padded = Layout::horizontal([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(inner)[1];

    let rows = Layout::vertical([
        Constraint::Length(1), // spacer
        Constraint::Length(2), // header
        Constraint::Length(1), // progress bar
        Constraint::Length(1), // spacer
        Constraint::Min(1),    // table (header + rows)
        Constraint::Length(1), // spacer
        Constraint::Length(1), // footer
        Constraint::Length(1), // spacer
    ])
    .split(padded);

    render_header(frame, app, rows[1]);
    render_progress(frame, app, rows[2]);
    // The table renders across the full inner width (not the padded region) so
    // its row highlight bleeds edge-to-edge; padding is handled by gutter columns.
    let table_area = Rect {
        x: inner.x,
        y: rows[4].y,
        width: inner.width,
        height: rows[4].height,
    };
    render_table(frame, app, table_area);
    render_footer(frame, app, rows[6]);

    if app.confirm.is_some() {
        render_confirm(frame, app, area);
    }
    if app.summary {
        render_summary(frame, app, area);
    }
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::horizontal([Constraint::Min(10), Constraint::Length(20)]).split(area);

    let glyph = if app.scanning { app.spinner() } else { "◈" };
    let title = Line::from(vec![
        Span::styled(format!("{glyph} "), Style::default().fg(ACCENT)),
        Span::styled("nukenpm ", Style::default().fg(FG)),
        Span::styled(concat!("v", env!("CARGO_PKG_VERSION")), Style::default().fg(DIM)),
    ]);

    let status = if app.scanning {
        format!("Scanning… {} found", app.entries.len())
    } else if app.marked_count() > 0 {
        format!(
            "{} selected · {} to free",
            app.marked_count(),
            human_size(app.marked_size())
        )
    } else {
        format!(
            "Scan complete · {} {} found",
            app.visible().len(),
            app.target
        )
    };
    let status = Line::from(Span::styled(status, Style::default().fg(MUTED)));
    frame.render_widget(Paragraph::new(vec![title, status]), cols[0]);

    let label = Line::from(Span::styled("RECLAIMABLE", Style::default().fg(DIM))).right_aligned();
    let value = Line::from(Span::styled(
        human_size(app.reclaimable()),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    ))
    .right_aligned();
    frame.render_widget(Paragraph::new(vec![label, value]), cols[1]);
}

/// A slim activity bar: an indeterminate sweep while scanning, a settled dim
/// bar once the scan is complete.
fn render_progress(frame: &mut Frame, app: &App, area: Rect) {
    let w = area.width as usize;
    if w == 0 {
        return;
    }

    let line = if app.scanning {
        let seg = (w / 3).max(2);
        let period = w + seg;
        let start = (app.anim() % period) as isize - seg as isize;
        let a = start.clamp(0, w as isize) as usize;
        let b = (start + seg as isize).clamp(0, w as isize) as usize;
        Line::from(vec![
            Span::styled("─".repeat(a), Style::default().fg(TRACK)),
            Span::styled("━".repeat(b - a), Style::default().fg(ACCENT)),
            Span::styled("─".repeat(w - b), Style::default().fg(TRACK)),
        ])
    } else {
        Line::from(Span::styled(
            "━".repeat(w),
            Style::default().fg(ACCENT_DIM),
        ))
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn render_table(frame: &mut Frame, app: &App, area: Rect) {
    let vis = app.visible();

    if vis.is_empty() {
        let msg = if app.scanning {
            "Searching…"
        } else {
            "Nothing found — you're all clean 🎉"
        };
        let msg_area = Rect {
            x: area.x + 2,
            y: area.y,
            width: area.width.saturating_sub(4),
            height: area.height,
        };
        frame.render_widget(
            Paragraph::new(msg).style(Style::default().fg(DIM)),
            msg_area,
        );
        return;
    }

    let arrow = |active: bool| if active { " ▼" } else { "" };
    let gutter = || Cell::from("");
    let header = Row::new([
        gutter(),
        Cell::from(Line::from(format!("PATH{}", arrow(app.sort == SortMode::Path)))),
        gutter(),
        Cell::from(
            Line::from(format!("MODIFIED{}", arrow(app.sort == SortMode::Modified))).right_aligned(),
        ),
        gutter(),
        Cell::from(Line::from(format!("SIZE{}", arrow(app.sort == SortMode::Size))).right_aligned()),
        gutter(),
    ])
    .style(Style::default().fg(DIM))
    .bottom_margin(1);

    let rows = vis.iter().enumerate().map(|(i, e)| {
        let is_cursor = i == app.cursor;
        let marked = app.is_marked(&e.path);
        let stale = is_stale(e.modified);

        let symbol = if marked {
            "◉ "
        } else if is_cursor {
            "▸ "
        } else {
            "  "
        };
        let name_color = if marked {
            ACCENT
        } else if is_cursor {
            Color::White
        } else {
            FG
        };
        let weight = if marked || is_cursor {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        let (prefix, name, suffix) = split_path(&e.path, &app.root, &app.target);
        let path_cell = Cell::from(Line::from(vec![
            Span::styled(symbol, Style::default().fg(ACCENT)),
            Span::styled(prefix, Style::default().fg(DIM)),
            Span::styled(name, Style::default().fg(name_color).add_modifier(weight)),
            Span::styled(suffix, Style::default().fg(DIM)),
        ]));

        let mod_color = if is_cursor {
            SUBTLE
        } else if stale {
            STALE
        } else {
            MUTED
        };
        let mod_cell = Cell::from(
            Line::from(format_age(e.modified)).right_aligned(),
        )
        .style(Style::default().fg(mod_color));

        let (size_text, size_color) = match e.status {
            EntryStatus::Deleting => ("deleting…".to_string(), MUTED),
            EntryStatus::Error => ("error".to_string(), DANGER),
            _ => (
                human_size(e.size),
                if marked || is_cursor { ACCENT } else { SUBTLE },
            ),
        };
        let size_cell = Cell::from(Line::from(size_text).right_aligned())
            .style(Style::default().fg(size_color).add_modifier(weight));

        Row::new([
            gutter(),
            path_cell,
            gutter(),
            mod_cell,
            gutter(),
            size_cell,
            gutter(),
        ])
    });

    let table = Table::new(rows, COLS)
        .header(header)
        .column_spacing(0)
        .row_highlight_style(Style::default().bg(CURSOR_BG));

    let mut state = TableState::default();
    state.select(Some(app.cursor));
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let marked = app.marked_count();
    let delete_label = if marked > 0 {
        format!("delete {marked}")
    } else {
        "delete".to_string()
    };
    let all_label = if app.all_marked() { "clear" } else { "all" };

    let key = |k: &str| Span::styled(k.to_string(), Style::default().fg(MUTED).add_modifier(Modifier::BOLD));
    let lbl = |t: String| Span::styled(t, Style::default().fg(DIM));

    let line = Line::from(vec![
        key("↑↓"),
        lbl(" navigate   ".to_string()),
        key("space"),
        lbl(" select   ".to_string()),
        key("a"),
        lbl(format!(" {all_label}   ")),
        key("⏎"),
        lbl(format!(" {delete_label}   ")),
        key("s"),
        lbl(format!(" sort: {}   ", app.sort.label())),
        key("q"),
        lbl(" quit".to_string()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_confirm(frame: &mut Frame, app: &App, area: Rect) {
    let entries = app.confirm_entries();
    if entries.is_empty() {
        return;
    }
    let count = entries.len();
    let total_size: u64 = entries.iter().map(|e| e.size).sum();
    let total_files: u64 = entries.iter().map(|e| e.files).sum();

    let rect = centered(58, 12, area);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(DIALOG_BG));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let pad = Layout::horizontal([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(inner)[1];

    let title = if count > 1 {
        format!("Delete {count} folders?")
    } else {
        "Delete this folder?".to_string()
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("⚠  ", Style::default().fg(DANGER)),
            Span::styled(title, Style::default().fg(FG)),
        ]),
        Line::from(""),
    ];

    if count == 1 {
        let (prefix, name, suffix) = split_path(&entries[0].path, &app.root, &app.target);
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(DIM)),
            Span::styled(name, Style::default().fg(SUBTLE)),
            Span::styled(suffix, Style::default().fg(DIM)),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            format!("{count} {} folders selected", app.target),
            Style::default().fg(SUBTLE),
        )));
    }

    lines.push(Line::from(vec![
        Span::styled(
            human_size(total_size),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::styled(
            format!("{} files", format_thousands(total_files)),
            Style::default().fg(DIM),
        ),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Permanently removes it from disk. Regenerate",
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(vec![
        Span::styled("later with ", Style::default().fg(DIM)),
        Span::styled("npm install", Style::default().fg(MUTED)),
        Span::styled(".", Style::default().fg(DIM)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            " ⏎ Delete ",
            Style::default()
                .bg(DANGER)
                .fg(PANEL_BG)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(" esc Cancel ", Style::default().fg(MUTED)),
    ]));

    frame.render_widget(Paragraph::new(lines), pad);
}

fn render_summary(frame: &mut Frame, app: &App, area: Rect) {
    let rect = centered(48, 11, area);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(DIALOG_BG));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let n = app.deleted_count();
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "✓",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .centered(),
        Line::from(Span::styled("SESSION COMPLETE", Style::default().fg(DIM))).centered(),
        Line::from(""),
        Line::from(Span::styled(
            format!("{} freed", human_size(app.freed)),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ))
        .centered(),
        Line::from(Span::styled(
            format!("{n} folder{} removed", if n == 1 { "" } else { "s" }),
            Style::default().fg(MUTED),
        ))
        .centered(),
        Line::from(""),
        Line::from(vec![
            Span::styled(" r ", Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            Span::styled(" scan again     ", Style::default().fg(DIM)),
            Span::styled(" q ", Style::default().fg(FG).add_modifier(Modifier::BOLD)),
            Span::styled(" quit", Style::default().fg(DIM)),
        ])
        .centered(),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

/// Split a target path into `(prefix, name, suffix)` for display, where `name`
/// is the parent project directory and `suffix` is the target (`/node_modules`).
fn split_path(path: &Path, root: &Path, target: &str) -> (String, String, String) {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let comps: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();

    match comps.len() {
        0 => (String::new(), target.to_string(), String::new()),
        1 => (String::new(), comps[0].clone(), String::new()),
        len => {
            let name = comps[len - 2].clone();
            let prefix = if len > 2 {
                format!("{}/", comps[..len - 2].join("/"))
            } else {
                String::new()
            };
            (prefix, name, format!("/{target}"))
        }
    }
}

/// A rectangle of fixed size centred within `area` (clamped to fit).
fn centered(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::scanner::Msg;
    use ratatui::{Terminal, backend::TestBackend};
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::SystemTime;

    /// Flatten a rendered frame into one string so we can search for content.
    fn render_to_string(app: &App, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..h {
            for x in 0..w {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn ready_app() -> App {
        let (tx, _rx) = mpsc::channel::<Msg>();
        let mut app = App::new(PathBuf::from("/home/me/projects"), "node_modules".into(), tx, true);
        app.handle_msg(Msg::Found {
            generation: 0,
            path: PathBuf::from("/home/me/projects/dashboard/node_modules"),
            size: 428 * 1024 * 1024,
            files: 18_400,
            modified: Some(SystemTime::now()),
        });
        app.handle_msg(Msg::Found {
            generation: 0,
            path: PathBuf::from("/home/me/projects/blog/node_modules"),
            size: 94 * 1024 * 1024,
            files: 3_200,
            modified: Some(SystemTime::now()),
        });
        app.handle_msg(Msg::Done { generation: 0 });
        app
    }

    #[test]
    fn renders_header_and_rows() {
        let out = render_to_string(&ready_app(), 80, 24);
        assert!(out.contains("nukenpm"), "header title missing:\n{out}");
        assert!(out.contains("RECLAIMABLE"), "reclaimable label missing:\n{out}");
        // Both discovered projects and the target suffix appear.
        assert!(out.contains("dashboard"), "row missing:\n{out}");
        assert!(out.contains("blog"), "row missing:\n{out}");
        assert!(out.contains("node_modules"), "suffix missing:\n{out}");
        // Size sort is the default, so its column carries the ▼ marker.
        assert!(out.contains("SIZE ▼"), "sort arrow missing:\n{out}");
        assert!(out.contains("navigate"), "footer missing:\n{out}");
    }

    #[test]
    fn confirm_dialog_shows_selection() {
        let mut app = ready_app();
        app.toggle_mark(); // mark the cursor row
        app.request_delete(); // opens the confirmation dialog
        let out = render_to_string(&app, 80, 24);
        assert!(out.contains("Delete this folder?"), "dialog title missing:\n{out}");
        assert!(out.contains("Cancel"), "cancel button missing:\n{out}");
    }

    #[test]
    fn summary_reports_freed_space() {
        let mut app = ready_app();
        // Simulate a completed deletion, then quit to the summary.
        app.handle_msg(Msg::Deleted {
            path: PathBuf::from("/home/me/projects/blog/node_modules"),
        });
        app.show_summary();
        let out = render_to_string(&app, 80, 24);
        assert!(out.contains("SESSION COMPLETE"), "summary missing:\n{out}");
        assert!(out.contains("freed"), "freed line missing:\n{out}");
    }
}
