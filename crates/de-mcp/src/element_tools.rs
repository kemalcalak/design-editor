//! UIKit element tools — one `insert_<comp>` MCP tool per built-in
//! [`UIKit`] component, so an LLM client can drop a Primary Button
//! (etc.) onto the canvas without first having to learn the kit-id
//! / component-id pair. The TS counterpart is `pen-mcp`'s ~100
//! `add_card_v0` / `add_toast_v0` element tools.
//!
//! Each tool returns `ToolOutcome::OkWithCommand(map,
//! EditorCommand::InstantiateKitComponent { … })` so the host's
//! applier runs the same `EditorState::instantiate_kit_component`
//! path the Component-Browser panel uses: a linked instance of a master
//! installed once on the `Components` page (#365), selected, one undo
//! step; `detach: true` drops an independent copy instead.

use std::collections::BTreeMap;

use de_editor_core::uikit::KitComponent;
use de_editor_core::{EditorCommand, EditorState, NodeId, UIKit};

use super::{McpTool, ToolErrorCode, ToolOutcome};

/// Sanitize a `kit-id` / `component-id` for embedding in a tool name
/// — MCP tool names are `[a-zA-Z0-9_]+`, so dashes become underscores.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Name prefix shared by every kit-component tool; hosts use it to
/// decide whether a direct call needs the kit tools built.
pub const ELEMENT_TOOL_PREFIX: &str = "insert_";

/// The MCP tool name for a kit component: `insert_<comp_sanitized>`.
/// v1 ships one starter kit so the kit prefix is dropped for terseness;
/// a future imported-kits surface can fold the kit id back in once
/// collisions are possible.
pub fn element_tool_name(component_id: &str) -> String {
    format!("{ELEMENT_TOOL_PREFIX}{}", sanitize(component_id))
}

/// `insert_<comp>` MCP tool — instantiates one UIKit component onto
/// the active page through the editor command bus.
pub struct InsertKitComponent {
    name: String,
    kit_id: String,
    component_id: String,
    /// Only a frame template can become a master; any other kit
    /// component (the starter divider is a rectangle) is always copied.
    linkable: bool,
}

impl InsertKitComponent {
    pub fn new(kit_id: impl Into<String>, component_id: impl Into<String>) -> Self {
        let component_id = component_id.into();
        Self {
            name: element_tool_name(&component_id),
            kit_id: kit_id.into(),
            component_id,
            linkable: true,
        }
    }
}

impl McpTool for InsertKitComponent {
    fn name(&self) -> &str {
        &self.name
    }
    fn call(&self, args: &BTreeMap<String, String>) -> ToolOutcome {
        // `x` / `y` are optional doc-px floats; omitted slots default
        // to 0.0 at apply time.
        let doc_x = match parse_optional_f64(args, "x") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let doc_y = match parse_optional_f64(args, "y") {
            Ok(v) => v,
            Err(e) => return e,
        };
        let detach = match args.get("detach").map(String::as_str) {
            None | Some("") | Some("false") => false,
            Some("true") => true,
            Some(other) => {
                return ToolOutcome::Err(
                    ToolErrorCode::InvalidArgument,
                    format!("detach must be true or false, got {other}"),
                )
            }
        };
        let mut result = BTreeMap::new();
        result.insert("kit_id".into(), self.kit_id.clone());
        result.insert("component_id".into(), self.component_id.clone());
        result.insert("linked".into(), (!detach && self.linkable).to_string());
        ToolOutcome::OkWithCommand(
            result,
            EditorCommand::InstantiateKitComponent {
                kit_id: self.kit_id.as_str().into(),
                component_id: self.component_id.as_str().into(),
                doc_x,
                doc_y,
                target_parent: NodeId::NONE,
                page_id: None,
                overrides_json: None,
                detach,
            },
        )
    }
}

