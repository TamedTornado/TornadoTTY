#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!(
        "zentty-linux: Rust product terminal boundary is not implemented ({})",
        zentty_core::PRODUCT_NAME
    );
    ExitCode::from(78)
}
