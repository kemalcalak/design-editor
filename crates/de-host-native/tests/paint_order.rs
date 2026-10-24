#[test]
fn canvas_paints_before_property_panel_overlays() {
    let source = std::fs::read_to_string(format!(
        "{}/src/widget_host/paint.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("paint source is readable");

    let canvas = source
        .find("CanvasViewport — the whole window")
        .expect("canvas paint block marker exists");
    let property = source
        .find("PropertyPanel — only when selection")
        .expect("property panel paint block marker exists");

    assert!(
        canvas < property,
        "PropertyPanel must paint after CanvasViewport so popovers extending into the canvas are not covered"
    );
    // Editor v3 (#86): the left card floats over the full-bleed canvas.
    let left_card = source
        .find("The left card — floats over the canvas")
        .expect("left card paint block marker exists");
    assert!(
        canvas < left_card,
        "the left card must paint over the canvas"
    );
}

#[test]
fn image_fill_popover_paints_above_status_bar() {
    let source = std::fs::read_to_string(format!(
        "{}/src/widget_host/paint.rs",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("paint source is readable");

    let status = source
        .find("StatusBar — floating bottom-right")
        .expect("status paint block marker exists");
    let overlays = source
        .find("PropertyPanel overlays")
        .expect("property overlay paint block marker exists");

    assert!(
        status < overlays,
        "image-fill popover must paint after StatusBar so the zoom pill cannot cover adjustment rows"
    );
}
