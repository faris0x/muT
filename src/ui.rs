use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;

// Theme is loaded from app.config.theme — no hardcoded constants.

pub fn render(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let cmd_active = matches!(app.mode, crate::app::Mode::Command { .. })
        || matches!(app.mode, crate::app::Mode::Confirm { .. })
        || matches!(app.mode, crate::app::Mode::ErrorView { .. })
        || matches!(app.mode, crate::app::Mode::Help);
    let status_height: u16 = if cmd_active { 2 } else { 1 };

    let layout = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(status_height),
    ]);
    let [content_area, status_area] = layout.areas(area);

    if matches!(app.mode, crate::app::Mode::Help) {
        render_help(app, frame, content_area);
    } else {
        render_editor(app, frame, content_area);
    }

    render_status(app, frame, status_area, cmd_active);
}

fn render_editor(app: &App, frame: &mut Frame, area: Rect) {
    let buffer = &app.buffer;
    let total_lines = buffer.line_count().max(1);

    // Gutter width: log10 of total lines + 2 (for " │ ")
    let gutter_width = if total_lines > 0 {
        (total_lines.ilog10() as usize) + 2
    } else {
        2
    };
    let gutter_width_u16 = gutter_width as u16;
    let gutter_width_c = Constraint::Length(gutter_width_u16);

    let layout = Layout::horizontal([gutter_width_c, Constraint::Min(1)]);
    let [gutter_area, text_area] = layout.areas(area);

    let scroll_line = app.scroll.line.min(
        total_lines.saturating_sub(1),
    );
    let scroll_col = app.scroll.col;

    render_gutter(app, frame, gutter_area, scroll_line, total_lines);
    render_text(app, frame, text_area, scroll_line, scroll_col);
}

fn render_gutter(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    scroll_line: usize,
    total_lines: usize,
) {
    let mut lines = Vec::new();
    let visible = area.height as usize;

    for i in 0..visible {
        let line_idx = scroll_line + i;
        let is_cursor_line = line_idx == app.cursor.line;

        let style = if is_cursor_line {
            Style::default()
                .fg(Color::Rgb(180, 180, 180))
                .bg(app.config.theme.cursor_line_bg)
        } else {
            Style::default()
                .fg(app.config.theme.line_num_fg)
                .bg(app.config.theme.gutter_bg)
        };

        if line_idx < total_lines {
            let num = format!("{:>width$}", line_idx + 1, width = area.width as usize - 2);
            // Show `~` for blank lines after buffer end? We'll show numbers for all lines in buffer.
            lines.push(Line::from(Span::styled(
                format!("{}", num),
                style,
            )));
        } else if line_idx == total_lines && total_lines > 0 {
            // Show `~` for empty lines after EOF
            let tilde = Span::styled(
                format!("{:>width$} ", "~", width = area.width as usize - 1),
                Style::default().fg(app.config.theme.line_num_fg).bg(app.config.theme.line_num_bg),
            );
            lines.push(Line::from(tilde));
        } else {
            break;
        }
    }

    let gutter_widget = Paragraph::new(lines).bg(app.config.theme.gutter_bg);
    frame.render_widget(gutter_widget, area);
}

fn is_latex(buffer: &crate::buffer::Buffer) -> bool {
    buffer
        .path()
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(|e| e == "tex")
        .unwrap_or(true)
}

