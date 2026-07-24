//! lt — self-contained tree view (no eza); git repos get a git icon.
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lt", about = "Tree view (dirs first, icons); git repos get a git icon")]
struct Cli {
    /// Directory to list (default: current directory).
    dir: Option<PathBuf>,
    /// How many levels deep to descend.
    #[arg(short, long, default_value_t = 2)]
    level: usize,
    /// Show hidden entries (dotfiles) too.
    #[arg(short, long)]
    all: bool,
}

fn main() {
    grove_core::reset_sigpipe();
    let cli = Cli::parse();
    if let Err(e) = grove_core::tree::run(cli.dir.as_deref(), cli.level, cli.all) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
