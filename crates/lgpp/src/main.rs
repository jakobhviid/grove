//! lgpp — push every repo in a folder that has unpushed commits (no pull).
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lgpp", version, about = "Push every repo in a folder that has unpushed commits (no pull)")]
struct Cli {
    /// Folder of repositories (default: current directory).
    dir: Option<PathBuf>,
}

fn main() {
    grove_core::reset_sigpipe();
    let cli = Cli::parse();
    if let Err(e) = grove_core::sync::push_all(cli.dir.as_deref()) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
