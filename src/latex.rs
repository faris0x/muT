use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static BUILD_ID: AtomicU32 = AtomicU32::new(0);

/// A single LaTeX compilation error with its originating line.
#[derive(Debug, Clone)]
pub struct CompileError {
    /// 0-indexed line number in the source.
    pub line: usize,
    /// Human-readable error message.
    pub message: String,
}

/// Outcome of a single LaTeX build.
#[derive(Debug, Clone)]
pub struct CompileResult {
    pub success: bool,
    /// The generated PDF bytes (present only on success).
    pub pdf_data: Option<Vec<u8>>,
    /// Parsed errors (non-empty only on failure).
    pub errors: Vec<CompileError>,
}

/// Compile a LaTeX document using the system `pdflatex`.
pub fn compile(source: &str) -> CompileResult {
    // Ensure pdflatex exists
    if Command::new("which")
        .arg("pdflatex")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_err()
    {
        return err_result("pdflatex not found — install TeX Live");
    }

    // Unique build directory so parallel calls don't collide
    let id = BUILD_ID.fetch_add(1, Ordering::SeqCst);
    let dir = PathBuf::from(format!("/tmp/muT_build_{}", id));
    let _ = std::fs::create_dir_all(&dir);
    let tex_path = dir.join("src.tex");
    let pdf_path = dir.join("src.pdf");
    let log_path = dir.join("src.log");

    // Write source
    if std::fs::write(&tex_path, source).is_err() {
        return err_result("failed to write temp source file");
    }

    // Run pdflatex
    let output = Command::new("pdflatex")
        .args(["-interaction=nonstopmode", "-output-directory", &dir.to_string_lossy(), &tex_path.to_string_lossy()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();

    if let Err(e) = output {
        return err_result(&format!("pdflatex failed: {}", e));
    }

    // Read log and check for errors
    let log_text = std::fs::read_to_string(&log_path).unwrap_or_default();
    let log_errors = parse_log(&log_text);
    let log_has_errors = !log_errors.is_empty();

    // Read PDF (may exist even on error with nonstopmode)
    let pdf_data = std::fs::read(&pdf_path).ok();

    if pdf_data.is_some() && !log_has_errors {
        return CompileResult {
            success: true,
            pdf_data,
            errors: Vec::new(),
        };
    }
    if pdf_data.is_some() {
        return CompileResult {
            success: false,
            pdf_data,
            errors: log_errors,
        };
    }

    // No PDF — parse log for errors
    if let Ok(log_text) = std::fs::read_to_string(&log_path) {
        let errors = parse_log(&log_text);
        if !errors.is_empty() {
            return CompileResult {
                success: false,
                pdf_data: None,
                errors,
            };
        }
        return CompileResult {
            success: false,
            pdf_data: None,
            errors: vec![CompileError {
                line: 0,
                message: log_text,
            }],
        };
    }

    err_result("pdflatex produced no output and no log")
}

/// Parse a LaTeX `.log` file for structured errors.
fn parse_log(log: &str) -> Vec<CompileError> {
    let mut errors: Vec<CompileError> = Vec::new();
    let mut cur_line: Option<usize> = None;
    let mut cur_msg = String::new();

    for line in log.lines() {
        if line.starts_with('!') {
            if !cur_msg.is_empty() {
                errors.push(CompileError {
                    line: cur_line.unwrap_or(0),
                    message: cur_msg.trim().to_string(),
                });
            }
            cur_msg = line.to_string();
            cur_line = None;
        } else if let Some(rest) = line.trim().strip_prefix("l.") {
            let num_str = rest.split_whitespace().next().unwrap_or("");
            if let Ok(n) = num_str.parse::<usize>() {
                cur_line = Some(n.saturating_sub(1));
            }
            cur_msg.push('\n');
            cur_msg.push_str(line.trim());
        } else if !cur_msg.is_empty() {
            cur_msg.push('\n');
            cur_msg.push_str(line.trim());
        }
    }

    if !cur_msg.is_empty() {
        errors.push(CompileError {
            line: cur_line.unwrap_or(0),
            message: cur_msg.trim().to_string(),
        });
    }

    errors
}

fn err_result(msg: &str) -> CompileResult {
    CompileResult {
        success: false,
        pdf_data: None,
        errors: vec![CompileError { line: 0, message: msg.into() }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_latex() {
        let src = r#"\documentclass{article}
\begin{document}
Hello
\end{document}"#;
        let result = compile(src);
        assert!(result.success, "expected success, got errors: {:?}", result.errors);
        assert!(result.pdf_data.is_some());
        assert!(result.pdf_data.unwrap().len() > 100);
    }

    #[test]
    fn test_invalid_latex() {
        let src = r#"\documentclass{article}
\begin{document}
\undefinedcommand
\end{document}"#;
        let result = compile(src);
        assert!(!result.success, "expected failure, got success");
        assert!(!result.errors.is_empty(), "expected errors, got none");
    }
}
