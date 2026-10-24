//! Regression coverage for CanvasKit shader-effect layers (#6).

use std::io::Write;
use std::process::{Command, Stdio};

fn source(path: &str) -> String {
    std::fs::read_to_string(format!("{}/src/{path}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

/// Run `javascript` as an ES module under node; skipped when node is absent.
fn run_node_module(javascript: &str, context: &str) {
    let mut child = match Command::new("node")
        .args(["--input-type=module"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("failed to start node for {context}: {error}"),
    };
    child
        .stdin
        .take()
        .expect("node stdin is available")
        .write_all(javascript.as_bytes())
        .unwrap_or_else(|error| panic!("failed to write {context} source: {error}"));
    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("failed to wait for {context}: {error}"));
    assert!(
        output.status.success(),
        "{context} JavaScript assertions failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn canvaskit_backend_overrides_the_shader_effect_layer() {
    // The effect items expand from a macro in a sibling (800-line cap).
    let backend = source("canvaskit/backend.rs") + &source("canvaskit/shader_textures.rs");
    let bindings = source("canvaskit/bindings.rs");
    let ops = source("canvaskit/ops.rs");
    let bridge = source("op_ck_bridge.js");
    assert!(backend.contains("shader_texture_items!()"));
    assert!(backend.contains("fn push_shader_effect_layer("));
    assert!(ops.contains("fn push_shader_effect_layer("));
    assert!(bindings.contains("js_name = pushShaderEffectLayer"));
    assert!(bridge.contains("pushShaderEffectLayer("));
    assert!(
        bridge.contains("finishObjectShaderEffect()"),
        "restore() must close an open object pass"
    );
}

#[cfg(feature = "canvaskit")]
#[test]
fn object_pass_draws_the_program_over_an_offscreen_capture_in_real_canvaskit() {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/canvaskit");
    let quote = |path: std::path::PathBuf| {
        serde_json::to_string(&path.to_string_lossy()).expect("path serializes")
    };
    let mut javascript = String::from(
        "import { createRequire } from 'module';\nimport fs from 'fs';\n\
         const require = createRequire(import.meta.url);\n",
    );
    javascript.push_str(&source("op_ck_bridge.js"));
    javascript.push_str(&format!(
        r#"
const assert = (condition, message) => {{ if (!condition) throw new Error(message); }};
const CanvasKitInit = require({js});
const CK = await CanvasKitInit({{ wasmBinary: fs.readFileSync({wasm}) }});
const main = CK.MakeSurface(40, 40);
const canvas = main.getCanvas();
canvas.clear(CK.TRANSPARENT);
// A 2x canvas transform, like a HiDPI frame: the 20x20 rect covers 40x40 px.
canvas.scale(2, 2);
const sksl = 'uniform shader object; uniform float2 size; half4 main(float2 p) {{ half4 c = object.eval(p); return p.x < 5.0 ? half4(0.0, c.a, 0.0, c.a) : c; }}';
const entry = opCkCreateRuntimeEffectCache(CK).get(sksl);
assert(entry && entry.children.includes('object'), 'program compiles with an object child');
const box = opCkDeviceBounds(canvas.getTotalMatrix(), 0, 0, 20, 20, main.width(), main.height());
assert(JSON.stringify(box) === '[0,0,40,40]', `device box: ${{box}}`);
const offscreen = main.makeSurface({{
  width: 40, height: 40,
  colorType: CK.ColorType.RGBA_8888, alphaType: CK.AlphaType.Premul, colorSpace: CK.ColorSpace.SRGB,
}});
assert(offscreen, 'makeSurface builds an offscreen surface');
const oc = offscreen.getCanvas();
oc.clear(CK.TRANSPARENT);
oc.concat(canvas.getTotalMatrix());
const red = new CK.Paint();
red.setColor(CK.Color(255, 0, 0, 1));
oc.drawRect(CK.XYWHRect(0, 0, 20, 20), red);
const image = offscreen.makeImageSnapshot();
// `size` = 10x10 document px over the 20x20 rect: document x < 5 is canvas x < 10.
const pass = {{ entry, x: 0, y: 0, w: 20, h: 20, names: ['size'], values: new Float32Array([10, 10]), arities: new Uint32Array([2]) }};
assert(opCkDrawShaderEffectPass(CK, canvas, pass, image, 'object', 0, 0), 'pass draws');
const pixels = main.makeImageSnapshot().readPixels(0, 0, {{
  width: 40, height: 40, colorType: CK.ColorType.RGBA_8888, alphaType: CK.AlphaType.Unpremul, colorSpace: CK.ColorSpace.SRGB,
}});
const at = (x, y) => Array.from(pixels.slice((y * 40 + x) * 4, (y * 40 + x) * 4 + 4));
const left = at(10, 20);
const right = at(30, 20);
assert(left[1] > 240 && left[0] < 16, `left half green: ${{left}}`);
assert(right[0] > 240 && right[1] < 16, `right half red: ${{right}}`);
"#,
        js = quote(assets.join("canvaskit.js")),
        wasm = quote(assets.join("canvaskit.wasm")),
    ));
    run_node_module(&javascript, "real CanvasKit object pass");
}

#[test]
fn shader_effect_matrix_helpers_map_captured_pixels_to_node_space() {
    let mut javascript = source("op_ck_bridge.js");
    javascript.push_str(
        r#"
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const near = (a, b) => Math.abs(a - b) < 1e-6;
const apply = (m, x, y) => {
  const z = m[6] * x + m[7] * y + m[8];
  return [(m[0] * x + m[1] * y + m[2]) / z, (m[3] * x + m[4] * y + m[5]) / z];
};

const m = [2, 0, 10, 0, 3, 5, 0, 0, 1];
const inv = opCkMat3Invert(m);
const id = opCkMat3Multiply(m, inv);
assert([1, 0, 0, 0, 1, 0, 0, 0, 1].every((v, i) => near(v, id[i])), 'inverse round-trips');
assert(opCkMat3Invert([0, 0, 0, 0, 0, 0, 0, 0, 1]) === null, 'singular has no inverse');

// A 2x device scale, node at canvas (10, 20), 40x30, clamped to 100x100.
const total = [2, 0, 0, 0, 2, 0, 0, 0, 1];
assert(JSON.stringify(opCkDeviceBounds(total, 10, 20, 40, 30, 100, 100)) === '[20,40,100,100]',
  'device box clamps to the surface');
assert(opCkDeviceBounds(total, 200, 200, 10, 10, 100, 100) === null, 'off-surface is null');

// Node-local doc space: size 20x15 over the 40x30 rect, so local scale 2.
const local = opCkRuntimeEffectLocalMatrix(['size'], [20, 15], [2], 10, 20, 40, 30);
const child = opCkShaderEffectChildMatrix(total, local, 20, 40);
// The capture's pixel (0, 0) is device (20, 40) = canvas (10, 20) = node (0, 0);
// pixel (80, 60) is device (100, 100) = canvas (50, 50) = node (20, 15).
const origin = apply(child, 0, 0);
const corner = apply(child, 80, 60);
assert(near(origin[0], 0) && near(origin[1], 0), `origin maps to node 0,0: ${origin}`);
assert(near(corner[0], 20) && near(corner[1], 15), `corner maps to node size: ${corner}`);
"#,
    );
    run_node_module(&javascript, "shader effect matrix helpers");
}

/// Shader textures (#165) in real CanvasKit: an SDF float image, a fill
/// child bound to a frame texture, an effect pass sampling its backdrop
/// and its own SDF, and an int uniform cast by `makeShader`.
#[cfg(feature = "canvaskit")]
#[test]
fn shader_textures_bind_in_real_canvaskit() {
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/canvaskit");
    let quote = |path: std::path::PathBuf| {
        serde_json::to_string(&path.to_string_lossy()).expect("path serializes")
    };
    let mut javascript = String::from(
        "import { createRequire } from 'module';\nimport fs from 'fs';\n\
         const require = createRequire(import.meta.url);\n",
    );
    javascript.push_str(&source("op_ck_bridge.js"));
    javascript.push_str(&format!(
        r#"
const assert = (condition, message) => {{ if (!condition) throw new Error(message); }};
const CanvasKitInit = require({js});
const CK = await CanvasKitInit({{ wasmBinary: fs.readFileSync({wasm}) }});
const read = (surface) => surface.makeImageSnapshot().readPixels(0, 0, {{
  width: 40, height: 40, colorType: CK.ColorType.RGBA_8888, alphaType: CK.AlphaType.Unpremul, colorSpace: CK.ColorSpace.SRGB,
}});
const at = (pixels, x, y) => Array.from(pixels.slice((y * 40 + x) * 4, (y * 40 + x) * 4 + 4));
const paint = (r, g, b) => {{ const p = new CK.Paint(); p.setColor(CK.Color(r, g, b, 1)); return p; }};

// Tags and kinds mirror the Rust rules.
const tags = opCkShaderChildTags('uniform shader object;\n/** @label S @sdf @mipmap */ uniform shader shape;');
assert(tags.get('shape').sdf && tags.get('shape').mipmap && !tags.get('object').sdf, 'child tags');
assert(opCkTextureKind(0xD8000000) === 2 && opCkTextureKind(0x12345678) === null, 'texture kinds');

// A disc's field: about +20 texels at the centre, negative at a corner.
const disc = CK.MakeSurface(40, 40);
disc.getCanvas().clear(CK.TRANSPARENT);
disc.getCanvas().drawOval(CK.XYWHRect(0, 0, 40, 40), paint(255, 255, 255));
const made = opCkMakeSdfImage(CK, disc.makeImageSnapshot(), 1);
assert(made && made.image.width() === 40 && made.factor === 1, 'an SDF float image is made');
const sdf = made.image;

// A fill reading the field through a frame texture child.
const fillSksl = '/** @sdf */ uniform shader shape; half4 main(float2 p) {{ float d = shape.eval(p).r; return d > 0.0 ? half4(d / 40.0, 0.0, 0.0, 1.0) : half4(0.0, 1.0, 0.0, 1.0); }}';
const fillEntry = opCkCreateRuntimeEffectCache(CK).get(fillSksl);
const id = new Float32Array([1, 0, 0, 0xD800]);
// Made at device (0, 0), one texel per pixel, under an identity canvas.
const shader = opCkMakeRuntimeShaderForRect(fillEntry, ['shape'], id, new Uint32Array([4]), 0, 0, 40, 40, CK,
  () => ({{ image: sdf, kind: 2, dl: 0, dt: 0, texel: 1 }}), [1, 0, 0, 0, 1, 0, 0, 0, 1]);
assert(shader, 'the fill shader builds');
const fill = CK.MakeSurface(40, 40);
const fillPaint = new CK.Paint();
fillPaint.setShader(shader);
fill.getCanvas().drawRect(CK.XYWHRect(0, 0, 40, 40), fillPaint);
const fp = read(fill);
assert(Math.abs(at(fp, 20, 20)[0] - 128) < 20 && at(fp, 20, 20)[1] < 16, `centre: ${{at(fp, 20, 20)}}`);
assert(at(fp, 1, 1)[1] > 240, `corner: ${{at(fp, 1, 1)}}`);

// An effect pass: node red on the left, backdrop blue, outline from its SDF.
const main = CK.MakeSurface(40, 40);
const canvas = main.getCanvas();
canvas.clear(CK.TRANSPARENT);
canvas.drawRect(CK.XYWHRect(0, 0, 40, 40), paint(0, 0, 255));
const backdrop = main.makeImageSnapshot();
const offscreen = main.makeSurface({{ width: 40, height: 40, colorType: CK.ColorType.RGBA_8888, alphaType: CK.AlphaType.Premul, colorSpace: CK.ColorSpace.SRGB }});
offscreen.getCanvas().clear(CK.TRANSPARENT);
offscreen.getCanvas().drawRect(CK.XYWHRect(0, 0, 20, 40), paint(255, 0, 0));
const capture = offscreen.makeImageSnapshot();
const effectSksl = 'uniform shader object; uniform shader background; /** @sdf */ uniform shader edge; half4 main(float2 p) {{ float d = edge.eval(p).r; return abs(d) < 1.5 ? half4(0.0, 1.0, 0.0, 1.0) : object.eval(p) + background.eval(p); }}';
const effectEntry = opCkCreateRuntimeEffectCache(CK).get(effectSksl);
assert(effectEntry, 'the effect compiles');
const values = new Float32Array([2, 0, 0, 0xD400, 0, 0, 0, 0xD800]);
const pass = {{ entry: effectEntry, x: 0, y: 0, w: 40, h: 40, names: ['background', 'edge'], values, arities: new Uint32Array([4, 4]) }};
assert(opCkDrawShaderEffectPass(CK, canvas, pass, capture, 'object', 0, 0,
  () => ({{ image: backdrop, kind: 1, dl: 0, dt: 0, texel: 1 }})), 'the effect pass draws');
// Every name of a multi-name declaration shares its doc tags.
assert(opCkShaderChildTags('/** @mipmap */ uniform shader a, b;').get('b').mipmap, 'multi-name tags');
const ep = read(main);
assert(at(ep, 10, 20)[0] > 240 && at(ep, 10, 20)[2] > 240, `node + backdrop: ${{at(ep, 10, 20)}}`);
assert(at(ep, 30, 20)[2] > 240 && at(ep, 30, 20)[0] < 16, `backdrop only: ${{at(ep, 30, 20)}}`);
assert(at(ep, 19, 20)[1] > 240 || at(ep, 20, 20)[1] > 240, `edge outline: ${{at(ep, 19, 20)}} ${{at(ep, 20, 20)}}`);

// Integer uniforms: CanvasKit casts the float slots.
const intSksl = 'uniform int steps; half4 main(float2 p) {{ return steps == 3 ? half4(1, 0, 0, 1) : half4(0, 1, 0, 1); }}';
const intEntry = opCkCreateRuntimeEffectCache(CK).get(intSksl);
const intShader = opCkMakeRuntimeShader(intEntry, ['steps'], new Float32Array([2.6]), new Uint32Array([1]));
const ints = CK.MakeSurface(40, 40);
const intPaint = new CK.Paint();
intPaint.setShader(intShader);
ints.getCanvas().drawRect(CK.XYWHRect(0, 0, 40, 40), intPaint);
assert(at(read(ints), 5, 5)[0] > 240, `int uniform binds 3: ${{at(read(ints), 5, 5)}}`);
"#,
        js = quote(assets.join("canvaskit.js")),
        wasm = quote(assets.join("canvaskit.wasm")),
    ));
    run_node_module(&javascript, "real CanvasKit shader textures");
}
