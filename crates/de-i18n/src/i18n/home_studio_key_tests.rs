//! Home key coverage across every locale table.

/// Every locale table must carry a DIRECT entry for the Studio Home
/// keys — `translate`'s EN fallback must not mask a missing translation.
#[test]
fn studio_home_keys_exist_in_every_locale_table() {
    const KEYS: [&str; 64] = [
        "home.title",
        "home.nav.recents",
        "home.nav.openFile",
        "home.nav.templates",
        "home.nav.designSystems",
        "home.nav.sceneTemplates",
        "home.nav.settings",
        "home.top.openFile",
        "home.top.newFile",
        "home.kind.landing",
        "home.kind.mobile",
        "home.kind.webapp",
        "home.kind.slides",
        "home.kind.dashboard",
        "home.kind.other",
        "home.placeholder.landing",
        "home.placeholder.mobile",
        "home.placeholder.webapp",
        "home.placeholder.slides",
        "home.placeholder.dashboard",
        "home.placeholder.other",
        "home.size.desktop",
        "home.size.tablet",
        "home.size.mobile",
        "home.size.slides",
        "home.size.pageHeader",
        "home.size.slideHeader",
        "home.model.addProvider",
        "home.import.figma",
        "home.import.figmaSub",
        "home.import.web",
        "home.import.webSub",
        "home.import.screenshot",
        "home.import.screenshotSub",
        "home.import.screenshotPrompt",
        "home.recent.title",
        "home.recent.edited",
        "home.recent.name",
        "home.recent.folder",
        "home.recent.editedColumn",
        "home.recent.emptyTitle",
        "home.recent.emptyBody",
        "home.mcp.running",
        "home.mcp.stopped",
        "home.mcp.connected",
        "home.mcp.noAgent",
        "home.mcp.title",
        "home.mcp.intro",
        "home.mcp.stoppedIntro",
        "home.mcp.endpoint",
        "home.mcp.agent",
        "home.mcp.addTo",
        "home.mcp.writes",
        "home.mcp.setUp",
        "home.mcp.added",
        "home.mcp.remove",
        "home.mcp.manual",
        "home.mcp.copy",
        "home.mcp.copied",
        "home.mcp.connectedHeader",
        "home.mcp.emptyAgents",
        "home.mcp.port",
        "home.mcp.settings",
        "home.mcp.start",
    ];
    let keys: Vec<String> = KEYS.iter().map(|key| key.to_string()).collect();
    type Lookup = fn(&str) -> Option<&'static str>;
    let tables: [(&str, Lookup); 15] = [
        ("en", super::en::lookup),
        ("zh_cn", super::zh_cn::lookup),
        ("zh_tw", super::zh_tw::lookup),
        ("ja", super::ja::lookup),
        ("ko", super::ko::lookup),
        ("fr", super::fr::lookup),
        ("es", super::es::lookup),
        ("de", super::de::lookup),
        ("pt", super::pt::lookup),
        ("ru", super::ru::lookup),
        ("hi", super::hi::lookup),
        ("tr", super::tr::lookup),
        ("th", super::th::lookup),
        ("vi", super::vi::lookup),
        ("id", super::id::lookup),
    ];
    for key in &keys {
        for (name, lookup) in tables {
            assert!(
                lookup(key).is_some(),
                "locale `{name}` is missing direct Studio Home key `{key}`"
            );
        }
    }
}
