use crate::test_support::rect;
use crate::{DocumentInstallError, EditorState, PreparedDocument};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::page::PenPage;
use jian_ops_schema::PenDocument;

fn page(id: &str, children: Vec<PenNode>) -> PenPage {
    PenPage {
        id: id.into(),
        name: id.into(),
        children,
        background_color: None,
        state: None,
        lifecycle: None,
    }
}

fn paged(pages: Vec<PenPage>) -> PenDocument {
    let mut doc = EditorState::new().doc;
    doc.pages = Some(pages);
    doc
}

fn doc_with_rect(id: &str, width: f64) -> PenDocument {
    serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{
            "type": "rectangle",
            "id": id,
            "name": "Rect",
            "x": 0,
            "y": 0,
            "width": width,
            "height": 10
        }]
    }))
    .expect("valid test document")
}

#[test]
fn preparing_a_document_runs_validation_before_any_state_is_touched() {
    let mut invalid = doc_with_rect("dupe", 10.0);
    invalid.children.push(invalid.children[0].clone());
    assert_eq!(
        PreparedDocument::prepare(invalid).err(),
        Some(DocumentInstallError::DuplicateId { id: "dupe".into() })
    );
    assert!(PreparedDocument::prepare(doc_with_rect("ok", 10.0)).is_ok());
}

#[test]
fn duplicate_ids_across_pages_are_rejected() {
    let invalid = paged(vec![page(
        "p1",
        vec![
            rect("duplicate", "A", 0.0, 0.0, 1.0, 1.0),
            rect("duplicate", "B", 0.0, 0.0, 1.0, 1.0),
        ],
    )]);
    assert_eq!(
        PreparedDocument::prepare(invalid).err(),
        Some(DocumentInstallError::DuplicateId {
            id: "duplicate".into(),
        })
    );
}

#[test]
fn explicit_pages_reject_root_content_and_empty_page_ids() {
    let mut mixed = paged(vec![page("p1", vec![])]);
    mixed.children = vec![rect("n1", "Root", 0.0, 0.0, 1.0, 1.0)];
    assert_eq!(
        PreparedDocument::prepare(mixed).err(),
        Some(DocumentInstallError::ExplicitPagesWithRootChildren)
    );

    let empty_page = paged(vec![page("", vec![])]);
    assert_eq!(
        PreparedDocument::prepare(empty_page).err(),
        Some(DocumentInstallError::EmptyPageId { page_index: 0 })
    );
}

#[test]
fn a_prepared_document_gives_back_exactly_what_was_validated() {
    let doc = doc_with_rect("kept", 12.0);
    let prepared = PreparedDocument::prepare(doc.clone()).expect("valid document");
    assert_eq!(prepared.document(), &doc);
    assert_eq!(prepared.into_document(), doc);
}
