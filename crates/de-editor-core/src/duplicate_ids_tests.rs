use super::*;
use crate::id_allocator::SequentialIdAllocator;
use crate::ref_resolve::resolve_refs_for_canvas;
use crate::{EditorCommand, NodeId};
use serde_json::json;

fn doc(value: Value) -> PenDocument {
    jian_ops_schema::load_str(&value.to_string())
        .expect("fixture parses")
        .value
}

fn pages(pages: Vec<Value>) -> PenDocument {
    doc(json!({"version":"1.0.0","children":[],"pages":pages}))
}

fn page(id: &str, name: &str, children: Value) -> Value {
    json!({"id":id,"name":name,"children":children})
}

/// What the canvas draws, without ids (the repair changes ids only).
fn scene(doc: &PenDocument) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                map.remove("id");
                map.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(resolve_refs_for_canvas(doc)).expect("json");
    strip(&mut value);
    value
}

fn repaired(doc: &PenDocument) -> (PenDocument, Vec<RenamedId>) {
    let mut allocator = SequentialIdAllocator::for_document(doc, 1).expect("allocator");
    let repaired = repaired_document(doc, &mut allocator)
        .expect("repairs")
        .expect("has duplicates");
    (repaired.doc, repaired.renamed)
}

/// Three pages: Home has a hero and a section; Library has a card component
/// and an instance whose slot holds a frame `n5` and whose title is patched;
/// Other reuses `sec` and `title` from the other pages and `dup` twice.
fn three_pages() -> PenDocument {
    pages(vec![
        page(
            "p1",
            "Home",
            json!([
                {"type":"frame","id":"n5","name":"Hero","width":100,"height":50,
                 "events":{"onTap":[{"scroll_to":{"target":"sec"}}]}},
                {"type":"frame","id":"sec","name":"Section","y":60,"width":100,"height":50}
            ]),
        ),
        page(
            "p2",
            "Library",
            json!([
                {"type":"frame","id":"card","name":"Card","reusable":true,"width":200,"height":120,
                 "children":[
                    {"type":"text","id":"title","name":"Title","content":"Hello"},
                    {"type":"frame","id":"slot","name":"Slot","width":200,"height":80}]},
                {"type":"ref","id":"inst","ref":"card","x":300,"descendants":{
                    "title":{"content":"Patched"},
                    "slot":{"children":[{"type":"frame","id":"n5","name":"Owned",
                        "width":20,"height":20}]}}}
            ]),
        ),
        page(
            "p3",
            "Other",
            json!([
                {"type":"frame","id":"sec","name":"Copy","width":10,"height":10},
                {"type":"rectangle","id":"dup","name":"A","width":10,"height":10},
                {"type":"rectangle","id":"dup","name":"B","x":20,"width":10,"height":10},
                {"type":"text","id":"title","name":"Caption","content":"Other"}
            ]),
        ),
    ])
}

#[test]
fn duplicates_across_pages_and_inside_instances_are_reported() {
    let reports = find_duplicate_ids(&three_pages());
    let summary: Vec<(&str, usize, Vec<&str>)> = reports
        .iter()
        .map(|report| {
            let pages = report.pages.iter().map(String::as_str).collect();
            (report.id.as_str(), report.count, pages)
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("n5", 2, vec!["Home", "Library"]),
            ("sec", 2, vec!["Home", "Other"]),
            ("title", 2, vec!["Library", "Other"]),
            ("dup", 2, vec!["Other"]),
        ]
    );
    assert_eq!(reports[0].names, vec!["Hero", "Owned"]);
    assert_eq!(reports[0].to_string(), "n5 ×2 (Home, Library): Hero, Owned");
}

