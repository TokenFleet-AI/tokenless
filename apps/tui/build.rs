//! Build script: embeds compile-time metadata via the `built` crate.

#![allow(clippy::expect_used)]

fn main() {
    built::write_built_file().expect("failed to write build metadata");
}
