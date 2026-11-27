//! Markdown parsing for the Design-MD panel now lives in jian
//! `components::markdown` (block structure + inline `**bold**` / `` `code` `` /
//! `#HEX` styling). Body lines wrap at their painted widths in
//! `design_md_wrap` (#451), not jian's character-budget `wrap_runs`.
//! Re-exported here so the panel's renderer keeps importing it from this
//! path; the panel owns the block painting + scroll layout.

pub(crate) use jian_widgets::components::markdown::{parse_blocks, parse_inline, MdBlock, MdRun};