fn render_text(
    app: &App,
    frame: &mut Frame,
    area: Rect,
    scroll_line: usize,
    scroll_col: usize,
) {
    let buffer = &app.buffer;
    let vis_lines = area.height as usize;
    let vis_cols = area.width as usize;
    let total = buffer.line_count();

    // Collect visible raw line strings for batch highlighting.
    let mut raw_lines: Vec<String> = Vec::with_capacity(vis_lines);
    let mut line_indices: Vec<usize> = Vec::with_capacity(vis_lines);
    for i in 0..vis_lines {
        let line_idx = scroll_line + i;
        if line_idx < total {
            raw_lines.push(buffer.line(line_idx));
            line_indices.push(line_idx);
        } else {
            break;
        }
    }

    // Highlight the visible block.
    let highlighted = app
        .highlighter
        .highlight_lines(&raw_lines, is_latex(buffer));

    // Compute selection byte range (if any) for the visible block.
    let sel_range = if app.cursor.has_selection() {
        app.cursor.selection_range(&buffer)
    } else {
        None
    };

    let mut ratatui_lines: Vec<Line> = Vec::with_capacity(vis_lines);

    for (hl_idx, spans) in highlighted.iter().enumerate() {
        let line_idx = line_indices[hl_idx];
        let is_cursor_line = line_idx == app.cursor.line;

        let bg = if is_cursor_line {
            app.config.theme.cursor_line_bg
        } else {
            Color::Reset
        };

        // Compute selection byte range on this line (unclipped coordinates).
        let line_sel = sel_range.and_then(|(sel_start, sel_end)| {
            let line_start = buffer.line_to_byte(line_idx);
            let line_end = line_start + buffer.line_len_bytes(line_idx);
            let lo = sel_start.max(line_start);
            let hi = sel_end.min(line_end);
            if lo < hi {
                Some((lo - line_start, hi - line_start))
            } else {
                None
            }
        });

        // Apply selection highlighting first (full-line spans), then
        // clip to the horizontal scroll window.
        let with_sel = if let Some((lo, hi)) = line_sel {
            span_bg_overlay(spans, lo, hi, app.config.theme.selection_bg, bg)
        } else {
            span_bg(spans, bg)
        };
        let clipped = clip_spans_horiz(&with_sel, scroll_col, vis_cols);

        let styled_spans: Vec<Span> = clipped
            .into_iter()
            .map(|(style, text)| Span::styled(text, style))
            .collect();

        ratatui_lines.push(Line::from(styled_spans));
    }

    // If the buffer is empty, push a single empty line so the cursor renders.
    if ratatui_lines.is_empty() {
        ratatui_lines.push(Line::from(""));
    }

    let text_widget = Paragraph::new(ratatui_lines);
    frame.render_widget(text_widget, area);

    // Cursor position — subtract horizontal scroll offset.
    let cursor_line = app.cursor.line;
    let cursor_col = app.cursor.col;
    if cursor_line >= scroll_line && cursor_line < scroll_line + vis_lines {
        let screen_row = (cursor_line - scroll_line) as u16;
        let screen_col = cursor_col.saturating_sub(scroll_col) as u16;
        frame.set_cursor_position((
            area.x + screen_col.min(area.width.saturating_sub(1)),
            area.y + screen_row,
        ));
    }

    // Ghost text (dimmed suggestion after cursor)
    if let Some(ref ghost) = app.ghost {
        if cursor_line >= scroll_line && cursor_line < scroll_line + vis_lines {
            let screen_row = (cursor_line - scroll_line) as u16;
            let screen_col = cursor_col.saturating_sub(scroll_col) as u16;
            let ghost_x = area.x + screen_col.min(area.width.saturating_sub(2));
            let ghost_y = area.y + screen_row;
            if ghost_x + 2 < area.x + area.width {
                let max_w = (area.width - (screen_col.min(area.width.saturating_sub(1)))).min(ghost.len() as u16);
                if max_w > 0 {
                    let display = if (ghost.len() as u16) > max_w { format!("{}…", &ghost[..max_w as usize - 1]) } else { ghost.clone() };
                    let ghost_style = Style::default().fg(app.config.theme.ghost_fg);
                    let span = Span::styled(display, ghost_style);
                    frame.render_widget(
                        Paragraph::new(Line::from(span)),
                        Rect { x: ghost_x, y: ghost_y, width: max_w, height: 1 },
                    );
                }
            }
        }
    }
}

