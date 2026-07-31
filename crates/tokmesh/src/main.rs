//! Thin install wrapper so `cargo install tokmesh` gets the `tokmesh` binary.
//!
//! Full implementation lives in `tokmesh-cli`; this crate exists so the short
//! crates.io / cargo package name installs the same CLI.
fn main() -> anyhow::Result<()> {
    tokmesh_cli::run()
}
