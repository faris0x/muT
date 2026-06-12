use std::path::PathBuf;

use ratatui::style::Color;
use serde::Deserialize;

/// Chrome colours that the user can theme independently of the syntax scheme.
#[derive(Debug, Clone)]
pub struct Theme {
    pub status_bg: Color,
    pub status_fg: Color,
    pub line_num_fg: Color,
    pub line_num_bg: Color,
    pub gutter_bg: Color,
    pub cursor_line_bg: Color,
    pub selection_bg: Color,
    pub command_bg: Color,
    pub command_fg: Color,
    pub modified_fg: Color,
    pub ok_fg: Color,
    pub err_fg: Color,
    pub building_fg: Color,
    pub ghost_fg: Color,
}

impl Theme {
    pub fn dark() -> Self {
        Theme {
            status_bg:     Color::Rgb(30, 30, 30),
            status_fg:     Color::Rgb(200, 200, 200),
            line_num_fg:   Color::Rgb(100, 100, 100),
            line_num_bg:   Color::Rgb(20, 20, 20),
            gutter_bg:     Color::Rgb(20, 20, 20),
            cursor_line_bg: Color::Rgb(30, 30, 40),
            selection_bg:  Color::Rgb(50, 60, 100),
            command_bg:    Color::Rgb(20, 20, 30),
            command_fg:    Color::Rgb(180, 220, 255),
            modified_fg:   Color::Rgb(255, 180, 60),
            ok_fg:         Color::Rgb(80, 220, 80),
            err_fg:        Color::Rgb(255, 120, 80),
            building_fg:   Color::Rgb(255, 200, 80),
            ghost_fg:      Color::Rgb(80, 80, 100),
        }
    }

    pub fn light() -> Self {
        Theme {
            status_bg:     Color::Rgb(220, 220, 220),
            status_fg:     Color::Rgb(30, 30, 30),
            line_num_fg:   Color::Rgb(140, 140, 140),
            line_num_bg:   Color::Rgb(240, 240, 240),
            gutter_bg:     Color::Rgb(240, 240, 240),
            cursor_line_bg: Color::Rgb(230, 230, 240),
            selection_bg:  Color::Rgb(180, 190, 220),
            command_bg:    Color::Rgb(240, 240, 250),
            command_fg:    Color::Rgb(20, 60, 120),
            modified_fg:   Color::Rgb(200, 120, 0),
            ok_fg:         Color::Rgb(0, 140, 0),
            err_fg:        Color::Rgb(200, 40, 40),
            building_fg:   Color::Rgb(180, 120, 0),
            ghost_fg:      Color::Rgb(160, 160, 180),
        }
    }
}

/// Raw TOML model — chrome overrides only (optional).
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ConfigToml {
    pub theme: ThemeToml,
    pub editor: EditorToml,
}

impl Default for ConfigToml {
    fn default() -> Self {
        ConfigToml {
            theme: ThemeToml::default(),
            editor: EditorToml::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ThemeToml {
    pub name: String,
    pub syntax: String,
    pub status_bg: Option<String>,
    pub status_fg: Option<String>,
    pub line_num_fg: Option<String>,
    pub line_num_bg: Option<String>,
    pub cursor_line_bg: Option<String>,
    pub selection_bg: Option<String>,
    pub command_bg: Option<String>,
    pub command_fg: Option<String>,
    pub modified_fg: Option<String>,
    pub ok_fg: Option<String>,
    pub err_fg: Option<String>,
    pub building_fg: Option<String>,
    pub ghost_fg: Option<String>,
}

impl Default for ThemeToml {
    fn default() -> Self {
        ThemeToml {
            name: "dark".into(),
            syntax: "base16-ocean.dark".into(),
            status_bg: None,
            status_fg: None,
            line_num_fg: None,
            line_num_bg: None,
            cursor_line_bg: None,
            selection_bg: None,
            command_bg: None,
            command_fg: None,
            modified_fg: None,
            ok_fg: None,
            err_fg: None,
            building_fg: None,
            ghost_fg: None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct EditorToml {
    pub tab_width: usize,
    pub auto_save_interval: u64, // seconds, 0 = off
}

impl Default for EditorToml {
    fn default() -> Self {
        EditorToml {
            tab_width: 4,
            auto_save_interval: 10,
        }
    }
}

/// Validated config used by the editor at runtime.
#[derive(Debug, Clone)]
pub struct Config {
    pub theme: Theme,
    pub syntax: String,
    pub tab_width: usize,
    pub auto_save_interval: u64,
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        let raw: ConfigToml = if let Ok(s) = std::fs::read_to_string(&path) {
            toml::from_str(&s).unwrap_or_default()
        } else {
            // Write default config so the user has something to edit
            let default = r#"[theme]
name = "dark"
syntax = "base16-ocean.dark"

[editor]
tab_width = 4
auto_save_interval = 10
"#;
            let _ = std::fs::write(&path, default);
            ConfigToml::default()
        };

        let mut theme = match raw.theme.name.as_str() {
            "light" => Theme::light(),
            _ => Theme::dark(),
        };

        // Apply optional chrome overrides
        macro_rules! apply {
            ($field:ident) => {
                if let Some(ref v) = raw.theme.$field {
                    theme.$field = parse_color(v).unwrap_or(theme.$field);
                }
            };
        }
        apply!(status_bg);
        apply!(status_fg);
        apply!(line_num_fg);
        apply!(line_num_bg);
        apply!(cursor_line_bg);
        apply!(selection_bg);
        apply!(command_bg);
        apply!(command_fg);
        apply!(modified_fg);
        apply!(ok_fg);
        apply!(err_fg);
        apply!(building_fg);
        apply!(ghost_fg);

        Config {
            theme,
            syntax: raw.theme.syntax,
            tab_width: raw.editor.tab_width,
            auto_save_interval: raw.editor.auto_save_interval,
        }
    }
}

fn config_path() -> PathBuf {
    let mut p = if let Ok(d) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(d)
    } else if let Ok(d) = std::env::var("HOME") {
        PathBuf::from(d).join(".config")
    } else {
        PathBuf::from(".")
    };
    p = p.join("muT");
    let _ = std::fs::create_dir_all(&p);
    p.join("config.toml")
}

fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim_start_matches('#');
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(Color::Rgb(r, g, b))
    } else {
        None
    }
}
