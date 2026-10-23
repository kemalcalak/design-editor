//! `load_fonts`: the agent's way to make Google Fonts families available
//! before it measures or screenshots text set in them. Writes never
//! download on their own; calling this tool is the agent's consent.

use std::collections::BTreeMap;
use std::time::Duration;

use de_mcp::{McpTool, ToolErrorCode, ToolOutcome};

use crate::font_host::{self, FontHost};

/// How long one call waits for its downloads before reporting the rest as
/// still `downloading`.
const LOAD_WAIT: Duration = Duration::from_secs(20);

/// Cap on families per call, so one call cannot start an unbounded number
/// of downloads.
const MAX_FAMILIES: usize = 16;

pub struct LoadFonts {
    host: Option<&'static dyn FontHost>,
}

pub fn load_fonts_snapshot() -> LoadFonts {
    LoadFonts {
        host: font_host::installed(),
    }
}

/// `families` as a JSON array string or a comma-separated list, trimmed,
/// without empties or case-insensitive duplicates.
fn parse_families(raw: &str) -> Vec<String> {
    let listed: Vec<String> = serde_json::from_str::<Vec<String>>(raw.trim())
        .unwrap_or_else(|_| raw.split(',').map(str::to_string).collect());
    let mut families: Vec<String> = Vec::new();
    for family in listed {
        let family = family.trim().trim_matches(|c| c == '\'' || c == '"').trim();
        if !family.is_empty() && !families.iter().any(|f| f.eq_ignore_ascii_case(family)) {
            families.push(family.to_string());
        }
    }
    families
}

impl McpTool for LoadFonts {
    fn name(&self) -> &str {
        "load_fonts"
    }

    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        let families = parse_families(args.get("families").map(String::as_str).unwrap_or(""));
        if families.is_empty() {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                "families: give one or more font family names, e.g. \"Montserrat, Space Mono\""
                    .into(),
            );
        }
        if families.len() > MAX_FAMILIES {
            return ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("families: at most {MAX_FAMILIES} per call"),
            );
        }
        let Some(host) = self.host else {
            return ToolOutcome::Err(
                ToolErrorCode::ToolFailed,
                "font downloads are only available in the desktop app".into(),
            );
        };
        let fonts = host.load(&families, LOAD_WAIT);
        match serde_json::to_string(&serde_json::json!({ "fonts": fonts })) {
            Ok(json) => ToolOutcome::OkJson(json),
            Err(error) => ToolOutcome::Err(
                ToolErrorCode::Internal,
                format!("serialize load_fonts result failed: {error}"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font_host::{FontIssue, FontLoadResult};
    use de_editor_core::EditorState;
    use std::sync::Mutex;

    struct FakeHost {
        asked: Mutex<Vec<String>>,
    }

    impl FontHost for FakeHost {
        fn load(&self, families: &[String], _wait: Duration) -> Vec<FontLoadResult> {
            self.asked.lock().unwrap().extend(families.iter().cloned());
            families
                .iter()
                .map(|family| FontLoadResult {
                    family: family.clone(),
                    status: if family == "Nope" {
                        "notOnGoogle"
                    } else {
                        "downloaded"
                    },
                    faces: (family != "Nope").then_some(4),
                    detail: None,
                })
                .collect()
        }

        fn font_issues(&self, _state: &EditorState) -> Vec<FontIssue> {
            Vec::new()
        }
    }

    fn call(host: Option<&'static dyn FontHost>, families: &str) -> ToolOutcome {
        let args = BTreeMap::from([("families".to_string(), families.to_string())]);
        LoadFonts { host }.call(&args)
    }

    #[test]
    fn families_parse_from_a_list_or_a_json_array() {
        assert_eq!(
            parse_families(" Montserrat, 'Space Mono',montserrat,, "),
            vec!["Montserrat", "Space Mono"]
        );
        assert_eq!(
            parse_families(r#"["Big Shoulders Display", "Inter"]"#),
            vec!["Big Shoulders Display", "Inter"]
        );
    }

    #[test]
    fn reports_each_family_from_the_host() {
        let host: &'static FakeHost = Box::leak(Box::new(FakeHost {
            asked: Mutex::new(Vec::new()),
        }));
        let ToolOutcome::OkJson(json) = call(Some(host), "Space Mono, Nope") else {
            panic!("expected JSON");
        };
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["fonts"][0]["status"], "downloaded");
        assert_eq!(value["fonts"][0]["faces"], 4);
        assert_eq!(value["fonts"][1]["status"], "notOnGoogle");
        assert_eq!(*host.asked.lock().unwrap(), vec!["Space Mono", "Nope"]);
    }

    #[test]
    fn refuses_without_families_or_without_a_font_host() {
        assert!(matches!(
            call(None, " , "),
            ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
        ));
        assert!(matches!(
            call(None, "Montserrat"),
            ToolOutcome::Err(ToolErrorCode::ToolFailed, _)
        ));
        let many = (0..=MAX_FAMILIES)
            .map(|i| format!("F{i}"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(matches!(
            call(None, &many),
            ToolOutcome::Err(ToolErrorCode::InvalidArgument, _)
        ));
    }
}
