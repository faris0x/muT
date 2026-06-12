use ropey::{Rope, LineType};
use std::fs;
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::path::PathBuf;

const LT: LineType = LineType::LF_CR;

pub struct Buffer {
    rope: Rope,
    path: Option<PathBuf>,
    modified: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Buffer {
            rope: Rope::new(),
            path: None,
            modified: false,
        }
    }

    pub fn load<P: AsRef<Path>>(path: P) -> color_eyre::Result<Self> {
        let path = path.as_ref();
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        let rope = Rope::from_reader(reader)?;
        Ok(Buffer {
            rope,
            path: Some(path.to_path_buf()),
            modified: false,
        })
    }

    pub fn load_from_str(s: &str) -> Self {
        Buffer {
            rope: Rope::from_str(s),
            path: None,
            modified: false,
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> color_eyre::Result<()> {
        let file = fs::File::create(path.as_ref())?;
        let writer = BufWriter::new(file);
        self.rope.write_to(writer)?;
        Ok(())
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines(LT)
    }

    pub fn line(&self, idx: usize) -> String {
        if idx >= self.line_count() {
            return String::new();
        }
        // line returns RopeSlice which may include trailing newline
        let slice = self.rope.line(idx, LT);
        let s = slice.to_string();
        // Strip trailing newline/cr for display
        s.trim_end_matches(&['\n', '\r'][..]).to_string()
    }

    pub fn line_len_bytes(&self, idx: usize) -> usize {
        if idx >= self.line_count() {
            return 0;
        }
        let start = self.rope.line_to_byte_idx(idx, LT);
        let end = self.rope.line_to_byte_idx(idx + 1, LT);
        end.saturating_sub(start)
    }

    pub fn line_to_byte(&self, idx: usize) -> usize {
        if idx >= self.line_count() {
            return self.rope.len();
        }
        self.rope.line_to_byte_idx(idx, LT)
    }

    pub fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_line_idx(byte_idx, LT)
    }

    pub fn insert(&mut self, byte_idx: usize, text: &str) {
        self.rope.insert(byte_idx, text);
        self.modified = true;
    }

    pub fn remove(&mut self, start: usize, end: usize) {
        self.rope.remove(start..end);
        self.modified = true;
    }

    pub fn len_bytes(&self) -> usize {
        self.rope.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.rope.len() == 0
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn set_modified(&mut self, val: bool) {
        self.modified = val;
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    pub fn path_or_untitled(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string()
    }

    pub fn text_between(&self, start: usize, end: usize) -> String {
        self.rope.slice(start..end).to_string()
    }

    /// Return the entire text content of the buffer.
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }
}