/// Parse a number arg as `Option<f64>`. An absent slot returns `None`
/// (the command falls back to 0.0); a malformed slot is a hard error
/// so the LLM client retries with a valid value.
///
/// The `Err` variant carries a full `ToolOutcome::Err` — the call site
/// returns it verbatim, so the large size is intentional.
#[allow(clippy::result_large_err)]
fn parse_optional_f64(
    args: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<f64>, ToolOutcome> {
    match args.get(key) {
        None => Ok(None),
        Some(v) if v.is_empty() => Ok(None),
        Some(v) => v.parse::<f64>().map(Some).map_err(|_| {
            ToolOutcome::Err(
                ToolErrorCode::InvalidArgument,
                format!("{key} must be a number"),
            )
        }),
    }
}

/// Walk every loaded kit and emit one [`InsertKitComponent`] per
/// component. The host's `rebuild_registry` chains this into the live
/// `ToolRegistry`.
pub fn insert_kit_component_tools(state: &EditorState) -> Vec<InsertKitComponent> {
    kit_components(&state.ui_kits)
        .map(|c| InsertKitComponent {
            linkable: c.linkable,
            ..InsertKitComponent::new(c.kit_id, c.id)
        })
        .collect()
}

/// JSON-encoded `tools/list` schema for one element tool. The host
/// concatenates this into the `tools/list` response next to the static
/// `TOOL_SCHEMAS`.
pub fn element_tool_schema(component_name: &str, component_id: &str) -> String {
    let tool = element_tool_name(component_id);
    canonical_element_tool_schema(&tool, component_name, true)
}

/// Whether a kit component inserts as a linked instance (its template can
/// be a master).
fn kit_component_linkable(component: &KitComponent) -> bool {
    matches!(component.template, jian_ops_schema::node::PenNode::Frame(_))
}

fn canonical_element_tool_schema(tool: &str, component_name: &str, linkable: bool) -> String {
    let how = if linkable {
        "as a linked instance: the first use installs the component as a master on the hidden Components page, every use is a ref to it, so editing that master changes every instance (the new instance is selected; get_selection gives its id). detach: true inserts an independent copy instead."
    } else {
        "as an independent copy (this component is not a frame, so it cannot be a master)."
    };
    format!(
        r#"{{"name":"{tool}","description":"Insert a {component_name} from the built-in UIKit onto the active page {how} Optional x/y doc-px floats place the top-left; defaults to (0, 0). See get_guidelines topic components.","inputSchema":{{"type":"object","properties":{{"x":{{"type":"string","description":"top-left doc-px (float)"}},"y":{{"type":"string","description":"top-left doc-px (float)"}},"detach":{{"type":"boolean","description":"insert an independent copy instead of a linked instance (default false)"}}}}}}}}"#
    )
}

/// JSON-encoded schemas for every element tool the live state has —
/// matches the iterator order of [`insert_kit_component_tools`] so
/// counts agree.
pub fn element_tool_schemas(state: &EditorState) -> Vec<String> {
    KitToolCatalog::of(&state.ui_kits).schemas()
}

/// What the element tools of a kit set are built from: each component's
/// `(id, name)`, in [`insert_kit_component_tools`] order. It lets a host
/// describe the kit tools without holding an [`EditorState`] (#406): the
/// live MCP server keeps the catalog its UI thread last published and
/// answers `tools/list` from it, never waiting for a document snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KitToolCatalog {
    /// `(id, name, linkable)` per component.
    components: Vec<(String, String, bool)>,
}

impl KitToolCatalog {
    pub fn of(kits: &[UIKit]) -> Self {
        Self {
            components: kit_components(kits)
                .map(|c| (c.id.to_string(), c.name.to_string(), c.linkable))
                .collect(),
        }
    }

    /// The catalog of the kits every [`EditorState`] starts with.
    pub fn builtin() -> &'static KitToolCatalog {
        static BUILTIN: std::sync::OnceLock<KitToolCatalog> = std::sync::OnceLock::new();
        BUILTIN.get_or_init(|| KitToolCatalog::of(&de_editor_core::uikit::builtin_kits()))
    }

    /// Whether `kits` would build this catalog, without allocating, so a
    /// host can check it on every frame and republish only on a change.
    pub fn matches(&self, kits: &[UIKit]) -> bool {
        let mut ours = self.components.iter();
        kit_components(kits).all(|c| {
            ours.next().is_some_and(|(our_id, our_name, our_linkable)| {
                our_id == c.id && our_name == c.name && *our_linkable == c.linkable
            })
        }) && ours.next().is_none()
    }

    /// One JSON schema per element tool, as [`element_tool_schemas`].
    pub fn schemas(&self) -> Vec<String> {
        self.components
            .iter()
            .map(|(id, name, linkable)| {
                canonical_element_tool_schema(&element_tool_name(id), name, *linkable)
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

/// One kit component as its element tool sees it.
struct KitComponentRef<'a> {
    kit_id: &'a str,
    id: &'a str,
    name: &'a str,
    /// Whether it inserts as a linked instance (#365).
    linkable: bool,
}

/// Every component of `kits` in tool order: the one walk both the tools
/// ([`insert_kit_component_tools`]) and their schemas ([`KitToolCatalog`])
/// take, so what `tools/list` describes is what dispatch accepts.
fn kit_components(kits: &[UIKit]) -> impl Iterator<Item = KitComponentRef<'_>> {
    kits.iter().flat_map(|kit| {
        kit.components.iter().map(|c| KitComponentRef {
            kit_id: &kit.id,
            id: &c.id,
            name: &c.name,
            linkable: kit_component_linkable(c),
        })
    })
}

#[cfg(test)]
mod catalog_tests {
    use super::*;

    #[test]
    fn the_catalog_matches_the_kits_it_was_built_from() {
        let mut kits = de_editor_core::builtin_kits();
        let catalog = KitToolCatalog::of(&kits);
        assert!(catalog.matches(&kits));
        assert_eq!(&catalog, KitToolCatalog::builtin());
        // Schemas and tools come out in the same order.
        let tool_names: Vec<String> = insert_kit_component_tools(&EditorState::new())
            .iter()
            .map(|tool| tool.name().to_string())
            .collect();
        let schema_names: Vec<String> = catalog
            .schemas()
            .iter()
            .map(|schema| {
                let value: serde_json::Value = serde_json::from_str(schema).expect("schema JSON");
                value["name"].as_str().expect("name").to_string()
            })
            .collect();
        assert_eq!(tool_names, schema_names);

        kits[0].components[0].name = "Renamed".into();
        assert!(
            !catalog.matches(&kits),
            "a renamed component changes its schema"
        );
        kits.pop();
        assert!(!catalog.matches(&kits), "fewer components");
        assert!(!catalog.matches(&[]), "no kits");
    }
}
