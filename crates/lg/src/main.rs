//! lg — multi-repo dashboard for a folder of repos.
use clap::{CommandFactory, Parser};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lg", version, about = "Multi-repo dashboard: branch, ahead/behind, and dirty state per repo")]
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
    if let Err(e) = grove_core::overview::run(cli.dir.as_deref()) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
