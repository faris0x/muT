use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind};

use crate::buffer::Buffer;
use crate::config::Config;
use crate::cursor::Cursor;
use crate::history::{Edit, History};
use crate::latex;
use crate::syntax::Highlighter;
use crate::ui;

pub const HELP_TEXT: &str = "\
Ctrl+S    Save                          Ctrl+O    Open .tex
Ctrl+N    New file                      Ctrl+Q    Quit
Ctrl+Z    Undo                          Ctrl+Y    Redo
Ctrl+C    Copy                          Ctrl+X    Cut
Ctrl+V    Paste                         Ctrl+A    Select all
Ctrl+F    Find                          Ctrl+G    Go to line
Ctrl+B    Build (pdflatex)              Ctrl+P    Toggle Zathura preview
Ctrl+E    View build errors             Ctrl+H    This help
Shift+↑↓←→  Extend selection            Tab       Accept ghost / 4 spaces
↑↓←→     Move cursor                   Ctrl+↑↓   Scroll view
{ → {}   ( → ()   $ → $$              \\begin{X}} → \\end{X} ghost";

const LATEX_TEMPLATE: &str = r#"\documentclass{article}
\usepackage[margin=1in]{geometry}
\usepackage{amsmath,amssymb}
\usepackage{graphicx}
\usepackage{xcolor}
\usepackage{hyperref}
\usepackage{tikz}
\usepackage{pgfplots}
\pgfplotsset{compat=1.18}
\usepackage{circuitikz}

\title{Untitled}
\author{}
\date{\today}

\begin{document}

\maketitle

\section{Introduction}

% Example TikZ drawing:
\begin{center}
\begin{tikzpicture}
  \draw[thick,->] (0,0) -- (4,0) node[right] {$x$};
  \draw[thick,->] (0,0) -- (0,3) node[above] {$y$};
  \draw[domain=0:3.5,smooth,variable=\x,blue] plot ({\x},{(\x)^2/4});
  \node at (2,2.5) {$y = x^2/4$};
\end{tikzpicture}
\end{center}

% Example PGFPlots:
\begin{center}
\begin{tikzpicture}
\begin{axis}[
  xlabel=$x$, ylabel=$y$,
  grid=major,
  legend entries={$\sin x$,$\cos x$}
]
  \addplot[red] {sin(deg(x))};
  \addplot[blue] {cos(deg(x))};
\end{axis}
\end{tikzpicture}
\end{center}

\end{document}
"#;

#[derive(Debug, Clone)]
pub enum CommandKind {
    SaveAs { force_tex: bool },
    Open,
    Find,
    GotoLine,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    SaveAsOverwrite,
}

#[derive(Debug, Clone)]
pub enum Mode {
    Normal,
    Command {
        input: String,
        kind: CommandKind,
    },
    Confirm {
        message: String,
        action: ConfirmAction,
    },
    ErrorView {
        errors: Vec<(usize, String)>,
        selected: usize,
    },
    Help,
}

pub struct Scroll {
    pub line: usize,
    pub col: usize,
}

pub struct App {
    // Core
    pub running: bool,
    pub mode: Mode,

    // Editor state
    pub buffer: Buffer,
    pub cursor: Cursor,
    pub scroll: Scroll,

    // Message display
    pub message: String,
    pub message_until: Instant,

    // Terminal dimensions
    pub terminal_height: u16,
    pub terminal_width: u16,

    // Undo/redo
    pub history: History,

    // Clipboard
    pub clipboard: String,

    // Syntax highlighting
    pub highlighter: Highlighter,

    // Configuration
    pub config: Config,

    // Ghost text (dimmed suggestion after cursor, accepted via Tab)
    pub ghost: Option<String>,

    // Zathura preview process
    pub zathura: Option<std::process::Child>,

    // Auto-save
    pub auto_save_timer: Instant,

    // Build / compilation
    pub last_compile: Option<latex::CompileResult>,
    pub build_rx: Option<mpsc::Receiver<latex::CompileResult>>,
    pub auto_debounce: Option<Instant>,
    pub build_dirty: bool,
    pub open_zathura_on_complete: bool,

    // Diagnostics (from LaTeX build)
    pub diagnostics: Vec<String>,
}

impl App {
    pub fn new() -> Self {
        let (terminal_width, terminal_height) = crossterm::terminal::size()
            .map(|(w, h)| (w, h))
            .unwrap_or((80, 24));
        let cfg = Config::load();
        App {
            running: true,
            mode: Mode::Normal,
            buffer: Buffer::new(),
            cursor: Cursor::new(),
            scroll: Scroll { line: 0, col: 0 },
            message: String::new(),
            message_until: Instant::now(),
            terminal_height,
            terminal_width,
            history: History::new(),
            clipboard: String::new(),
            highlighter: Highlighter::with_theme(&cfg.syntax),
            config: cfg,
            ghost: None,
            zathura: None,
            auto_save_timer: Instant::now(),
            build_rx: None,
            auto_debounce: None,
            build_dirty: false,
            open_zathura_on_complete: false,
            diagnostics: Vec::new(),
            last_compile: None,
        }
    }

    pub fn open_file(path: PathBuf) -> Self {
        let buffer = match Buffer::load(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Error opening {}: {}", path.display(), e);
                std::process::exit(1);
            }
        };
        let (terminal_width, terminal_height) = crossterm::terminal::size()
            .map(|(w, h)| (w, h))
            .unwrap_or((80, 24));
        App {
            running: true,
            mode: Mode::Normal,
            buffer,
            cursor: Cursor::new(),
            scroll: Scroll { line: 0, col: 0 },
            message: String::new(),
            message_until: Instant::now(),
            terminal_height,
            terminal_width,
            history: History::new(),
            clipboard: String::new(),
            highlighter: Highlighter::with_theme(&Config::load().syntax),
            config: Config::load(),
            ghost: None,
            zathura: None,
            auto_save_timer: Instant::now(),
            build_rx: None,
            auto_debounce: None,
            build_dirty: false,
            open_zathura_on_complete: false,
            diagnostics: Vec::new(),
            last_compile: None,
        }
}

    pub fn run(&mut self) -> color_eyre::Result<()> {
        let mut terminal = ratatui::init();
        crossterm::execute!(
            std::io::stdout(),
            crossterm::event::EnableMouseCapture,
            crossterm::event::EnableBracketedPaste,
        )?;

        let mut last_tick = Instant::now();
        let tick_rate = Duration::from_millis(50);

        while self.running {
            let now = Instant::now();
            if now >= self.message_until {
                self.message.clear();
            }

            terminal.draw(|f| ui::render(self, f))?;
            self.poll_build();
            self.check_auto_save();

            let timeout = tick_rate
                .checked_sub(now.duration_since(last_tick))
                .unwrap_or(Duration::ZERO);

            if event::poll(timeout)? {
                let event = event::read()?;

                // Show "Building…" before async build starts
                let is_build_key = matches!(&event, Event::Key(KeyEvent {
                    code: KeyCode::Char('b') | KeyCode::Char('p'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }));
                if is_build_key {
                    self.set_message("Building…");
                    let _ = terminal.draw(|f| { ui::render(self, f); }).ok();
                }

                self.handle_event(event)?;
            }

            if now.duration_since(last_tick) >= tick_rate {
                last_tick = now;
            }
        }

        // Kill zathura before tearing down the terminal.
        self.kill_zathura();

        crossterm::execute!(
            std::io::stdout(),
            crossterm::event::DisableMouseCapture,
            crossterm::event::DisableBracketedPaste,
        )?;
        ratatui::restore();
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> color_eyre::Result<()> {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Paste(text) => {
                if matches!(self.mode, Mode::Normal) {
                    self.paste_received(text);
                    self.modified_since_build();
                }
            }
            Event::Resize(w, h) => {
                self.terminal_width = w;
                self.terminal_height = h;
            }
            Event::Mouse(m) => match m.kind {
                MouseEventKind::ScrollDown => {
                    let visible = self.editor_height();
                    self.scroll.line = self
                        .scroll
                        .line
                        .saturating_add(3)
                        .min(self.buffer.line_count().saturating_sub(1));
                    // Don't scroll past the last visible line
                    let max_scroll = self
                        .buffer
                        .line_count()
                        .saturating_sub(1)
                        .saturating_sub(visible.saturating_sub(1));
                    if self.scroll.line > max_scroll {
                        self.scroll.line = max_scroll;
                    }
                }
                MouseEventKind::ScrollUp => {
                    self.scroll.line = self.scroll.line.saturating_sub(3);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    self.mouse_click(m.column, m.row);
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        match self.mode.clone() {
            Mode::Normal => self.handle_normal_key(key),
            Mode::Command { .. } => self.handle_command_key(key),
            Mode::Confirm { .. } => self.handle_confirm_key(key),
            Mode::ErrorView { .. } => self.handle_error_view_key(key),
            Mode::Help => self.handle_help_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            // Quit
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                self.running = false;
            }
            // Save
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.save();
            }
            // Open
            (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Command {
                    input: String::new(),
                    kind: CommandKind::Open,
                };
            }
            // Find
            (KeyCode::Char('f'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Command {
                    input: String::new(),
                    kind: CommandKind::Find,
                };
            }
            // Go to line
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Command {
                    input: String::new(),
                    kind: CommandKind::GotoLine,
                };
            }
            // New file
            (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
                self.new_file();
            }
            // Undo / Redo
            (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                self.undo();
            }
            (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                self.redo();
            }
            // Build / compile
            (KeyCode::Char('b'), KeyModifiers::CONTROL) => {
                self.build_async();
            }
            // Toggle Zathura preview
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                // Reap dead Zathura first
                self.check_zathura_alive();
                if self.zathura.is_some() {
                    self.kill_zathura();
                    self.set_message("Preview closed");
                } else {
                    self.open_zathura_on_complete = true;
                    self.build_async();
                }
            }
            // Clipboard
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                self.copy_line();
            }
            (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
                self.select_all();
            }
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
                self.cut_selection_or_line();
            }
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => {
                self.paste();
                self.modified_since_build();
            }
            // Help overlay
            (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Help;
            }
            // View build errors
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                if self.diagnostics.is_empty() {
                    self.set_message("No errors");
                } else {
                    let errors: Vec<(usize, String)> = self.diagnostics.iter().enumerate()
                        .map(|(i, msg)| (i, msg.clone())).collect();
                    self.mode = Mode::ErrorView { errors, selected: 0 };
                }
            }
            // Scroll view without moving cursor (must precede bare Up/Down)
            (KeyCode::Up, KeyModifiers::CONTROL) => {
                self.scroll.line = self.scroll.line.saturating_sub(1);
            }
            (KeyCode::Down, KeyModifiers::CONTROL) => {
                let max_line = self.buffer.line_count().saturating_sub(1);
                if self.scroll.line < max_line {
                    self.scroll.line += 1;
                }
            }
            // Shift+Arrow — select while moving (must precede bare arrows)
            (KeyCode::Up, KeyModifiers::SHIFT) => {
                self.cursor.move_up(&self.buffer, true);
                self.ensure_cursor_visible();
            }
            (KeyCode::Down, KeyModifiers::SHIFT) => {
                self.cursor.move_down(&self.buffer, true);
                self.ensure_cursor_visible();
            }
            (KeyCode::Left, KeyModifiers::SHIFT) => {
                self.cursor.move_left(&self.buffer, true);
                self.ensure_cursor_visible();
            }
            (KeyCode::Right, KeyModifiers::SHIFT) => {
                self.cursor.move_right(&self.buffer, true);
                self.ensure_cursor_visible();
            }
            (KeyCode::Home, KeyModifiers::SHIFT) => {
                self.cursor.move_home(&self.buffer, true);
                self.ensure_cursor_visible();
            }
            (KeyCode::End, KeyModifiers::SHIFT) => {
                self.cursor.move_end(&self.buffer, true);
                self.ensure_cursor_visible();
            }
            (KeyCode::PageUp, KeyModifiers::SHIFT) => {
                let height = self.editor_height();
                self.cursor.move_page_up(&self.buffer, height, true);
                self.ensure_cursor_visible();
            }
            (KeyCode::PageDown, KeyModifiers::SHIFT) => {
                let height = self.editor_height();
                self.cursor.move_page_down(&self.buffer, height, true);
                self.ensure_cursor_visible();
            }
            // Navigation — move cursor (clears selection)
            (KeyCode::Up, _) => {
                self.cursor.move_up(&self.buffer, false);
                self.ensure_cursor_visible();
            }
            (KeyCode::Down, _) => {
                self.cursor.move_down(&self.buffer, false);
                self.ensure_cursor_visible();
            }
            (KeyCode::Left, _) => {
                self.cursor.move_left(&self.buffer, false);
                self.ensure_cursor_visible();
            }
            (KeyCode::Right, _) => {
                self.cursor.move_right(&self.buffer, false);
                self.ensure_cursor_visible();
            }
            (KeyCode::Home, _) => {
                self.cursor.move_home(&self.buffer, false);
                self.ensure_cursor_visible();
            }
            (KeyCode::End, _) => {
                self.cursor.move_end(&self.buffer, false);
                self.ensure_cursor_visible();
            }
            (KeyCode::PageUp, _) => {
                let height = self.editor_height();
                self.cursor.move_page_up(&self.buffer, height, false);
                self.ensure_cursor_visible();
            }
            (KeyCode::PageDown, _) => {
                let height = self.editor_height();
                self.cursor.move_page_down(&self.buffer, height, false);
                self.ensure_cursor_visible();
            }
            // Text editing (clears any active selection)
            (KeyCode::Backspace, _) => {
                self.cursor.clear_selection();
                self.backspace();
                self.ensure_cursor_visible();
                self.modified_since_build();
            }
            (KeyCode::Delete, _) => {
                self.cursor.clear_selection();
                self.delete();
                self.modified_since_build();
            }
            (KeyCode::Enter, _) => {
                self.cursor.clear_selection();
                self.insert_newline();
                self.ensure_cursor_visible();
                self.modified_since_build();
            }
            (KeyCode::Tab, _) => {
                if let Some(ghost) = self.ghost.take() {
                    self.cursor.clear_selection();
                    self.insert_str_with_history(&ghost);
                    self.ensure_cursor_visible();
                } else {
                    self.cursor.clear_selection();
                    self.insert_str_with_history("    ");
                    self.ensure_cursor_visible();
                }
                self.modified_since_build();
            }
            (KeyCode::Char(c), KeyModifiers::NONE)
            | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                self.cursor.clear_selection();
                self.insert_char(c);
                self.ensure_cursor_visible();
                self.modified_since_build();
            }
            _ => {}
        }
    }

    fn handle_command_key(&mut self, key: KeyEvent) {
        let _mode = self.mode.clone();
        if let Mode::Command { ref mut input, ref kind } = &mut self.mode {
            match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    self.mode = Mode::Normal;
                    self.cancel_message();
                }
                (KeyCode::Enter, _) => {
                    let cmd_input = input.clone();
                    let cmd_kind = kind.clone();
                    self.mode = Mode::Normal;
                    self.execute_command(cmd_kind, &cmd_input);
                }
                (KeyCode::Backspace, _) => {
                    input.pop();
                }
                (KeyCode::Char(c), KeyModifiers::NONE)
                | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                    input.push(c);
                }
                _ => {}
            }
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        let action = match &self.mode {
            Mode::Confirm { action, .. } => action.clone(),
            _ => return,
        };

        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('n'), _) | (KeyCode::Char('N'), _) => {
                self.mode = Mode::Normal;
                self.set_message("Cancelled");
            }
            (KeyCode::Char('y'), _) | (KeyCode::Char('Y'), _) | (KeyCode::Enter, _) => {
                match action {
                    ConfirmAction::SaveAsOverwrite => {
                        // Re-trigger the save with the stored path
                        if let Some(path) = self.buffer.path().map(|p| p.to_path_buf()) {
                            self.mode = Mode::Normal;
                            match self.buffer.save(&path) {
                                Ok(()) => {
                                    self.buffer.set_modified(false);
                                    self.set_message("Saved");
                                    if path
                                        .extension()
                                        .and_then(|e| e.to_str())
                                        .map(|e| e == "tex")
                                        .unwrap_or(false)
                                    {
                                        self.build_async();
                                    }
                                }
                                Err(e) => {
                                    self.set_message(&format!("Error saving: {}", e));
                                }
                            }
                        } else {
                            self.mode = Mode::Normal;
                            self.set_message("No filename");
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('h'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
    }

    fn handle_error_view_key(&mut self, key: KeyEvent) {
        let (errors, selected) = match &self.mode {
            Mode::ErrorView { errors, selected } => (errors.clone(), *selected),
            _ => return,
        };
        let n = errors.len();
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
                self.mode = Mode::Normal;
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
                let next = if selected == 0 { n.saturating_sub(1) } else { selected - 1 };
                self.mode = Mode::ErrorView { errors, selected: next };
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), KeyModifiers::CONTROL) => {
                let next = if selected + 1 >= n { 0 } else { selected + 1 };
                self.mode = Mode::ErrorView { errors, selected: next };
            }
            _ => {}
        }
    }

    fn execute_command(&mut self, kind: CommandKind, input: &str) {
        match kind {
            CommandKind::SaveAs { force_tex } => {
                if input.is_empty() {
                    self.set_message("No filename given");
                    return;
                }
                let mut path = PathBuf::from(input);
                // Ctrl+S (force_tex) → only .tex allowed
                if force_tex {
                    match path.extension().and_then(|e| e.to_str()) {
                        None => { path.set_extension("tex"); }
                        Some(ext) if ext != "tex" => {
                            self.set_message("Only .tex files can be saved");
                            return;
                        }
                        _ => {}
                    }
                }
                if path.exists() {
                    self.buffer.set_path(Some(path.clone()));
                    self.mode = Mode::Confirm {
                        message: format!("Overwrite {}? (y/N)", &path.display()),
                        action: ConfirmAction::SaveAsOverwrite,
                    };
                    return;
                }
                match self.buffer.save(&path) {
                    Ok(()) => {
                        self.buffer.set_path(Some(path.clone()));
                        self.buffer.set_modified(false);
                        self.set_message("Saved");
                        if path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|e| e == "tex")
                            .unwrap_or(false)
                        {
                            self.build_async();
                        }
                    }
                    Err(e) => {
                        self.set_message(&format!("Error saving: {}", e));
                    }
                }
            }
            CommandKind::Open => {
                if input.is_empty() {
                    self.set_message("No filename given");
                    return;
                }
                let path = PathBuf::from(input);
                if path.extension().and_then(|e| e.to_str()) != Some("tex") {
                    self.set_message("Only .tex files can be opened");
                    return;
                }
                match Buffer::load(&path) {
                    Ok(buf) => {
                        self.buffer = buf;
                        self.cursor = Cursor::new();
                        self.scroll = Scroll { line: 0, col: 0 };
                        self.last_compile = None;
                        self.diagnostics = Vec::new();
                        self.set_message(&format!("Opened {}", path.display()));
                    }
                    Err(e) => {
                        self.set_message(&format!("Error opening: {}", e));
                    }
                }
            }
            CommandKind::Find => {
                if input.is_empty() {
                    self.set_message("No search term");
                    return;
                }
                self.find_next(input);
            }
            CommandKind::GotoLine => {
                let line: usize = match input.parse::<usize>() {
                    Ok(n) if n > 0 => n - 1, // Convert 1-based to 0-based
                    _ => {
                        self.set_message("Invalid line number");
                        return;
                    }
                };
                if line < self.buffer.line_count() {
                    self.cursor.line = line;
                    self.cursor.col = 0;
                    self.ensure_cursor_visible();
                    self.set_message(&format!("Goto line {}", line + 1));
                } else {
                    self.set_message("Line number out of range");
                }
            }
        }
    }

    fn find_next(&mut self, term: &str) {
        let cursor_byte = self.cursor.byte_idx(&self.buffer);
        let total = self.buffer.len_bytes();
        if total == 0 || term.is_empty() {
            self.set_message("Not found");
            return;
        }

        let text = self.buffer.text_between(cursor_byte, total);
        if let Some(pos) = text.find(term) {
            let byte_idx = cursor_byte + pos;
            self.cursor.line = self.buffer.byte_to_line(byte_idx);
            self.cursor.col = byte_idx - self.buffer.line_to_byte(self.cursor.line);
            self.ensure_cursor_visible();
            self.set_message(&format!("Found \"{}\"", term));
        } else {
            // Wrap search from beginning
            let text = self.buffer.text_between(0, cursor_byte);
            if let Some(pos) = text.find(term) {
                let byte_idx = pos;
                self.cursor.line = self.buffer.byte_to_line(byte_idx);
                self.cursor.col = byte_idx - self.buffer.line_to_byte(self.cursor.line);
                self.ensure_cursor_visible();
                self.set_message(&format!("Found \"{}\" (wrapped)", term));
            } else {
                self.set_message("Not found");
            }
        }
    }

    fn save(&mut self) {
        if let Some(path) = self.buffer.path().map(|p| p.to_path_buf()) {
            match self.buffer.save(&path) {
                Ok(()) => {
                    self.buffer.set_modified(false);
                    self.set_message("Saved");

                    // Auto-compile .tex files after save
                    if path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e == "tex")
                        .unwrap_or(false)
                    {
                        self.build_async();
                    }
                }
                Err(e) => {
                    self.set_message(&format!("Error saving: {}", e));
                }
            }
        } else {
            // No path yet — prompt for Save As
            self.mode = Mode::Command {
                input: String::new(),
            kind: CommandKind::SaveAs { force_tex: true },
        };
    }
    }

    fn new_file(&mut self) {
        self.buffer = Buffer::new();
        self.buffer.insert(0, LATEX_TEMPLATE);
        self.cursor = Cursor::new();
        self.history.clear();
        self.scroll = Scroll { line: 0, col: 0 };
        self.last_compile = None;
        self.set_message("New LaTeX document");
    }

    fn insert_char(&mut self, c: char) {
        let cursor_before = self.cursor;
        let idx = self.cursor.byte_idx(&self.buffer);

        // ── Smart close: skip over existing closing brace ──
        if c == '}'
            && idx < self.buffer.len_bytes()
            && self.buffer.text_between(idx, idx + 1) == "}"
        {
            self.cursor.col += 1;
            self.ghost_check(idx);
            return;
        }

        let mut s = String::new();
        s.push(c);
        self.buffer.insert(idx, &s);
        self.cursor.col += 1;

        // Auto-close braces
        if c == '{' {
            self.buffer.insert(idx + 1, "}");
        } else if c == '(' {
            self.buffer.insert(idx + 1, ")");
        } else if c == '$' {
            if idx > 0 && self.buffer.text_between(idx - 1, idx) == "$" {
                // Already have $$, do nothing
            } else {
                self.buffer.insert(idx + 1, "$");
            }
        } else if c == '}' {
            self.ghost_check(idx);
        } else {
            self.ghost = None;
        }

        self.history.push(Edit {
            byte_idx: idx,
            deleted: String::new(),
            inserted: s,
            cursor_before,
            cursor_after: self.cursor,
        });
    }

    fn ghost_check(&mut self, idx: usize) {
        let line_start = self.buffer.line_to_byte(self.cursor.line);
        let line_len = self.buffer.line_len_bytes(self.cursor.line);
        let line_raw = self.buffer.text_between(line_start, line_start + line_len);
        let cursor_in_line = idx.saturating_sub(line_start);
        if cursor_in_line < 1 { return; }
        let before = &line_raw[..cursor_in_line];
        if let Some(bs) = before.rfind("\\begin{") {
            let env_start = bs + 7;
            // Find the first `}` or `{` — whichever terminates the env name
            let rest = &line_raw[env_start..];
            let closes_at = rest.find('}').unwrap_or(rest.len());
            let opens_at = rest.find('{').unwrap_or(rest.len());
            let env_end = env_start + closes_at.min(opens_at);
            if env_end > env_start {
                let env = &line_raw[env_start..env_end];
                self.ghost = Some(format!("\\end{{{}}}", env));
            }
        }
    }

    fn insert_str_with_history(&mut self, s: &str) {
        let cursor_before = self.cursor;
        let idx = self.cursor.byte_idx(&self.buffer);
        self.buffer.insert(idx, s);
        self.cursor.col += s.len();

        self.history.push(Edit {
            byte_idx: idx,
            deleted: String::new(),
            inserted: s.to_string(),
            cursor_before,
            cursor_after: self.cursor,
        });
    }

    fn insert_newline(&mut self) {
        let cursor_before = self.cursor;
        let idx = self.cursor.byte_idx(&self.buffer);
        let indent = self.current_line_indent();
        self.buffer.insert(idx, "\n");
        self.cursor.line += 1;
        self.cursor.col = 0;
        let mut inserted = "\n".to_string();
        if !indent.is_empty() {
            self.buffer.insert(self.cursor.byte_idx(&self.buffer), &indent);
            self.cursor.col = indent.len();
            inserted.push_str(&indent);
        }

        self.history.push(Edit {
            byte_idx: idx,
            deleted: String::new(),
            inserted,
            cursor_before,
            cursor_after: self.cursor,
        });
    }

    fn backspace(&mut self) {
        let cursor_before = self.cursor;
        let idx = self.cursor.byte_idx(&self.buffer);
        if idx > 0 {
            let prev_byte = idx - 1;
            let prev_char = self.buffer.text_between(prev_byte, idx);
            if prev_char == "\n" {
                let cur_line = self.cursor.line;
                let prev_line = cur_line.saturating_sub(1);
                let prev_line_len = self.buffer.line_len_bytes(prev_line);
                self.buffer.remove(prev_byte, idx);
                self.cursor.line = prev_line;
                self.cursor.col = prev_line_len.saturating_sub(1);

                self.history.push(Edit {
                    byte_idx: prev_byte,
                    deleted: "\n".to_string(),
                    inserted: String::new(),
                    cursor_before,
                    cursor_after: self.cursor,
                });
            } else {
                let deleted = self.buffer.text_between(prev_byte, idx);
                self.buffer.remove(prev_byte, idx);
                if self.cursor.col > 0 {
                    self.cursor.col -= 1;
                }

                self.history.push(Edit {
                    byte_idx: prev_byte,
                    deleted,
                    inserted: String::new(),
                    cursor_before,
                    cursor_after: self.cursor,
                });
            }
        }
    }

    fn delete(&mut self) {
        let cursor_before = self.cursor;
        let idx = self.cursor.byte_idx(&self.buffer);
        let total = self.buffer.len_bytes();
        if idx < total {
            let end = idx + 1;
            let deleted = self.buffer.text_between(idx, end);
            self.buffer.remove(idx, end);

            self.history.push(Edit {
                byte_idx: idx,
                deleted,
                inserted: String::new(),
                cursor_before,
                cursor_after: self.cursor,
            });
        }
    }

    fn system_copy(text: &str) {
        use std::process::{Command, Stdio};
        // Wayland
        if let Ok(mut child) = Command::new("wl-copy")
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
        {
            if let Some(ref mut stdin) = child.stdin {
                let _ = std::io::Write::write_all(stdin, text.as_bytes());
            }
            let _ = child.wait();
            return;
        }
        // X11
        if let Ok(mut child) = Command::new("xclip")
            .arg("-sel").arg("c").stdin(Stdio::piped())
            .stdout(Stdio::null()).stderr(Stdio::null()).spawn()
        {
            if let Some(ref mut stdin) = child.stdin {
                let _ = std::io::Write::write_all(stdin, text.as_bytes());
            }
            let _ = child.wait();
            return;
        }
        // macOS
        if let Ok(mut child) = Command::new("pbcopy")
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()
        {
            if let Some(ref mut stdin) = child.stdin {
                let _ = std::io::Write::write_all(stdin, text.as_bytes());
            }
            let _ = child.wait();
        }
    }

    fn system_paste() -> Option<String> {
        use std::process::{Command, Stdio};
        // Wayland
        if let Ok(out) = Command::new("wl-paste")
            .stdout(Stdio::piped()).stderr(Stdio::null()).output()
        {
            if out.status.success() {
                if let Ok(s) = String::from_utf8(out.stdout) {
                    if !s.is_empty() { return Some(s); }
                }
            }
        }
        // X11
        if let Ok(out) = Command::new("xclip")
            .arg("-sel").arg("c").arg("-o")
            .stdout(Stdio::piped()).stderr(Stdio::null()).output()
        {
            if out.status.success() {
                if let Ok(s) = String::from_utf8(out.stdout) {
                    if !s.is_empty() { return Some(s); }
                }
            }
        }
        // macOS
        if let Ok(out) = Command::new("pbpaste")
            .stdout(Stdio::piped()).stderr(Stdio::null()).output()
        {
            if out.status.success() {
                if let Ok(s) = String::from_utf8(out.stdout) {
                    if !s.is_empty() { return Some(s); }
                }
            }
        }
        None
    }

    fn copy_line(&mut self) {
        if self.cursor.has_selection() {
            if let Some(text) = self.cursor.selected_text(&self.buffer) {
                self.clipboard = text.clone();
                Self::system_copy(&text);
                self.set_message("Copied selection");
            }
        } else {
            let line = self.buffer.line(self.cursor.line);
            self.clipboard = line.clone();
            Self::system_copy(&line);
            self.set_message("Copied line");
        }
    }

    fn cut_selection_or_line(&mut self) {
        let cursor_before = self.cursor;
        if self.cursor.has_selection() {
            let (start, end) = self
                .cursor
                .selection_range(&self.buffer)
                .expect("has_selection was true");
            self.clipboard = self.buffer.text_between(start, end);
            Self::system_copy(&self.clipboard);
            self.buffer.remove(start, end);
            // Move cursor to start of selection
            self.cursor.line = self.buffer.byte_to_line(start);
            self.cursor.col = start - self.buffer.line_to_byte(self.cursor.line);
            self.ensure_cursor_visible();
            self.cursor.clear_selection();
            self.history.push(Edit {
                byte_idx: start,
                deleted: self.clipboard.clone(),
                inserted: String::new(),
                cursor_before,
                cursor_after: self.cursor,
            });
            self.set_message("Cut selection");
        } else {
            // Fall back to cutting the whole line
            let byte_start = self.buffer.line_to_byte(self.cursor.line);
            let line_len = self.buffer.line_len_bytes(self.cursor.line);
            let byte_end = byte_start + line_len;
            self.clipboard = self.buffer.text_between(byte_start, byte_end);
            Self::system_copy(&self.clipboard);
            if self.buffer.line_count() > 1 {
                self.buffer.remove(byte_start, byte_end);
                if self.cursor.line >= self.buffer.line_count() {
                    self.cursor.line = self.buffer.line_count().saturating_sub(1);
                }
                self.cursor.col = 0;
            } else {
                self.buffer.remove(0, self.buffer.len_bytes());
                self.cursor.col = 0;
            }
            self.ensure_cursor_visible();
            self.history.push(Edit {
                byte_idx: byte_start,
                deleted: self.clipboard.clone(),
                inserted: String::new(),
                cursor_before,
                cursor_after: self.cursor,
            });
            self.set_message("Cut line");
        }
    }

    fn select_all(&mut self) {
        let _total = self.buffer.len_bytes();
        self.cursor.selecting = true;
        self.cursor.anchor = Some((0, 0));
        self.cursor.line = self.buffer.line_count().saturating_sub(1);
        let last_line_len = self.buffer.line_len_bytes(self.cursor.line);
        self.cursor.col = last_line_len;
        self.ensure_cursor_visible();
        self.set_message("Selected all");
    }

    fn paste(&mut self) {
        // Try system clipboard first, fall back to internal buffer
        let text = Self::system_paste()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.clipboard.clone());
        if text.is_empty() {
            self.set_message("Nothing to paste");
            return;
        }
        self.clipboard = text.clone();
        let cursor_before = self.cursor;
        let idx = self.cursor.byte_idx(&self.buffer);
        self.buffer.insert(idx, &text);
        // Place cursor after pasted text
        let pasted = text.clone();
        // Find the end position — count newlines to move cursor
        let newline_count = pasted.chars().filter(|&c| c == '\n').count();
        if newline_count > 0 {
            self.cursor.line += newline_count;
            let last_line_start = pasted.rfind('\n').map(|i| i + 1).unwrap_or(0);
            self.cursor.col = pasted[last_line_start..].len();
        } else {
            self.cursor.col += pasted.len();
        }
        self.ensure_cursor_visible();

        self.history.push(Edit {
            byte_idx: idx,
            deleted: String::new(),
            inserted: pasted,
            cursor_before,
            cursor_after: self.cursor,
        });
        self.set_message("Pasted");
    }

    /// Called when the terminal emits a bracketed-paste event
    /// (e.g. Super+V in Alacritty/Omarchy).  Inserts the text as
    /// a single undoable edit, same as paste() but uses the
    /// event-provided text rather than self.clipboard.
    fn paste_received(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        let cursor_before = self.cursor;
        let idx = self.cursor.byte_idx(&self.buffer);
        self.buffer.insert(idx, &text);
        let newline_count = text.chars().filter(|&c| c == '\n').count();
        if newline_count > 0 {
            self.cursor.line += newline_count;
            let last_line_start = text.rfind('\n').map(|i| i + 1).unwrap_or(0);
            self.cursor.col = text[last_line_start..].len();
        } else {
            self.cursor.col += text.len();
        }
        self.ensure_cursor_visible();
        self.history.push(Edit {
            byte_idx: idx,
            deleted: String::new(),
            inserted: text,
            cursor_before,
            cursor_after: self.cursor,
        });
        self.set_message("Pasted");
    }

    fn build_async(&mut self) {
        self.build_dirty = false;
        let source = self.buffer.to_string();
        let (tx, rx) = mpsc::channel();
        self.build_rx = Some(rx);
        std::thread::spawn(move || {
            let result = latex::compile(&source);
            let _ = tx.send(result);
        });
        self.set_message("Building…");
    }

    fn check_zathura_alive(&mut self) {
        if let Some(ref mut child) = self.zathura {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited
                    self.zathura = None;
                    self.set_message("Preview closed");
                }
                _ => {} // Still running or error — leave as-is
            }
        }
    }

    fn poll_build(&mut self) {
        // Check if Zathura was closed externally
        self.check_zathura_alive();

        // Check auto-debounce
        if let Some(t) = self.auto_debounce {
            if t.elapsed() >= Duration::from_millis(800) {
                self.auto_debounce = None;
                self.build_async();
            }
        }

        // Check for async build completion
        if let Some(ref rx) = self.build_rx {
            if let Ok(result) = rx.try_recv() {
                self.build_rx = None;
                self.last_compile = Some(result.clone());

                if result.success {
                    let size = result.pdf_data.as_ref().map(|d| d.len()).unwrap_or(0);
                    self.set_message(&format!("Build OK ({} bytes)", size));
                    // Write PDF to disk
                    if let Some(ref pdf) = result.pdf_data {
                        let path = self.pdf_output_path();
                        let _ = std::fs::write(&path, pdf);
                    }
                    if self.open_zathura_on_complete {
                        self.open_zathura_on_complete = false;
                        self.open_zathura();
                    }
                    if self.build_dirty {
                        self.set_message("Build OK — edits made since build");
                    }
                } else {
                    let n = result.errors.len();
                    let first = &result.errors[0];
                    // Store full error messages for ErrorView
                    self.diagnostics = result.errors.iter().map(|e| {
                        format!("line {}: {}", e.line + 1, &e.message)
                    }).collect();
                    self.set_message(&format!(
                        "Build failed ({} error{}) — line {}: {}",
                        n,
                        if n == 1 { "" } else { "s" },
                        first.line + 1,
                        first.message.lines().next().unwrap_or(""),
                    ));
                    self.cursor.line = first.line;
                    self.cursor.col = 0;
                    self.ensure_cursor_visible();
                }
            }
        }
    }

    fn modified_since_build(&mut self) {
        self.auto_debounce = Some(Instant::now());
        self.build_dirty = true;
    }

    fn check_auto_save(&mut self) {
        let interval = self.config.auto_save_interval;
        if interval == 0 { return; }
        if !self.buffer.modified() { return; }
        if self.auto_save_timer.elapsed() < Duration::from_secs(interval) { return; }
        self.auto_save_timer = Instant::now();

        // Auto-save directly to the file on disk
        if let Some(path) = self.buffer.path().map(|p| p.to_path_buf()) {
            if self.buffer.save(&path).is_ok() {
                self.buffer.set_modified(false);
            }
        }
    }

    fn undo(&mut self) {
        if let Some(edit) = self.history.undo() {
            // Reverse: remove inserted, restore deleted
            let end = edit.byte_idx + edit.inserted.len();
            if !edit.inserted.is_empty() {
                self.buffer.remove(edit.byte_idx, end);
            }
            if !edit.deleted.is_empty() {
                self.buffer.insert(edit.byte_idx, &edit.deleted);
            }
            self.cursor = edit.cursor_before;
            self.ensure_cursor_visible();
            self.buffer.set_modified(true);
            self.set_message("Undo");
        } else {
            self.set_message("Nothing to undo");
        }
    }

    fn redo(&mut self) {
        if let Some(edit) = self.history.redo() {
            // Apply forward: remove deleted, restore inserted
            let end = edit.byte_idx + edit.deleted.len();
            if !edit.deleted.is_empty() {
                self.buffer.remove(edit.byte_idx, end);
            }
            if !edit.inserted.is_empty() {
                self.buffer.insert(edit.byte_idx, &edit.inserted);
            }
            self.cursor = edit.cursor_after;
            self.ensure_cursor_visible();
            self.buffer.set_modified(true);
            self.set_message("Redo");
        } else {
            self.set_message("Nothing to redo");
        }
    }

    fn current_line_indent(&self) -> String {
        let line = self.buffer.line(self.cursor.line);
        let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        indent
    }

    fn pdf_output_path(&self) -> std::path::PathBuf {
        if let Some(p) = self.buffer.path() {
            p.with_extension("pdf")
        } else {
            std::path::PathBuf::from("/tmp/muT_preview.pdf")
        }
    }

    fn open_zathura(&mut self) {
        use std::process::{Command, Stdio};
        if self.zathura.is_some() {
            return;
        }
        // Check if zathura is installed
        if Command::new("which").arg("zathura").stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() {
            self.set_message("Install zathura: https://pwmt.org/projects/zathura/download/");
            return;
        }
        let pdf = self.pdf_output_path();
        if !pdf.exists() {
            self.set_message("Build first — no PDF yet");
            return;
        }
        match Command::new("zathura").arg(&pdf).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(child) => {
                self.zathura = Some(child);
                self.set_message(&format!("Preview opened: {}", pdf.display()));
            }
            Err(e) => {
                self.set_message(&format!("Failed to open zathura: {}", e));
            }
        }
    }

    fn kill_zathura(&mut self) {
        if let Some(mut child) = self.zathura.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let vis_lines = self.editor_height();
        let total = self.buffer.line_count();

        // Vertical
        if self.cursor.line < self.scroll.line {
            self.scroll.line = self.cursor.line;
        }
        if total > vis_lines && self.cursor.line >= self.scroll.line + vis_lines {
            self.scroll.line = self.cursor.line.saturating_sub(vis_lines.saturating_sub(1));
        }
        if total > vis_lines {
            let max_s = total.saturating_sub(vis_lines);
            if self.scroll.line > max_s {
                self.scroll.line = max_s;
            }
        } else {
            self.scroll.line = 0;
        }

        // Horizontal
        let vis_cols = self.text_area_width();
        if self.cursor.col < self.scroll.col {
            self.scroll.col = self.cursor.col;
        }
        if self.cursor.col >= self.scroll.col + vis_cols {
            self.scroll.col = self.cursor.col.saturating_sub(vis_cols.saturating_sub(1));
        }
    }

    fn gutter_width(&self) -> u16 {
        let n = self.buffer.line_count().max(1);
        (n.ilog10() as u16) + 2
    }

    fn text_area_width(&self) -> usize {
        self.terminal_width
            .saturating_sub(self.gutter_width())
            .saturating_sub(1)
            .max(1) as usize
    }

    fn editor_height(&self) -> usize {
        let status_lines: u16 = if matches!(self.mode, Mode::Command { .. }) {
            2
        } else {
            1
        };
        self.terminal_height
            .saturating_sub(status_lines)
            .max(1) as usize
    }

    fn mouse_click(&mut self, col: u16, row: u16) {
        let gutter = self.gutter_width() as usize;
        let height = self.editor_height();

        // Most terminals (Kitty, Alacritty) send 0-indexed coordinates;
        // crossterm's docs claim 1-indexed.  Try 0-indexed first, fall
        // back to 1-indexed if the raw value would be out of bounds.
        let r = if (row as usize) < height {
            row as usize
        } else {
            row.saturating_sub(1) as usize
        };
        if r >= height {
            return;
        }

        let c = if (col as usize) > gutter {
            col as usize - gutter
        } else {
            col.saturating_sub(1).saturating_sub(gutter as u16) as usize
        };

        let buf_line = self.scroll.line + r;
        if buf_line >= self.buffer.line_count() {
            return;
        }
        let raw_col = self.scroll.col + c;
        let line_len = self.buffer.line_len_bytes(buf_line);
        let is_last = buf_line + 1 >= self.buffer.line_count();
        let limit = if is_last { line_len } else { line_len.saturating_sub(1) };
        self.cursor.clear_selection();
        self.cursor.line = buf_line;
        self.cursor.col = raw_col.min(limit);
        self.ensure_cursor_visible();
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message = msg.to_string();
        self.message_until = Instant::now() + Duration::from_secs(3);
    }

    fn cancel_message(&mut self) {
        self.message.clear();
        self.message_until = Instant::now();
    }
}
