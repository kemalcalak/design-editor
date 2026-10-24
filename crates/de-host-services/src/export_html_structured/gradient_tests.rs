//! Gradient fills as CSS `<image>` values — split out of `tests.rs` at
//! the 800-line ceiling; shares its board fixtures.

use super::*;

#[test]
fn a_linear_gradient_becomes_a_css_gradient_at_the_same_angle() {
    let mut node = SceneNode::leaf("g", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    node.fill_type = SceneFillType::LinearGradient;
    node.gradient = Some(SceneGradient::Linear {
        angle_deg: 90.0,
        opacity: 1.0,
        stops: vec![
            SceneGradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            SceneGradientStop {
                offset: 1.0,
                color: Color::BLUE,
            },
        ],
    });

    let body = body_of(board(vec![node]));

    assert!(
        body.contains("background-image:linear-gradient(90deg,rgb(255,0,0) 0%,rgb(0,0,255) 100%)"),
        "{body}"
    );
}

#[test]
fn a_radial_gradient_keeps_its_centre_and_radius_in_pixels() {
    let mut node = SceneNode::leaf("g", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    node.fill_type = SceneFillType::RadialGradient;
    node.gradient = Some(SceneGradient::Radial {
        cx: 0.5,
        cy: 0.5,
        radius: 0.5,
        opacity: 1.0,
        stops: vec![
            SceneGradientStop {
                offset: 0.0,
                color: Color::WHITE,
            },
            SceneGradientStop {
                offset: 1.0,
                color: Color::BLACK,
            },
        ],
    });

    let body = body_of(board(vec![node]));

    // radius = max(w, h) * 0.5 = 50; centre = (50, 25).
    assert!(
        body.contains("radial-gradient(circle 50px at 50px 25px,"),
        "{body}"
    );
}

#[test]
fn an_angular_gradient_becomes_a_css_conic_gradient() {
    let mut node = SceneNode::leaf("g", NodeKind::Rect);
    node.bounds = Rect::xywh(0.0, 0.0, 100.0, 50.0);
    node.fill_type = SceneFillType::AngularGradient;
    node.gradient = Some(SceneGradient::Angular {
        cx: 0.25,
        cy: 0.5,
        rotation_deg: 45.0,
        opacity: 1.0,
        stops: vec![
            SceneGradientStop {
                offset: 0.0,
                color: Color::RED,
            },
            SceneGradientStop {
                offset: 1.0,
                color: Color::BLUE,
            },
        ],
    });

    let body = body_of(board(vec![node]));

    assert!(
        body.contains(
            "background-image:conic-gradient(from 45deg at 25px 25px,rgb(255,0,0) 0%,rgb(0,0,255) 100%)"
        ),
        "{body}"
    );
}
