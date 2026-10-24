use super::*;

#[test]
fn a_prompt_adds_the_model_row() {
    let note = annotation_section_height(AnnotationKind::Note);
    let prompt = annotation_section_height(AnnotationKind::Prompt);
    assert_eq!(prompt - note, ROW_ADVANCE);
    let mut rects = Vec::new();
    push_annotation_input_rects(&mut rects, AnnotationKind::Prompt, 0.0, 0.0, 200.0);
    let focuses: Vec<_> = rects.iter().map(|(focus, _)| *focus).collect();
    assert_eq!(
        focuses,
        vec![
            PropertyFocus::AnnotationContent,
            PropertyFocus::AnnotationModel
        ]
    );
}

#[test]
fn the_kind_row_has_one_cell_per_kind() {
    let mut out = Vec::new();
    push_annotation_action_rects(&mut out, 0.0, 0.0, 240.0);
    let kinds: Vec<_> = out
        .iter()
        .map(|(action, _)| match action {
            PropertyPanelAction::SetAnnotationKind(kind) => *kind,
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    assert_eq!(kinds, AnnotationKind::ALL.to_vec());
    assert!(out[0].1.origin.x < out[1].1.origin.x);
}

#[test]
fn the_summary_reads_content_and_model() {
    let node: PenNode = serde_json::from_str(
        r#"{"type":"prompt","id":"p","content":"Draft the hero","model":"m1"}"#,
    )
    .unwrap();
    let summary = annotation_summary_of(&node).unwrap();
    assert_eq!(summary.kind, AnnotationKind::Prompt);
    assert_eq!(summary.content, "Draft the hero");
    assert_eq!(summary.model, "m1");
    let text: PenNode = serde_json::from_str(r#"{"type":"text","id":"t","content":"x"}"#).unwrap();
    assert!(annotation_summary_of(&text).is_none());
}
