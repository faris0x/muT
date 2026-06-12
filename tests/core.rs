use std::path::PathBuf;
use mu_t::buffer::Buffer;
use mu_t::cursor::Cursor;

#[test]
fn test_new_buffer_empty() {
    let buf = Buffer::new();
    assert!(buf.is_empty());
    assert_eq!(buf.line_count(), 1); // empty rope still has 1 line
    assert_eq!(buf.len_bytes(), 0);
    assert!(!buf.modified());
    assert_eq!(buf.path_or_untitled(), "untitled");
}

#[test]
fn test_load_and_save() {
    let dir = std::env::temp_dir().join("muT_test_save");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test_save.tex");

    // Create a test file
    std::fs::write(&path, "Hello\nWorld\n").unwrap();

    // Load it
    let buf = Buffer::load(&path).unwrap();
    assert!(!buf.is_empty());
    assert_eq!(buf.line_count(), 3); // "Hello\n", "World\n", ""
    assert_eq!(buf.line(0), "Hello");
    assert_eq!(buf.line(1), "World");
    assert_eq!(buf.line(2), "");
    assert!(!buf.modified());
    assert_eq!(buf.path_or_untitled(), "test_save.tex");

    // Clean up
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_buffer_line_operations() {
    let mut buf = Buffer::new();

    // Insert text
    buf.insert(0, "abc\ndef\nghi");
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.line(0), "abc");
    assert_eq!(buf.line(1), "def");
    assert_eq!(buf.line(2), "ghi");
    assert!(buf.modified());

    // Line lengths (including newline)
    assert_eq!(buf.line_len_bytes(0), 4); // "abc\n"
    assert_eq!(buf.line_len_bytes(1), 4); // "def\n"
    assert_eq!(buf.line_len_bytes(2), 3); // "ghi" (no trailing newline)
}

#[test]
fn test_buffer_insert_and_remove() {
    let mut buf = Buffer::new();
    buf.insert(0, "Hello World");
    assert_eq!(buf.line(0), "Hello World");

    buf.remove(5, 11);
    assert_eq!(buf.line(0), "Hello");
}

#[test]
fn test_cursor_new() {
    let buf = Buffer::new();
    let cur = Cursor::new();
    assert_eq!(cur.line, 0);
    assert_eq!(cur.col, 0);
    assert_eq!(cur.byte_idx(&buf), 0);
}

#[test]
fn test_cursor_movement() {
    let buf = Buffer::load_from_str("line1\nline2\nline3\n");
    let mut cur = Cursor::new();

    // Start at (0,0)
    assert_eq!(cur.line, 0);
    assert_eq!(cur.col, 0);

    // Move right
    cur.move_right(&buf, false);
    assert_eq!(cur.col, 1);
    assert_eq!(cur.byte_idx(&buf), 1);

    // Move to end of line (at the newline byte position = col 5 for "line1\n")
    cur.move_end(&buf, false);
    assert_eq!(cur.col, 5, "should be at newline position, the end of line");

    // Move right again should go to next line
    cur.move_right(&buf, false);
    assert_eq!(cur.line, 1);
    assert_eq!(cur.col, 0);

    // Move down
    cur.move_down(&buf, false);
    assert_eq!(cur.line, 2);

    // Move up
    cur.move_up(&buf, false);
    assert_eq!(cur.line, 1);

    // Move home
    cur.move_home(&buf, false);
    assert_eq!(cur.col, 0);

    // Move left from (1,0) should go to end of previous line (newline byte)
    cur.move_left(&buf, false);
    assert_eq!(cur.line, 0);
    assert_eq!(cur.col, 5, "should be at newline position of 'line1'");
}

#[test]
fn test_cursor_clamp() {
    // Should not crash when moving beyond buffer bounds
    let buf = Buffer::new();
    let mut cur = Cursor::new();
    cur.move_up(&buf, false); // no-op
    cur.move_down(&buf, false); // no-op
    cur.move_left(&buf, false); // no-op
    cur.move_right(&buf, false); // no-op
    assert_eq!(cur.line, 0);
    assert_eq!(cur.col, 0);
}

#[test]
fn test_line_to_byte_conversion() {
    let buf = Buffer::load_from_str("abc\ndef\nghi\n");
    assert_eq!(buf.line_to_byte(0), 0);
    assert_eq!(buf.line_to_byte(1), 4);
    assert_eq!(buf.line_to_byte(2), 8);
    assert_eq!(buf.line_to_byte(3), 12);
}

#[test]
fn test_byte_to_line_conversion() {
    let buf = Buffer::load_from_str("abc\ndef\nghi\n");
    assert_eq!(buf.byte_to_line(0), 0);
    assert_eq!(buf.byte_to_line(2), 0);
    assert_eq!(buf.byte_to_line(4), 1);
    assert_eq!(buf.byte_to_line(7), 1);
    assert_eq!(buf.byte_to_line(8), 2);
    assert_eq!(buf.byte_to_line(11), 2);
    assert_eq!(buf.byte_to_line(12), 3);
}

#[test]
fn test_text_between() {
    let buf = Buffer::load_from_str("Hello World");
    assert_eq!(buf.text_between(0, 5), "Hello");
    assert_eq!(buf.text_between(6, 11), "World");
}

