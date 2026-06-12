# μT (muT)

A terminal-based LaTeX editor with Micro-like editing, asynchronous `pdflatex` compilation, Zathura preview, syntax highlighting, undo/redo, selection, bracket auto-closing, and more.

## Dependencies

| Package | Required for |
|---------|-------------|
| `texlive-core` | `pdflatex` compilation |
| `zathura` + `zathura-pdf-poppler` | Preview window (`Ctrl+P`) |
| `rust` / `cargo` | Building from source |

## Installation

### AUR (Arch Linux)

```bash
yay -S mut
```

All dependencies are installed automatically.

### From source

```bash
git clone https://github.com/faris0x/muT.git
cd muT
make install
```

Ensure `~/.local/bin` is in your `PATH`.

### Cargo

```bash
cargo install --git https://github.com/faris0x/muT.git
```

## Quick start

```bash
muT                    # New file with LaTeX template
muT doc.tex            # Open existing file
```

| Key | Action |
|-----|--------|
| `Ctrl+S` | Save |
| `Ctrl+O` | Open `.tex` file |
| `Ctrl+N` | New file (LaTeX template) |
| `Ctrl+Q` | Quit |
| `Ctrl+Z` / `Ctrl+Y` | Undo / Redo |
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | Copy / Cut / Paste |
| `Ctrl+A` | Select all |
| `Ctrl+F` | Find |
| `Ctrl+G` | Go to line |
| `Ctrl+B` | Build (async `pdflatex`) |
| `Ctrl+P` | Toggle Zathura preview |
| `Ctrl+E` | View build errors |
| `Ctrl+H` | Keybinding reference |
| `Ctrl+Up` / `Ctrl+Down` | Scroll view |
| `Shift+Arrow` / `Home` / `End` / `PgUp` / `PgDn` | Extend selection |
| `Tab` | Accept ghost `\end{X}` or 4 spaces |

### Auto-close

| Type | Result |
|------|--------|
| `{` | `{}` with cursor between |
| `(` | `()` with cursor between |
| `$` | `$$` with cursor between |
| `\begin{X}` then `}` | Ghost `\end{X}` (Tab to accept) |

### Auto-build

The editor compiles via `pdflatex` 800ms after the last keystroke when modified. The PDF is written alongside the `.tex` file. Zathura auto-reloads it when open.

## Configuration

`~/.config/muT/config.toml`:

```toml
[theme]
name = "dark"                    # "dark" | "light"
syntax = "base16-ocean.dark"     # Syntect theme name or .tmTheme path

[editor]
tab_width = 4
auto_save_interval = 10          # seconds, 0 = off
```

Available syntax themes: `base16-ocean.dark`, `dracula`, `monokai`, `Solarized (dark)`, `Solarized (light)`, `InspiredGitHub`, `OneHalfDark`, `OneHalfLight`, and ~25 more from the Syntect library.

## License

MIT
