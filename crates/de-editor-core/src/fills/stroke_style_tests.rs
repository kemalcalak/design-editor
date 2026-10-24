use super::*;

fn rect(stroke: &str) -> PenNode {
    let src = format!(r#"{{"type":"rectangle","id":"r1","width":10,"height":10{stroke}}}"#);
    serde_json::from_str(&src).expect("rect fixture")
}

#[test]
fn missing_align_reads_center_and_no_stroke_reads_none() {
    assert_eq!(node_stroke_align(&rect("")), None);
    let stroked = rect(r#","stroke":{"thickness":2}"#);
    assert_eq!(node_stroke_align(&stroked), Some(StrokeAlign::Center));
}

#[test]
fn set_stroke_align_writes_existing_strokes_only() {
    let mut bare = rect("");
    assert!(!set_stroke_align(&mut bare, StrokeAlign::Inside));
    assert_eq!(node_stroke_align(&bare), None);

    let mut stroked = rect(r#","stroke":{"thickness":2}"#);
    assert!(!set_stroke_align(&mut stroked, StrokeAlign::Center));
    assert!(set_stroke_align(&mut stroked, StrokeAlign::Outside));
    assert_eq!(node_stroke_align(&stroked), Some(StrokeAlign::Outside));
    assert!(!set_stroke_align(&mut stroked, StrokeAlign::Outside));
}

#[test]
fn style_reads_authored_fields_and_absent_stays_none() {
    assert_eq!(node_stroke_style(&rect("")), None);
    let plain = rect(r#","stroke":{"thickness":2}"#);
    assert_eq!(
        node_stroke_style(&plain),
        Some(StrokeStyleFields::default())
    );
    let styled = rect(
        r#","stroke":{"thickness":2,"cap":"square","join":"bevel","dashPattern":[3,1],"dashOffset":2}"#,
    );
    let style = node_stroke_style(&styled).expect("stroke");
    assert_eq!(style.cap, Some(StrokeCap::Square));
    assert_eq!(style.join, Some(StrokeJoin::Bevel));
    assert_eq!(style.dash_pattern, Some(vec![3.0, 1.0]));
    assert_eq!(style.dash_offset, Some(2.0));
}

#[test]
fn partial_write_keeps_untouched_fields_and_reports_changes() {
    let mut node = rect(r#","stroke":{"thickness":2,"join":"round"}"#);
    let cap = StrokeStyleWrite {
        cap: Some(StrokeCap::Round),
        ..StrokeStyleWrite::default()
    };
    assert!(set_stroke_style(&mut node, &cap));
    assert!(
        !set_stroke_style(&mut node, &cap),
        "same value is no change"
    );
    let style = node_stroke_style(&node).expect("stroke");
    assert_eq!(style.cap, Some(StrokeCap::Round));
    assert_eq!(style.join, Some(StrokeJoin::Round));

    let mut bare = rect("");
    assert!(!set_stroke_style(&mut bare, &cap), "no stroke is created");
}

#[test]
fn empty_pattern_clears_dash_and_offset() {
    let mut node = rect(r#","stroke":{"thickness":1,"dashPattern":[2,2],"dashOffset":1}"#);
    let clear = StrokeStyleWrite {
        dash_pattern: Some(Vec::new()),
        ..StrokeStyleWrite::default()
    };
    assert!(set_stroke_style(&mut node, &clear));
    let style = node_stroke_style(&node).expect("stroke");
    assert_eq!(style.dash_pattern, None);
    assert_eq!(style.dash_offset, None);
}

#[test]
fn invalid_writes_are_rejected() {
    let mut node = rect(r#","stroke":{"thickness":1}"#);
    for pattern in [vec![-1.0, 2.0], vec![0.0, 0.0], vec![f32::NAN]] {
        let write = StrokeStyleWrite {
            dash_pattern: Some(pattern),
            ..StrokeStyleWrite::default()
        };
        assert!(!set_stroke_style(&mut node, &write));
    }
    let offset = StrokeStyleWrite {
        dash_offset: Some(f32::INFINITY),
        ..StrokeStyleWrite::default()
    };
    assert!(!set_stroke_style(&mut node, &offset));
}

#[test]
fn presets_scale_with_width_and_round_trip() {
    assert_eq!(StrokeDashPreset::Dashed.pattern(2.0), Some(vec![8.0, 8.0]));
    assert_eq!(StrokeDashPreset::Dotted.pattern(0.5), Some(vec![0.0, 2.0]));
    for preset in StrokeDashPreset::ALL {
        let pattern = preset.pattern(3.0);
        assert_eq!(
            StrokeDashPreset::matching(pattern.as_deref(), 3.0),
            Some(preset)
        );
    }
    assert_eq!(StrokeDashPreset::matching(Some(&[5.0, 1.0]), 3.0), None);
    assert_eq!(
        StrokeDashPreset::matching(Some(&[]), 3.0),
        Some(StrokeDashPreset::Solid)
    );
}

#[test]
fn dotted_preset_sets_round_caps_and_solid_clears_dash() {
    let mut node = rect(r#","stroke":{"thickness":2}"#);
    assert!(set_stroke_dash_preset(&mut node, StrokeDashPreset::Dotted));
    let style = node_stroke_style(&node).expect("stroke");
    assert_eq!(style.cap, Some(StrokeCap::Round));
    assert_eq!(style.dash_pattern, Some(vec![0.0, 4.0]));

    assert!(set_stroke_dash_preset(&mut node, StrokeDashPreset::Solid));
    let style = node_stroke_style(&node).expect("stroke");
    assert_eq!(style.dash_pattern, None);
    assert_eq!(style.cap, Some(StrokeCap::Round), "solid keeps the cap");
    assert!(!set_stroke_dash_preset(
        &mut rect(""),
        StrokeDashPreset::Dashed
    ));
}

/// #141: a None / Square cap click while Dotted is active switches the
/// dash to Solid; a Round click keeps the dots, and Dashed keeps its dash.
#[test]
fn a_non_round_cap_turns_dotted_into_solid() {
    let mut node = rect(r#","stroke":{"thickness":2}"#);
    let dotted = StrokeStyleEdit::Dash(StrokeDashPreset::Dotted);
    assert!(apply_stroke_style_edit(&mut node, &dotted));
    assert!(!apply_stroke_style_edit(
        &mut node,
        &StrokeStyleEdit::Cap(StrokeCap::Round)
    ));
    assert_eq!(
        node_stroke_style(&node).unwrap().dash_pattern,
        Some(vec![0.0, 4.0])
    );

    for cap in [StrokeCap::None, StrokeCap::Square] {
        // Back to Dotted (a no-op on the first pass, which still is).
        let _ = apply_stroke_style_edit(&mut node, &dotted);
        assert_eq!(
            node_stroke_style(&node).unwrap().dash_pattern,
            Some(vec![0.0, 4.0])
        );
        assert!(apply_stroke_style_edit(
            &mut node,
            &StrokeStyleEdit::Cap(cap.clone())
        ));
        let style = node_stroke_style(&node).unwrap();
        assert_eq!(style.cap, Some(cap));
        assert_eq!(style.dash_pattern, None, "dots need round caps");
    }

    // Dots left over from another width still count as dots.
    let mut resized = rect(r#","stroke":{"thickness":5,"cap":"round","dashPattern":[0,4]}"#);
    assert!(apply_stroke_style_edit(
        &mut resized,
        &StrokeStyleEdit::Cap(StrokeCap::Square)
    ));
    assert_eq!(node_stroke_style(&resized).unwrap().dash_pattern, None);

    let dashed = StrokeStyleEdit::Dash(StrokeDashPreset::Dashed);
    assert!(apply_stroke_style_edit(&mut node, &dashed));
    assert!(apply_stroke_style_edit(
        &mut node,
        &StrokeStyleEdit::Cap(StrokeCap::Round)
    ));
    assert!(apply_stroke_style_edit(
        &mut node,
        &StrokeStyleEdit::Cap(StrokeCap::None)
    ));
    assert_eq!(
        node_stroke_style(&node).unwrap().dash_pattern,
        Some(vec![8.0, 8.0])
    );
}
