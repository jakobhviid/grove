//! lgp — auto pull/push the clean, in-sync repos in a folder, then show lg.
use clap::{CommandFactory, Parser};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lgp", version, about = "Auto pull/push the clean, in-sync repos in a folder, then show the overview")]
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
    if let Err(e) = grove_core::sync::run(cli.dir.as_deref()) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
