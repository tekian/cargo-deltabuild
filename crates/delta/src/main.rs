#![doc(hidden)]

//! A cargo tool to detect impacted crates from git changes.

fn main() {
    lib::run(std::env::args());
}
