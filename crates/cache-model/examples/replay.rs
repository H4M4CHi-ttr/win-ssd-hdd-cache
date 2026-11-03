#![forbid(unsafe_code)]
// One CLI implementation; the binary entry also enables real process tests.
#[path = "../src/bin/cache-replay/main.rs"]
mod entry;

fn main() -> std::process::ExitCode {
    entry::main()
}
