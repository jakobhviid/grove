//! lt — self-contained tree view (no eza); git repos get a git icon.
use clap::{CommandFactory, Parser};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lt", version, about = "Tree view (dirs first, icons); git repos get a git icon")]
struct Cli {
    /// Directory to list (default: current directory).
    dir: Option<PathBuf>,
    /// How many levels deep to descend.
    #[arg(short, long, default_value_t = 2)]
    level: usize,
    /// Show hidden entries (dotfiles) too.
    #[arg(short, long)]
    all: bool,
    /// Print the man page (roff) and exit.
    #[arg(long, hide = true)]
    man: bool,
}

fn main() {
    grove_core::reset_sigpipe();
    let cli = Cli::parse();
    if cli.man {
        clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout()).ok();
        return;
    }
    if let Err(e) = grove_core::tree::run(cli.dir.as_deref(), cli.level, cli.all) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
