#![doc(hidden)]

//! A cargo tool to detect impacted crates from git changes.

fn main() {
    cargo_deltabuild_lib::run(std::env::args());
}