#[test]
fn repair_keeps_first_uses_and_leaves_the_canvas_unchanged() {
    let doc = three_pages();
    let (fixed, renamed) = repaired(&doc);
    assert!(find_duplicate_ids(&fixed).is_empty());
    let moved: Vec<(&str, &str, &str)> = renamed
        .iter()
        .map(|r| (r.old_id.as_str(), r.page.as_str(), r.name.as_str()))
        .collect();
    assert_eq!(
        moved,
        vec![
            ("n5", "Library", "Owned"),
            ("sec", "Other", "Copy"),
            ("dup", "Other", "B"),
            ("title", "Other", "Caption"),
        ]
    );
    assert!(renamed.iter().all(|r| r.new_id.starts_with('n')));
    assert_eq!(scene(&fixed), scene(&doc));
    // Home still holds `sec`, so its section link is left alone.
    let hero = serde_json::to_value(&fixed.pages.as_ref().unwrap()[0].children[0]).unwrap();
    assert_eq!(
        hero.pointer("/events/onTap/0/scroll_to/target"),
        Some(&json!("sec"))
    );
}

fn ref_target(doc: &PenDocument, page: usize, index: usize) -> String {
    match &doc.pages.as_ref().unwrap()[page].children[index] {
        PenNode::Ref(instance) => instance.target.clone(),
        other => panic!("instance expected, got {other:?}"),
    }
}

#[test]
fn a_component_master_keeps_its_id_over_instance_content() {
    // The first use of `c1` is instance content on page 1, but `c1` is also
    // a component that an instance on page 2 renders: the content moves.
    let doc = pages(vec![
        page(
            "p1",
            "One",
            json!([
                {"type":"frame","id":"box","name":"Box","reusable":true,"width":50,"height":50},
                {"type":"ref","id":"i1","ref":"box","descendants":{
                    "box":{"children":[{"type":"rectangle","id":"c1","width":5,"height":5}]}}}
            ]),
        ),
        page(
            "p2",
            "Two",
            json!([
                {"type":"frame","id":"c1","name":"Chip","reusable":true,"width":40,"height":20,
                 "children":[{"type":"text","id":"chip-label","content":"Chip"}]},
                {"type":"ref","id":"i2","ref":"c1","x":100}
            ]),
        ),
    ]);
    let (fixed, renamed) = repaired(&doc);
    assert_eq!(renamed.len(), 1);
    assert_eq!(renamed[0].page, "One");
    assert_eq!(ref_target(&fixed, 1, 1), "c1");
    assert_eq!(scene(&fixed), scene(&doc));
}

#[test]
fn a_component_master_keeps_its_id_over_an_earlier_plain_node() {
    let doc = pages(vec![
        page(
            "p1",
            "One",
            json!([{"type":"frame","id":"chip","name":"Plain","width":10,"height":10}]),
        ),
        page(
            "p2",
            "Two",
            json!([
                {"type":"frame","id":"chip","name":"Chip","reusable":true,"width":40,"height":20},
                {"type":"ref","id":"i2","ref":"chip","x":100}
            ]),
        ),
    ]);
    let (fixed, renamed) = repaired(&doc);
    assert_eq!(renamed.len(), 1);
    assert_eq!(
        (renamed[0].page.as_str(), renamed[0].name.as_str()),
        ("One", "Plain")
    );
    assert_eq!(ref_target(&fixed, 1, 1), "chip");
}

#[test]
fn instance_overrides_follow_a_renamed_master_node() {
    let doc = pages(vec![
        page(
            "p1",
            "One",
            json!([{"type":"text","id":"lbl","name":"Loose","content":"First"}]),
        ),
        page(
            "p2",
            "Two",
            json!([
                {"type":"frame","id":"card","reusable":true,"width":100,"height":40,
                 "children":[{"type":"text","id":"lbl","name":"Label","content":"Master"}]},
                {"type":"ref","id":"inst","ref":"card","x":200,"descendants":{
                    "lbl":{"content":"Patched"}}},
                {"type":"ref","id":"swap","ref":"card","x":400,"descendants":{
                    "lbl":{"type":"text","id":"lbl","content":"Swapped"}}}
            ]),
        ),
    ]);
    let (fixed, renamed) = repaired(&doc);
    assert_eq!(renamed.len(), 1);
    assert!(find_duplicate_ids(&fixed).is_empty());
    let new_id = &renamed[0].new_id;
    for index in [1, 2] {
        let PenNode::Ref(instance) = &fixed.pages.as_ref().unwrap()[1].children[index] else {
            panic!("instance");
        };
        let descendants = instance.descendants.as_ref().unwrap();
        assert_eq!(descendants.keys().collect::<Vec<_>>(), vec![new_id]);
        if index == 2 {
            assert_eq!(
                descendants[new_id]["id"],
                json!(new_id),
                "keeps the slot id"
            );
        }
    }
    assert_eq!(scene(&fixed), scene(&doc));
}

