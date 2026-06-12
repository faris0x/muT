use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Holds the loaded syntax definitions and colour theme used to highlight.
pub struct Highlighter {
    pub syntax_set: SyntaxSet,
    pub theme: Theme,
    /// The ".tex" syntax reference (cloned cheaply — it's an `Arc`).
    tex_syntax: syntect::parsing::SyntaxReference,
    /// Cached fallback plain-text syntax.
    plain_syntax: syntect::parsing::SyntaxReference,
}

impl Highlighter {
    pub fn new() -> Self {
        Self::with_theme("base16-ocean.dark")
    }

    /// Create a highlighter using a named syntect theme or a `.tmTheme` path.
    pub fn with_theme(name_or_path: &str) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme = ThemeSet::load_defaults()
            .themes
            .remove(name_or_path)
            .or_else(|| {
                // Try loading as a .tmTheme file
                let p = std::path::Path::new(name_or_path);
                if p.extension().and_then(|e| e.to_str()) == Some("tmTheme") {
                    ThemeSet::get_theme(p).ok()
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                ThemeSet::load_defaults()
                    .themes
                    .into_values()
                    .next()
                    .unwrap()
            });

        let tex_syntax = syntax_set
            .find_syntax_by_extension("tex")
            .cloned()
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text().clone());

        let plain_syntax = syntax_set.find_syntax_plain_text().clone();

        Highlighter {
            syntax_set,
            theme,
            tex_syntax,
            plain_syntax,
        }
    }

    /// Highlight the visible portion of a buffer.
    ///
    /// `lines` is a slice of the raw line strings (without trailing newlines).
    /// Returns a vector parallel to `lines`, each entry being a list of
    /// `(ratatui_style, text)` segments.
    pub fn highlight_lines(
        &self,
        lines: &[String],
        is_latex: bool,
    ) -> Vec<Vec<(ratatui::style::Style, String)>> {
        let syntax = if is_latex {
            &self.tex_syntax
        } else {
            &self.plain_syntax
        };

        // Re-join with newlines so syntect sees the correct line boundaries.
        let mut joined = String::new();
        for (i, l) in lines.iter().enumerate() {
            joined.push_str(l);
            if i + 1 < lines.len() {
                joined.push('\n');
            }
        }

        let mut hl = HighlightLines::new(syntax, &self.theme);
        let mut result = Vec::with_capacity(lines.len());

        for line in LinesWithEndings::from(&joined) {
            let ranges = hl
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();

            let spans: Vec<(ratatui::style::Style, String)> = ranges
                .into_iter()
                .map(|(style, text)| {
                    let mut s = ratatui::style::Style::default();
                    // Foreground
                    s = s.fg(into_colour(style.foreground));
                    // Background (only if non-transparent)
                    if style.background.a != 0 {
                        s = s.bg(into_colour(style.background));
                    }
                    // Font style
                    if style.font_style.contains(syntect::highlighting::FontStyle::BOLD) {
                        s = s.add_modifier(ratatui::style::Modifier::BOLD);
                    }
                    if style.font_style.contains(syntect::highlighting::FontStyle::ITALIC) {
                        s = s.add_modifier(ratatui::style::Modifier::ITALIC);
                    }
                    if style.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE) {
                        s = s.add_modifier(ratatui::style::Modifier::UNDERLINED);
                    }
                    (s, text.to_string())
                })
                .collect();

            result.push(spans);
        }

        result
    }
}

fn into_colour(c: syntect::highlighting::Color) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(c.r, c.g, c.b)
}
