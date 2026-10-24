//! Shader image uniforms in the `.de` folder layout (#9).
//!
//! A shader fill's image texture is a string uniform bound to a
//! `uniform shader` child ([`de_editor_core::shader_images`]). The schema
//! types it as a plain string, not an `ImageSrc`, so the jian image visitor
//! never sees it and an `op-image:` reference there would not resolve on
//! load. [`crate::asset_folder`] therefore walks these slots separately: save
//! moves their `data:` payloads into the asset folder like any image, and
//! load puts the file's `data:` URL straight back into the slot.

use serde_json::{Map, Value};

/// Visit every shader image uniform string in `root`'s node tree: the
/// uniforms of `shader` fills — direct `fill` arrays, `stroke.fill` and
/// widget-state overrides — whose names the fill's SkSL declares as
/// `uniform shader`. Same structural walk as the jian image visitor, so
/// free-form JSON is never entered.
pub(crate) fn visit_shader_image_uniforms_mut(root: &mut Value, f: &mut impl FnMut(&mut String)) {
    let Value::Object(map) = root else {
        return;
    };
    let mut stack: Vec<&mut Value> = Vec::new();
    for (key, value) in map.iter_mut() {
        if key == "children" || key == "pages" {
            if let Value::Array(items) = value {
                stack.extend(items.iter_mut());
            }
        }
    }
    while let Some(node) = stack.pop() {
        let Value::Object(node) = node else {
            continue;
        };
        for (key, value) in node.iter_mut() {
            match (key.as_str(), value) {
                ("fill", fill) => visit_fill_array(fill, f),
                ("stroke", stroke) => visit_stroke(stroke, f),
                ("states", Value::Object(states)) => {
                    for state in states.values_mut() {
                        let Value::Object(state) = state else {
                            continue;
                        };
                        for (state_key, state_value) in state.iter_mut() {
                            match state_key.as_str() {
                                "fill" => visit_fill_array(state_value, f),
                                "stroke" => visit_stroke(state_value, f),
                                _ => {}
                            }
                        }
                    }
                }
                ("children", Value::Array(items)) => stack.extend(items.iter_mut()),
                _ => {}
            }
        }
    }
}

fn visit_fill_array(fill: &mut Value, f: &mut impl FnMut(&mut String)) {
    let Value::Array(items) = fill else {
        return;
    };
    for item in items {
        if let Value::Object(item) = item {
            visit_shader_fill(item, f);
        }
    }
}

fn visit_stroke(stroke: &mut Value, f: &mut impl FnMut(&mut String)) {
    if let Some(fill) = stroke
        .as_object_mut()
        .and_then(|stroke| stroke.get_mut("fill"))
    {
        visit_fill_array(fill, f);
    }
}

fn visit_shader_fill(item: &mut Map<String, Value>, f: &mut impl FnMut(&mut String)) {
    if item.get("type").and_then(Value::as_str) != Some("shader") {
        return;
    }
    let Some(sksl) = item.get("sksl").and_then(Value::as_str) else {
        return;
    };
    let children: Vec<String> = de_editor_core::shader_images::shader_children(sksl)
        .into_iter()
        .map(str::to_owned)
        .collect();
    if children.is_empty() {
        return;
    }
    let Some(Value::Object(uniforms)) = item.get_mut("uniforms") else {
        return;
    };
    for (name, value) in uniforms.iter_mut() {
        if let Value::String(src) = value {
            if children.contains(name) {
                f(src);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_only_strings_bound_to_shader_children() {
        let mut doc = serde_json::json!({
            "children": [{
                "type": "rectangle",
                "fill": [{
                    "type": "shader",
                    "sksl": "uniform shader photo; uniform half4 tint; half4 main(float2 p){return photo.eval(p);}",
                    "uniforms": { "photo": "a.assets/x.png", "tint": "#ff0000" }
                }],
                "states": { "hover": { "stroke": { "fill": [{
                    "type": "shader",
                    "sksl": "uniform shader mask;",
                    "uniforms": { "mask": "data:image/png;base64,AA==" }
                }]}}},
                "children": [{ "type": "image", "src": "b.assets/y.png" }]
            }],
            "variables": { "photo": "c.assets/z.png" }
        });
        let mut seen = Vec::new();
        visit_shader_image_uniforms_mut(&mut doc, &mut |slot| seen.push(slot.clone()));
        seen.sort();
        assert_eq!(seen, vec!["a.assets/x.png", "data:image/png;base64,AA=="]);
    }
}
