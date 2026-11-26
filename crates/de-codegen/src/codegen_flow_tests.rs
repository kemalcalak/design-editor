use crate::{Codegen, Compose, Flutter, React, ReactNative, Svelte, SwiftUi, Vue};
use jian_ops_schema::PenDocument;

fn doc(container: &str) -> PenDocument {
    let card = |id: &str| {
        format!(
            r##"{{"type":"rectangle","id":"{id}","width":100,"height":60,"fill":[{{"type":"solid","color":"#ff0000"}}]}}"##
        )
    };
    jian_ops_schema::load_str(&format!(
        r#"{{"version":"1.0","children":[{{"type":"frame","id":"box","x":10,"y":20,"width":340,"height":200,{container},"children":[{},{},{}]}}]}}"#,
        card("a"),
        card("b"),
        card("c")
    ))
    .expect("document")
    .value
}

const GRID: &str = r#""layout":"grid","columns":[100,"1fr","auto"],"gap":12,"rowGap":8"#;
const WRAP: &str = r#""layout":"horizontal","wrap":true,"gap":6"#;

#[test]
fn html_targets_emit_css_grid_and_flow_children() {
    for out in [Vue.generate(&doc(GRID)), Svelte.generate(&doc(GRID))] {
        assert!(
            out.contains(
                "display:grid;grid-template-columns:100px 1fr auto;row-gap:8px;column-gap:12px"
            ),
            "{out}"
        );
        assert!(
            out.contains("position:relative;flex:none;width:100px;height:60px"),
            "{out}"
        );
        assert!(
            !out.contains("position:absolute;left:0px;top:0px;width:100px"),
            "{out}"
        );
    }
}

#[test]
fn html_targets_emit_flex_wrap() {
    let out = Vue.generate(&doc(WRAP));
    assert!(out.contains("display:flex;flex-wrap:wrap"), "{out}");
    assert!(out.contains("row-gap:6px;column-gap:6px"), "{out}");
}

#[test]
fn react_and_react_native() {
    let react = React.generate(&doc(GRID));
    assert!(
        react.contains("display: 'grid', gridTemplateColumns: '100px 1fr auto'"),
        "{react}"
    );
    assert!(
        react.contains("position: 'relative', flex: 'none', width: 100"),
        "{react}"
    );
    let rn = ReactNative.generate(&doc(WRAP));
    assert!(
        rn.contains("flexDirection: 'row', flexWrap: 'wrap'"),
        "{rn}"
    );
    assert!(rn.contains("position: 'relative', width: 100"), "{rn}");
}

#[test]
fn flutter_grid_view_and_wrap() {
    let grid = Flutter.generate(&doc(GRID));
    assert!(
        grid.contains("GridView.count(crossAxisCount: 3, mainAxisSpacing: 8, crossAxisSpacing: 12"),
        "{grid}"
    );
    assert!(grid.contains("SizedBox(width: 100, height: 60"), "{grid}");
    let wrap = Flutter.generate(&doc(WRAP));
    assert!(
        wrap.contains("Wrap(spacing: 6, runSpacing: 6, children: ["),
        "{wrap}"
    );
}

#[test]
fn swiftui_lazy_v_grid() {
    let grid = SwiftUi.generate(&doc(GRID));
    assert!(
        grid.contains("LazyVGrid(columns: [GridItem(.fixed(100), spacing: 12), GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)], alignment: .leading, spacing: 8)"),
        "{grid}"
    );
    assert!(grid.contains(".position(x: 180, y: 120)"), "{grid}");
    let wrap = SwiftUi.generate(&doc(WRAP));
    assert!(
        wrap.contains("GridItem(.adaptive(minimum: 100), spacing: 6)"),
        "{wrap}"
    );
}

#[test]
fn compose_lazy_vertical_grid_and_flow_row() {
    let grid = Compose.generate(&doc(GRID));
    assert!(grid.contains("LazyVerticalGrid(columns = GridCells.Fixed(3), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp))"), "{grid}");
    assert!(
        grid.contains("item { Box(modifier = Modifier.size(width = 100.dp, height = 60.dp)"),
        "{grid}"
    );
    let wrap = Compose.generate(&doc(WRAP));
    assert!(wrap.contains("FlowRow(horizontalArrangement"), "{wrap}");
}

#[test]
fn plain_layouts_keep_absolute_positions() {
    let out = Vue.generate(&doc(r#""layout":"horizontal""#));
    assert!(!out.contains("display:grid"));
    assert!(!out.contains("flex-wrap"));
    assert!(out.contains("position:absolute"));
}
