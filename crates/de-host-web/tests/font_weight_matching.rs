//! CanvasKit picks a family's face for a weight the way CSS font matching
//! does, like jian-skia's native resolver (#300).

#[test]
fn canvaskit_font_face_matching_runs_in_javascript() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut source = std::fs::read_to_string(format!(
        "{}/src/op_ck_bridge.js",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("CanvasKit bridge source is readable");
    source.push_str(
        r#"
const assert = (condition, message) => { if (!condition) throw new Error(message); };
const regularBold = [
  { tf: 'regular', weight: 400, italic: false, width: 5 },
  { tf: 'bold', weight: 700, italic: false, width: 5 },
];
const picked = (faces, weight, italic = false) => faces[opCkMatchFontFace(faces, weight, italic)];

assert(picked(regularBold, 500).tf === 'regular', '500 on a Regular + Bold family must pick Regular');
assert(picked(regularBold, 600).tf === 'bold', '600 on a Regular + Bold family must pick Bold');
for (const [weight, expected] of [[100, 'regular'], [200, 'regular'], [300, 'regular'], [400, 'regular'],
  [500, 'regular'], [600, 'bold'], [700, 'bold'], [800, 'bold'], [900, 'bold']]) {
  assert(picked(regularBold, weight).tf === expected, `${weight} must pick ${expected}`);
}

const weights = (list) => list.map((weight) => ({ tf: weight, weight, italic: false }));
assert(picked(weights([300, 500, 600]), 400).tf === 500, '400 tries 500 before lighter faces');
assert(picked(weights([300, 600]), 450).tf === 300, '450 tries lighter faces before heavier ones');
assert(picked(weights([200, 400]), 300).tf === 200, 'below 400 goes lighter first');
assert(picked(weights([500, 800]), 600).tf === 800, 'above 500 goes heavier first');
assert(picked(weights([400, 500]), 600).tf === 500, 'above 500 falls back to the heaviest lighter face');

const slanted = [...regularBold,
  { tf: 'italic', weight: 400, italic: true, width: 5 },
  { tf: 'bold-italic', weight: 700, italic: true, width: 5 }];
assert(picked(slanted, 500, true).tf === 'italic', 'an italic request keeps to italic faces');
assert(picked(slanted, 700, false).tf === 'bold', 'an upright request keeps to upright faces');
const condensedBold = [{ tf: 'condensed-bold', weight: 700, italic: false, width: 3 }, ...regularBold];
assert(picked(condensedBold, 700).tf === 'bold', 'a normal-width face wins over a condensed one');
assert(opCkMatchFontFace([], 400, false) === -1, 'an empty family matches nothing');

assert(!opCkSyntheticBold(500, 400), 'no synthetic bold below 600');
assert(opCkSyntheticBold(600, 400), 'a 600 request over Regular strokes');
assert(!opCkSyntheticBold(600, 700), 'a real Bold face is not stroked again');

const system = new Map([['arial', { tf: 'regular', faces: regularBold }]]);
let resolved = opCkResolveRegisteredTypeface('Arial', new Map(), system, new Map(), 500, false);
assert(resolved.tf === 'regular' && resolved.faceWeight === 400, 'Arial 500 resolves to Regular');
const regularKey = resolved.key;
resolved = opCkResolveRegisteredTypeface('Arial', new Map(), system, new Map(), 600, false);
assert(resolved.tf === 'bold' && resolved.faceWeight === 700, 'Arial 600 resolves to Bold');
assert(resolved.key !== regularKey, 'each face gets its own cache key');
resolved = opCkResolveRegisteredTypeface('Inter', new Map([['inter', { tf: 'inter' }]]), system, new Map(), 700, false);
assert(resolved.tf === 'inter' && resolved.faceWeight === 400 && resolved.key === 'imported:inter',
  'a single-face entry keeps its face and key at any weight');
"#,
    );

    let mut child = match Command::new("node")
        .args(["--input-type=module"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => panic!("failed to start node for CanvasKit bridge test: {error}"),
    };
    child
        .stdin
        .take()
        .expect("node stdin is available")
        .write_all(source.as_bytes())
        .expect("CanvasKit bridge test source is writable");
    let output = child
        .wait_with_output()
        .expect("CanvasKit bridge JavaScript test completes");
    assert!(
        output.status.success(),
        "CanvasKit font matching JavaScript assertions failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
