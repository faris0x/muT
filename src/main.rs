use mu_t::app::App;
use std::path::PathBuf;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let args: Vec<String> = std::env::args().collect();

    // Parse CLI args for --help and --version
    if args.len() > 1 {
        match args[1].as_str() {
            "--help" | "-h" => {
                println!("Usage: muT [file]");
                println!();
                println!("A terminal-based LaTeX editor (μT)");
                println!();
                println!("Options:");
                println!("  -h, --help     Show this help message");
                println!("  -v, --version  Show version information");
                println!();
                println!("Keybindings: see man muT(1) or Ctrl+H within the editor");
                return Ok(());
            }
            "--version" | "-v" => {
                println!("muT {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {}
        }
    }

    let mut app = if args.len() > 1 {
        App::open_file(PathBuf::from(&args[1]))
    } else {
        App::new()
    };

    app.run()?;
    Ok(())
}
