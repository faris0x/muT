use crate::buffer::Buffer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
    pub selecting: bool,
    pub anchor: Option<(usize, usize)>,
}

impl Cursor {
    pub fn new() -> Self {
        Cursor {
            line: 0,
            col: 0,
            selecting: false,
            anchor: None,
        }
    }

    pub fn byte_idx(&self, buffer: &Buffer) -> usize {
        buffer.line_to_byte(self.line) + self.col
    }

    /// Begin or continue a selection.  On first call in a selection
    /// gesture the anchor is fixed at the current cursor position.
    fn begin_select(&mut self) {
        if !self.selecting {
            self.selecting = true;
            self.anchor = Some((self.line, self.col));
        }
    }

    pub fn clear_selection(&mut self) {
        self.selecting = false;
        self.anchor = None;
    }

    pub fn has_selection(&self) -> bool {
        self.selecting
            && self.anchor.is_some()
            && self.anchor != Some((self.line, self.col))
    }

    /// Normalised byte range `(start, end)` of the selection, if any.
    pub fn selection_range(&self, buffer: &Buffer) -> Option<(usize, usize)> {
        let anchor = self.anchor?;
        if !self.selecting {
            return None;
        }
        let a = buffer.line_to_byte(anchor.0) + anchor.1;
        let b = buffer.line_to_byte(self.line) + self.col;
        if a <= b {
            Some((a, b))
        } else {
            Some((b, a))
        }
    }

    /// Extract the selected text, if any.
    pub fn selected_text<'a>(&self, buffer: &'a Buffer) -> Option<String> {
        let (start, end) = self.selection_range(buffer)?;
        if start == end {
            return None;
        }
        Some(buffer.text_between(start, end))
    }

    // ── movement with selection support ──────────────────────────

    pub fn move_up(&mut self, buffer: &Buffer, select: bool) {
        if select {
            self.begin_select();
        } else {
            self.clear_selection();
        }
        if self.line > 0 {
            self.line -= 1;
            self.clamp_col(buffer);
        }
    }

    pub fn move_down(&mut self, buffer: &Buffer, select: bool) {
        if select {
            self.begin_select();
        } else {
            self.clear_selection();
        }
        if self.line + 1 < buffer.line_count() {
            self.line += 1;
            self.clamp_col(buffer);
        }
    }

    pub fn move_left(&mut self, buffer: &Buffer, select: bool) {
        if select {
            self.begin_select();
        } else {
            self.clear_selection();
        }
        if self.col > 0 {
            self.col -= 1;
        } else if self.line > 0 {
            self.line -= 1;
            let len = buffer.line_len_bytes(self.line);
            self.col = if len > 1 { len - 1 } else { 0 };
        }
    }

    pub fn move_right(&mut self, buffer: &Buffer, select: bool) {
        if select {
            self.begin_select();
        } else {
            self.clear_selection();
        }
        let line_len = buffer.line_len_bytes(self.line);
        let is_last = self.line + 1 >= buffer.line_count();
        let limit = if is_last {
            line_len
        } else {
            line_len.saturating_sub(1)
        };
        if self.col < limit {
            self.col += 1;
        } else if self.line + 1 < buffer.line_count() {
            self.line += 1;
            self.col = 0;
        }
    }

    pub fn move_home(&mut self, _buffer: &Buffer, select: bool) {
        if select {
            self.begin_select();
        } else {
            self.clear_selection();
        }
        self.col = 0;
    }

    pub fn move_end(&mut self, buffer: &Buffer, select: bool) {
        if select {
            self.begin_select();
        } else {
            self.clear_selection();
        }
        let len = buffer.line_len_bytes(self.line);
        if self.line + 1 < buffer.line_count() {
            self.col = len.saturating_sub(1);
        } else {
            self.col = len;
        }
    }

    pub fn move_page_up(&mut self, buffer: &Buffer, view_height: usize, select: bool) {
        if select {
            self.begin_select();
        } else {
            self.clear_selection();
        }
        for _ in 0..view_height.saturating_sub(1) {
            if self.line == 0 {
                break;
            }
            self.line -= 1;
        }
        self.clamp_col(buffer);
    }

    pub fn move_page_down(&mut self, buffer: &Buffer, view_height: usize, select: bool) {
        if select {
            self.begin_select();
        } else {
            self.clear_selection();
        }
        let max_line = buffer.line_count().saturating_sub(1);
        for _ in 0..view_height.saturating_sub(1) {
            if self.line >= max_line {
                break;
            }
            self.line += 1;
        }
        self.clamp_col(buffer);
    }

    // ── helpers ──────────────────────────────────────────────────

    fn clamp_col(&mut self, buffer: &Buffer) {
        let max_col = buffer.line_len_bytes(self.line);
        let is_last = self.line + 1 >= buffer.line_count();
        let limit = if is_last {
            max_col
        } else {
            max_col.saturating_sub(1)
        };
        if self.col > limit {
            self.col = limit;
        }
    }
}