fn render_help(_app: &App, frame: &mut Frame, area: Rect) {
    let header = "μT — keybindings (Esc to close)";
    let items: Vec<&str> = crate::app::HELP_TEXT.lines().collect();
    let mut lines: Vec<Line> = Vec::with_capacity(items.len() + 2);

    lines.push(Line::from(Span::styled(
        header,
        Style::default()
            .fg(Color::Rgb(200, 200, 220))
            .add_modifier(ratatui::style::Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(Color::Rgb(60, 60, 80)),
    )));

    for item in items {
        lines.push(Line::from(Span::styled(
            item,
            Style::default().fg(Color::Rgb(180, 180, 200)),
        )));
    }

    let widget = Paragraph::new(lines).bg(Color::Rgb(15, 15, 20));
    frame.render_widget(widget, area);
}

/// Given the highlighted span list for a single line, keep only the portion
/// that falls within `[scroll_col, scroll_col + vis_cols)`.
fn clip_spans_horiz(
    spans: &[(ratatui::style::Style, String)],
    scroll_col: usize,
    vis_cols: usize,
) -> Vec<(ratatui::style::Style, String)> {
    let end = scroll_col.saturating_add(vis_cols);
    let mut result = Vec::new();
    let mut offset = 0usize;

    for (style, text) in spans {
        let span_start = offset;
        let span_end = offset + text.len();

        // Does this span overlap the visible window?
        if span_end > scroll_col && span_start < end {
            let lo = if span_start < scroll_col {
                scroll_col - span_start
            } else {
                0
            };
            let hi = if span_end > end {
                span_end - end
            } else {
                0
            };
            let clipped = &text[lo..text.len() - hi];
            result.push((*style, clipped.to_string()));
        }

        offset = span_end;
    }

    result
}

/// Set a flat background on all spans (used for cursor-line highlight).
fn span_bg(
    spans: &[(ratatui::style::Style, String)],
    bg: Color,
) -> Vec<(ratatui::style::Style, String)> {
    spans
        .iter()
        .map(|(style, text)| {
            let mut s = *style;
            s.bg = Some(bg);
            (s, text.clone())
        })
        .collect()
}

/// Overlay a selection background on the byte range `[lo, hi)` of the line,
/// using `sel_bg` for the selected portion and `default_bg` for the rest.
fn span_bg_overlay(
    spans: &[(ratatui::style::Style, String)],
    lo: usize,
    hi: usize,
    sel_bg: Color,
    default_bg: Color,
) -> Vec<(ratatui::style::Style, String)> {
    let mut out = Vec::new();
    let mut offset = 0usize;

    for (style, text) in spans {
        let span_start = offset;
        let span_end = offset + text.len();

        // Before selection
        if span_start < lo {
            let pre_end = lo.min(span_end);
            let pre_len = pre_end - span_start;
            if pre_len > 0 {
                let mut s = *style;
                s.bg = Some(default_bg);
                out.push((s, text[..pre_len].to_string()));
            }
        }

        // Selection portion
        let sel_start = span_start.max(lo);
        let sel_end = span_end.min(hi);
        if sel_start < sel_end {
            let start_in_span = sel_start - span_start;
            let len = sel_end - sel_start;
            let mut s = *style;
            s.bg = Some(sel_bg);
            out.push((s, text[start_in_span..start_in_span + len].to_string()));
        }

        // After selection
        let post_start = span_start.max(hi);
        if post_start < span_end {
            let start_in_span = post_start - span_start;
            let mut s = *style;
            s.bg = Some(default_bg);
            out.push((s, text[start_in_span..].to_string()));
        }

        offset = span_end;
    }

    out
}

fn render_status(app: &App, frame: &mut Frame, area: Rect, cmd_active: bool) {
    if cmd_active {
        let [cmd_bar_area, status_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
        render_command_bar(app, frame, cmd_bar_area);
        render_status_bar(app, frame, status_area);
    } else {
        render_status_bar(app, frame, area);
    }
}

fn render_command_bar(app: &App, frame: &mut Frame, area: Rect) {
    if matches!(app.mode, crate::app::Mode::Help) {
        let widget = Paragraph::new(Line::from(Span::styled(
            "μT keybindings — Esc to close",
            Style::default()
                .fg(Color::Rgb(180, 220, 255))
                .bg(app.config.theme.command_bg),
        )))
        .bg(app.config.theme.command_bg);
        frame.render_widget(widget, area);
        return;
    }

    if let crate::app::Mode::ErrorView { ref errors, ref selected } = app.mode {
        let (_, msg) = &errors[*selected.min(&errors.len().saturating_sub(1))];
        let lines: Vec<&str> = msg.lines().collect();
        let display = format!("[{}/{}] {}", selected + 1, errors.len(), lines[0]);
        let widget = Paragraph::new(Line::from(Span::styled(
            display,
            Style::default()
                .fg(Color::Rgb(255, 150, 150))
                .bg(app.config.theme.command_bg)
                .add_modifier(Modifier::BOLD),
        )))
        .bg(app.config.theme.command_bg);
        frame.render_widget(widget, area);
        return;
    }

    if let crate::app::Mode::Command { ref input, ref kind } = app.mode {
        let prompt = match kind {
            crate::app::CommandKind::SaveAs { .. } => "Save As: ",
            crate::app::CommandKind::Open => "Open: ",
            crate::app::CommandKind::Find => "Find: ",
            crate::app::CommandKind::GotoLine => "Go To Line: ",
        };
        let display = format!("{}{}", prompt, input);
        let widget = Paragraph::new(Line::from(Span::styled(
            display,
            Style::default().fg(app.config.theme.command_fg).bg(app.config.theme.command_bg),
        )))
        .bg(app.config.theme.command_bg);
        frame.render_widget(widget, area);
        frame.set_cursor_position((
            area.x + prompt.len() as u16 + input.len() as u16,
            area.y,
        ));
    } else if let crate::app::Mode::Confirm { ref message, .. } = app.mode {
        let widget = Paragraph::new(Line::from(Span::styled(
            message,
            Style::default()
                .fg(Color::Rgb(255, 200, 100))
                .bg(app.config.theme.command_bg)
                .add_modifier(Modifier::BOLD),
        )))
        .bg(app.config.theme.command_bg);
        frame.render_widget(widget, area);
    }
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let buffer = &app.buffer;
    let cursor = &app.cursor;

    let filename = buffer.path_or_untitled();
    let pos = format!("{}:{}", cursor.line + 1, cursor.col + 1);
    let mode_label = "NORMAL";
    let right = format!("{}  {}", pos, mode_label);
    let style = Style::default().fg(app.config.theme.status_fg).bg(app.config.theme.status_bg);

    let mut spans: Vec<Span> = Vec::new();

    // Leading space + filename
    spans.push(Span::styled(format!(" {} ", filename), style));

    // Modified indicator (bold + yellow)
    if buffer.modified() {
        spans.push(Span::styled(
            "•",
            Style::default()
                .fg(app.config.theme.modified_fg)
                .bg(app.config.theme.status_bg)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(" ", style));
    }

    // Status message (e.g. "Building…", "Build OK (…)", "Saved")
    let msg = app.message.trim();
    if !msg.is_empty() {
        spans.push(Span::styled(
            format!(" {} ", msg),
            Style::default().fg(Color::Rgb(180, 220, 255)).bg(app.config.theme.status_bg),
        ));
    }

    // Build in progress
    if app.build_rx.is_some() {
        spans.push(Span::styled(
            "[Building…] ",
            Style::default()
                .fg(app.config.theme.building_fg)
                .bg(app.config.theme.status_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Build result
    if let Some(ref cr) = app.last_compile {
        if cr.success {
            spans.push(Span::styled(
                "[OK] ",
                Style::default()
                    .fg(app.config.theme.ok_fg)
                    .bg(app.config.theme.status_bg),
            ));
        } else {
            let n = cr.errors.len();
            spans.push(Span::styled(
                format!("[{} err] ", n),
                Style::default()
                    .fg(app.config.theme.err_fg)
                    .bg(app.config.theme.status_bg)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    // Modified since last build
    if app.build_dirty {
        spans.push(Span::styled(
            "[modified] ",
            Style::default()
                .fg(app.config.theme.modified_fg)
                .bg(app.config.theme.status_bg),
        ));
    }

    // Compute padding so right section is right-aligned
    let used: usize = spans.iter().map(|s| s.content.len()).sum();
    let right_bytes = right.len();
    let total = area.width as usize;
    let pad_len = total.saturating_sub(used + right_bytes + 1);
    spans.push(Span::styled(" ".repeat(pad_len), style));

    // Right section: position + mode
    spans.push(Span::styled(format!(" {}", right), style));

    let widget = Paragraph::new(Line::from(spans)).bg(app.config.theme.status_bg);
    frame.render_widget(widget, area);
}
