//! lgpp — push every repo in a folder that has unpushed commits (no pull).
use clap::{CommandFactory, Parser};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lgpp", version, about = "Push every repo in a folder that has unpushed commits (no pull)")]
struct Cli {
    /// Folder of repositories (default: current directory).
    dir: Option<PathBuf>,
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
    if let Err(e) = grove_core::sync::push_all(cli.dir.as_deref()) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
