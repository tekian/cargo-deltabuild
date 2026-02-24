#![doc(hidden)]

//! A legacy stub to point people to the real deal.

use std::process::exit;

fn main() {
    eprintln!("cargo-detlabuild is now cargo-delta! Please install cargo-delta and uninstall cargo-deltabuild.");
    exit(1);
}
