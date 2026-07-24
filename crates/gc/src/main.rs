//! gc — `git commit -m <msg>`. `-a` stages tracked changes first; `-p` pushes
//! after a successful commit (so `gc -a -p "msg"` is commit-everything-and-push).
use clap::{CommandFactory, Parser};

#[derive(Parser)]
#[command(name = "gc", version, about = "git commit -m; -a stages tracked changes first, -p pushes after")]
struct Cli {
    /// Stage all tracked changes first (git commit -a).
    #[arg(short, long)]
    all: bool,
    /// Push after a successful commit.
    #[arg(short, long)]
    push: bool,
    /// Print the man page (roff) and exit.
    #[arg(long, hide = true)]
    man: bool,
    /// Commit message (all words joined into one message).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    message: Vec<String>,
}

fn main() {
    grove_core::reset_sigpipe();
    let cli = Cli::parse();
    if cli.man {
        clap_mangen::Man::new(Cli::command()).render(&mut std::io::stdout()).ok();
        return;
    }
    if let Err(e) = grove_core::passthrough::commit(cli.all, cli.push, &cli.message) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
