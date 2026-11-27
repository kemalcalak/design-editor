//! Design Editor headless host services — the GUI-free host backend.
//!
//! Everything the host does that doesn't need a window: the stdio + HTTP
//! MCP servers and the live MCP endpoint, the AI chat providers + design
//! orchestration, server-side PNG/SVG/PDF export (skia raster),
//! document/settings persistence, and model/provider discovery. It links NO
//! winit / glutin / muda / accesskit adapters / skia-GL — the desktop GUI
//! stack stays in `de-host-native` / `de-host-desktop`, which embeds this
//! crate for its MCP / chat / export.
//!
//! Modules were migrated here out of `de-host-desktop` over Phases 2–5 of
//! the extraction.

// Migrated headless modules (Phases 2-5), kept alphabetical.
pub mod acp_agent_probe_host;
#[cfg(test)]
mod boolean_export_tests;
pub mod brand_catalog;
pub mod builtin_model_discovery;
pub mod chat_agent_loop;
pub mod chat_attachment;
#[cfg(test)]
mod chat_avatar_modify_tests;
pub mod chat_builtin_http;
mod chat_builtin_http_wire;
pub mod chat_canvas_tools;
pub mod chat_claude;
pub mod chat_copilot;
pub mod chat_grok_stream;
pub mod chat_http_server;
pub mod chat_intent;
pub mod chat_mention_context;
mod chat_mention_context_error;
pub mod chat_mention_prefetch;
pub mod chat_mention_scan;
pub mod chat_provider_llm;
pub mod chat_runtime;
pub mod chat_spawn;
pub mod chat_subprocess;
mod chat_subprocess_antigravity_log;
mod chat_subprocess_claude_stream;
mod chat_subprocess_dsh;
mod chat_subprocess_exit;
mod chat_subprocess_lifecycle;
mod chat_subprocess_parse;
pub mod chat_subprocess_quirks;
mod chat_subprocess_safety;
pub mod chat_system_prompt;
pub(crate) mod chat_text_excerpt;
mod claude_artifact;
pub mod cli_model_discovery;
pub mod cli_modes;
pub mod cli_probe_error;
mod cli_probe_support;
pub mod cli_provider_probe;
mod cli_resolver_windows;
pub mod clipboard_export;
pub mod clipboard_export_error;
mod copilot_sdk_probe;
mod deck_motion;
#[cfg(test)]
mod design_agent_reflow_tests;
pub mod design_agent_run;
mod design_agent_run_error;
mod design_agent_run_executor;
#[cfg(test)]
mod design_agent_tool_result_tests;
pub mod design_agent_tools;
pub mod design_context;
pub(crate) mod design_md_evidence;
mod design_md_evidence_appendix;
mod design_md_evidence_error;
mod design_md_evidence_normalize;
mod design_md_evidence_roles;
pub mod design_md_llm;
pub mod design_md_llm_error;
pub mod design_session;
pub mod doc_io;
pub mod export;
pub mod export_batch;
#[cfg(test)]
mod export_deck_roundtrip_tests;
pub mod export_html;
mod export_html_motion;
mod export_html_structured;
mod export_html_template;
pub mod export_hyperframes;
pub mod export_pdf;
pub mod export_pptx;
pub mod export_svg;
pub mod font_host;
mod font_host_error;
pub mod generate_jobs;
pub mod google_fonts;
pub mod google_fonts_error;
pub mod headless_export;
mod headless_export_error;
pub mod home_preview;
pub mod image_relevance_judge;
mod import_html_url;
pub mod import_html_url_error;
pub mod local_image_reader;
pub mod loop_blocker_ledger;
pub mod mcp_live;
pub mod mcp_serve;
pub mod model_catalog_refresh;
pub mod model_discovery;
mod model_probe;
pub mod pre_validator;
mod provider_dial;
pub mod provider_probe;
pub mod provider_probe_host;
pub mod provider_probe_models;
pub mod quality_credential;
pub mod reference_context;
pub mod reference_context_error;
#[cfg(test)]
pub(crate) mod test_support;
// Settings persistence moved to de-editor-host-core (feature `settings-io`)
// so every host shares the exact desktop load/save path; these
// re-exports keep every `de_host_services::settings_io*` import stable.
pub use de_editor_host_core::settings_io;
pub use de_editor_host_core::settings_io_error;
mod test_config_root;
pub mod user_scene_template_store;
pub mod validation_providers;
pub mod web_credentials;
pub mod web_credentials_error;
pub mod web_image_generate;
pub mod web_image_search;
