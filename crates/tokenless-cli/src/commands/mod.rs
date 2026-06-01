//! Command handler modules for the tokenless CLI.
//!
//! Each module exports handler functions that are dispatched from `main.rs`.

pub(crate) mod compress;
pub(crate) mod demo;
pub(crate) mod env_check_cmd;
pub(crate) mod hook;
pub(crate) mod init_cmd;
pub(crate) mod mcp_cmd;
pub(crate) mod rewrite;
pub(crate) mod stats;
pub(crate) mod toon;
pub(crate) mod tui;
