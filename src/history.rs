use crate::cursor::Cursor;

/// A single reversible edit operation.
#[derive(Debug, Clone)]
pub struct Edit {
    /// Byte index where the edit occurred.
    pub byte_idx: usize,
    /// Text that was removed (empty for pure insertions).
    pub deleted: String,
    /// Text that was inserted (empty for pure deletions).
    pub inserted: String,
    /// Cursor state before the edit.
    pub cursor_before: Cursor,
    /// Cursor state after the edit.
    pub cursor_after: Cursor,
}

/// Simple undo/redo stack with coalescing of adjacent character insertions.
#[derive(Debug)]
pub struct History {
    /// Past edits (index `pos` is the next one to undo).
    stack: Vec<Edit>,
    /// Position in the stack. `stack[..pos]` is the undo history,
    /// `stack[pos..]` is the redo history.
    pos: usize,
    /// Maximum number of entries before oldest entries are dropped.
    max_len: usize,
}

impl History {
    pub fn new() -> Self {
        History {
            stack: Vec::new(),
            pos: 0,
            max_len: 1000,
        }
    }

    /// Push a new edit, discarding any redo history ahead of `pos`.
    pub fn push(&mut self, edit: Edit) {
        // Discard redo entries
        self.stack.truncate(self.pos);

        // Coalesce consecutive single-character insertions at adjacent positions
        if let Some(last) = self.stack.last_mut() {
            let is_single_insert = edit.deleted.is_empty() && edit.inserted.len() <= 1;
            let last_was_single_insert =
                last.deleted.is_empty() && last.inserted.len() <= 1;
            let is_adjacent = last.byte_idx + last.inserted.len() == edit.byte_idx;
            if is_single_insert && last_was_single_insert && is_adjacent {
                last.inserted.push_str(&edit.inserted);
                last.cursor_after = edit.cursor_after;
                return;
            }
        }

        self.stack.push(edit);
        self.pos += 1;

        // Drop oldest if over limit
        if self.stack.len() > self.max_len {
            self.stack.remove(0);
            self.pos -= 1;
        }
    }

    /// Undo the most recent edit. Returns `(Edit, reverse)` or `None`.
    pub fn undo(&mut self) -> Option<Edit> {
        if self.pos == 0 {
            return None;
        }
        self.pos -= 1;
        let edit = self.stack[self.pos].clone();
        Some(edit)
    }

    /// Redo the last undone edit. Returns `(Edit, forward)` or `None`.
    pub fn redo(&mut self) -> Option<Edit> {
        if self.pos >= self.stack.len() {
            return None;
        }
        let edit = self.stack[self.pos].clone();
        self.pos += 1;
        Some(edit)
    }

    /// Whether there are any undos available.
    pub fn can_undo(&self) -> bool {
        self.pos > 0
    }

    /// Whether there are any redos available.
    pub fn can_redo(&self) -> bool {
        self.pos < self.stack.len()
    }

    /// Clear all history (e.g. after opening a new file).
    pub fn clear(&mut self) {
        self.stack.clear();
        self.pos = 0;
    }
}
