use super::*;

fn stroke(json: &str) -> PenStroke {
    serde_json::from_str(json).expect("stroke fixture")
}

#[test]
fn plain_stroke_has_no_render_style() {
    assert_eq!(
        StrokeRenderStyle::from_stroke(&stroke(r#"{"thickness":1}"#)),
        None
    );
}

#[test]
fn schema_fields_map_to_render_style() {
    let style = StrokeRenderStyle::from_stroke(&stroke(
        r#"{"thickness":1,"cap":"none","join":"bevel","dashPattern":[4,2],"dashOffset":1}"#,
    ))
    .expect("style");
    assert_eq!(style.cap, Some(LineCap::Butt));
    assert_eq!(style.join, Some(LineJoin::Bevel));
    assert_eq!(style.dash, Some(vec![4.0, 2.0]));
    assert_eq!(style.dash_offset, 1.0);
}

#[test]
fn odd_patterns_repeat_and_unusable_dashes_read_solid() {
    let odd = StrokeRenderStyle::from_stroke(&stroke(r#"{"thickness":1,"dashPattern":[3]}"#))
        .expect("style");
    assert_eq!(odd.dash, Some(vec![3.0, 3.0]));
    for bad in ["[]", "[0,0]", "[-1,2]"] {
        let json = format!(r#"{{"thickness":1,"dashPattern":{bad},"dashOffset":5}}"#);
        assert_eq!(
            StrokeRenderStyle::from_stroke(&stroke(&json)),
            None,
            "{bad}"
        );
    }
}

#[test]
fn guards_scale_nest_and_restore() {
    let dashed = StrokeRenderStyle {
        cap: None,
        join: Some(LineJoin::Round),
        dash: Some(vec![2.0, 1.0]),
        dash_offset: 0.5,
    };
    assert!(active().is_none());
    {
        let _outer = enter(Some(&dashed), 2.0);
        let outer = active().expect("outer active");
        assert_eq!(outer.dash, Some(vec![4.0, 2.0]));
        assert_eq!(outer.dash_offset, 1.0);
        {
            let _inner = enter(None, 2.0);
            assert!(active().is_none(), "a child scope does not inherit");
        }
        assert_eq!(active().expect("restored").join, Some(LineJoin::Round));
    }
    assert!(active().is_none());
}

#[test]
fn registry_records_and_clears_by_node_id() {
    let id = "stroke-render-style-test-node";
    let style = StrokeRenderStyle {
        cap: Some(LineCap::Square),
        join: None,
        dash: None,
        dash_offset: 0.0,
    };
    register_node(id, Some(style.clone()));
    assert_eq!(registered(id).as_deref(), Some(&style));
    {
        let _guard = enter_node(id, 1.0);
        assert_eq!(active().expect("active").cap, Some(LineCap::Square));
    }
    register_node(id, None);
    assert!(registered(id).is_none());
}