#[test]
fn test_modified_flag() {
    let mut buf = Buffer::new();
    assert!(!buf.modified());
    buf.insert(0, "test");
    assert!(buf.modified());
    buf.set_modified(false);
    assert!(!buf.modified());
}

#[test]
fn test_path_functions() {
    let mut buf = Buffer::new();
    assert!(buf.path().is_none());
    assert_eq!(buf.path_or_untitled(), "untitled");

    buf.set_path(Some(PathBuf::from("/home/user/doc.tex")));
    assert!(buf.path().is_some());
    assert_eq!(buf.path_or_untitled(), "doc.tex");
}

#[test]
fn test_cursor_byte_idx() {
    let buf = Buffer::load_from_str("abc\ndef");
    let mut cur = Cursor::new();

    assert_eq!(cur.byte_idx(&buf), 0);

    cur.col = 2;
    assert_eq!(cur.byte_idx(&buf), 2);

    cur.line = 1;
    cur.col = 0;
    assert_eq!(cur.byte_idx(&buf), 4);

    cur.col = 2;
    assert_eq!(cur.byte_idx(&buf), 6);
}

#[test]
fn test_last_line_end_and_right_arrow() {
    // "line1\nline2" has 2 lines: "line1\n" and "line2" (last line, no \n)
    let buf = Buffer::load_from_str("line1\nline2");
    let mut cur = Cursor::new();

    // On last line "line2" (len=5), End should go to col=5 (one past end)
    cur.line = 1;
    cur.move_end(&buf, false);
    assert_eq!(cur.col, 5, "End on last line should be past the last visible char");

    // Right arrow at col=5 on last line should do nothing (no next line)
    cur.move_right(&buf, false);
    assert_eq!(cur.col, 5, "Right arrow at end of last line is a no-op");

    // Type a char — should appear after "line2"
    let idx = cur.byte_idx(&buf);
    assert_eq!(idx, buf.len_bytes());

    // Move left, then right — should return to col 5
    cur.move_left(&buf, false);
    assert_eq!(cur.col, 4);
    cur.move_right(&buf, false);
    assert_eq!(cur.col, 5, "Right arrow should restore position at end of last line");
}

#[test]
fn test_paste_undo_is_single_step() {
    // Simulate the paste + undo pattern used by the App.
    use mu_t::history::{Edit, History};
    use mu_t::cursor::Cursor;

    let mut buf = Buffer::new();
    let mut hist = History::new();

    // Start with some text
    buf.insert(0, "hello ");
    assert_eq!(buf.line(0), "hello ");

    // "Paste" multi-line content
    let pasted = "world\nof\nLaTeX";
    let cursor_before = Cursor { line: 0, col: 6, selecting: false, anchor: None };
    let mut cursor_after = cursor_before;
    let idx = cursor_before.byte_idx(&buf);
    buf.insert(idx, pasted);

    // Move cursor after paste (simulating what paste() does)
    let newline_count = pasted.chars().filter(|&c| c == '\n').count();
    cursor_after.line += newline_count;
    let last_line_start = pasted.rfind('\n').map(|i| i + 1).unwrap_or(0);
    cursor_after.col = pasted[last_line_start..].len();

    // Push as single edit (not coalesced with anything)
    hist.push(Edit {
        byte_idx: idx,
        deleted: String::new(),
        inserted: pasted.to_string(),
        cursor_before,
        cursor_after,
    });
    assert_eq!(buf.line(0), "hello world");
    assert_eq!(buf.line_count(), 3);

    // Undo should reverse the entire paste in one step
    let edit = hist.undo().expect("undo should return an edit");
    let end = edit.byte_idx + edit.inserted.len();
    buf.remove(edit.byte_idx, end);
    assert_eq!(buf.line(0), "hello ");
    assert_eq!(buf.line_count(), 1);
    // Next undo should return None (nothing more to undo)
    assert!(hist.undo().is_none(), "paste should be a single undo step");
}

#[test]
fn test_history_coalescing_does_not_merge_paste() {
    use mu_t::history::{Edit, History};
    use mu_t::cursor::Cursor;

    let mut hist = History::new();

    // Type a single char
    hist.push(Edit {
        byte_idx: 0, deleted: String::new(), inserted: "a".into(),
        cursor_before: Cursor::new(), cursor_after: Cursor { line: 0, col: 1, selecting: false, anchor: None },
    });

    // Paste multi-char content
    hist.push(Edit {
        byte_idx: 1, deleted: String::new(), inserted: "long paste".into(),
        cursor_before: Cursor { line: 0, col: 1, selecting: false, anchor: None }, cursor_after: Cursor { line: 0, col: 11, selecting: false, anchor: None },
    });

    // Undo should undo the paste (not coalesced with the "a")
    let e = hist.undo().expect("undo paste");
    assert_eq!(e.inserted, "long paste", "paste must be its own undo entry");
}

#[test]
fn test_latex_content() {
    let latex = r#"\documentclass{article}
\usepackage{amsmath}
\begin{document}
\section{Test}
$E = mc^2$
\end{document}
"#;
    let buf = Buffer::load_from_str(latex);
    assert!(buf.line_count() >= 6);
    assert_eq!(buf.line(0), r"\documentclass{article}");
    assert_eq!(buf.line(1), r"\usepackage{amsmath}");
    assert_eq!(buf.line(4), r"$E = mc^2$");
}
