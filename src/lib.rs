//! swgp2-tool - save-file editor for Super Woden GP 2.
//!
//! Library crate shared by the two front-ends:
//!   `swgp2`     - graphical (egui) editor, just run it
//!   `swgp2-tui` - terminal (ratatui) editor, just run it

pub mod app;
pub mod crypto;
pub mod mcp;
pub mod nrbf;
pub mod save;
pub mod tool;
