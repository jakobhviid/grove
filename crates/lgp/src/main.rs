//! lgp — auto pull/push the clean, in-sync repos in a folder, then show lg.
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lgp", about = "Auto pull/push the clean, in-sync repos in a folder, then show the overview")]
struct Cli {
    /// Folder of repositories (default: current directory).
    dir: Option<PathBuf>,
}

fn main() {
    grove_core::reset_sigpipe();
    let cli = Cli::parse();
    if let Err(e) = grove_core::sync::run(cli.dir.as_deref()) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
