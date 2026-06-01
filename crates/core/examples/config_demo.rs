//! Example: basic `Config` usage.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use tokenless_core::Config;

fn main() {
    let config = Config::new("tokenless")
        .expect("valid name")
        .with_description("LLM token optimization toolkit");

    println!("Config name: {}", config.name());
    if let Some(desc) = config.description() {
        println!("Description: {desc}");
    }
}
