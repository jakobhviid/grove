//! ga — `git add`. Stages `.` when given no paths, else the paths you pass.
fn main() {
    grove_core::reset_sigpipe();
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = grove_core::passthrough::add(&paths) {
        grove_core::ui::err(&e.to_string());
        std::process::exit(1);
    }
}
