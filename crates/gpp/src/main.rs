//! gpp — `git push`. Forwards any extra args straight to git.
fn main() {
    grove_core::reset_sigpipe();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if grove_core::maybe_version("gpp", &args) {
        return;
    }
    if let Err(e) = grove_core::passthrough::push(&args) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