#[test]
fn a_nested_path_follows_a_node_renamed_inside_the_nested_component() {
    // `toolbar` nests a button instance `save`; the instance of the toolbar
    // patches the button's `label`, which is also a loose node's id.
    let doc = pages(vec![
        page(
            "p1",
            "One",
            json!([{"type":"text","id":"label","name":"Loose","content":"First"}]),
        ),
        page(
            "p2",
            "Two",
            json!([
                {"type":"frame","id":"button","reusable":true,"width":80,"height":30,
                 "children":[{"type":"text","id":"label","content":"OK"}]},
                {"type":"frame","id":"toolbar","reusable":true,"width":200,"height":40,
                 "children":[{"type":"ref","id":"save","ref":"button"}]},
                {"type":"ref","id":"bar","ref":"toolbar","y":100,"descendants":{
                    "save/label":{"content":"Store"}}}
            ]),
        ),
    ]);
    let (fixed, renamed) = repaired(&doc);
    assert_eq!(renamed.len(), 1);
    let PenNode::Ref(bar) = &fixed.pages.as_ref().unwrap()[1].children[2] else {
        panic!("instance");
    };
    let keys: Vec<&String> = bar.descendants.as_ref().unwrap().keys().collect();
    assert_eq!(keys, vec![&format!("save/{}", renamed[0].new_id)]);
    assert_eq!(scene(&fixed), scene(&doc));
}

#[test]
fn a_section_link_follows_the_only_copy_on_its_page() {
    let doc = pages(vec![
        page(
            "p1",
            "One",
            json!([{"type":"frame","id":"s","width":10,"height":10}]),
        ),
        page(
            "p2",
            "Two",
            json!([
                {"type":"frame","id":"s","name":"Pricing","width":10,"height":10},
                {"type":"frame","id":"btn","width":10,"height":10,
                 "events":{"onTap":[{"scroll_to":{"target":"s","alignment":"start"}}]}}
            ]),
        ),
    ]);
    let (fixed, renamed) = repaired(&doc);
    let button = serde_json::to_value(&fixed.pages.as_ref().unwrap()[1].children[1]).unwrap();
    assert_eq!(
        button.pointer("/events/onTap/0/scroll_to/target"),
        Some(&json!(renamed[0].new_id))
    );
}

#[test]
fn repair_command_is_one_undo_step() {
    let mut state = EditorState::from_document(three_pages());
    state.set_single_selection(NodeId::new("n5"));
    assert!(state.apply(EditorCommand::RepairDuplicateIds));
    assert!(state.duplicate_ids().is_empty());
    assert!(state.selection.set.is_empty());
    assert!(
        !state.apply(EditorCommand::RepairDuplicateIds),
        "nothing left"
    );
    assert!(state.apply(EditorCommand::Undo));
    assert_eq!(state.duplicate_ids().len(), 4);
}

#[test]
fn a_document_without_duplicates_needs_no_repair() {
    let doc = pages(vec![page(
        "p1",
        "One",
        json!([{"type":"frame","id":"a","width":10,"height":10}]),
    )]);
    let mut allocator = SequentialIdAllocator::new(1);
    assert!(repaired_document(&doc, &mut allocator)
        .expect("runs")
        .is_none());
}
