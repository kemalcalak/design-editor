---
name: shader-effect
description: SkSL shader effects — post-process a node's rendering or its backdrop (glass, distortion, pixelate, colour grading)
phase: [generation]
trigger:
  keywords: [shader effect, post-process, postprocess, glass, refraction, distortion, distort, pixelate, chromatic, colour grading, color grading, grayscale, duotone]
priority: 27
budget: 760
category: knowledge
---

SHADER EFFECT (advanced — render-only):

An `effects` entry can run an SkSL program over the node AFTER it renders.
Use it for looks plain effects can't make: glass/refraction, distortion,
pixelate, chromatic aberration, duotone or colour grading. For ordinary
depth use `shadow`; for frosted glass FIRST try `background_blur`. A
program that fails to compile leaves the node unaffected.

SHAPE (one effects entry):

```
effects: [{ type: "shader", sksl: "<SkSL source>", uniforms: { name: value } }]
```

- Textures are SkSL children: `uniform shader object;` = the node's own
  rendering (fill, stroke, children, shadows; the output replaces it),
  `uniform shader background;` = what is painted behind the node. With
  only `background` the node paints on top of the output. Both together
  work too (glass on a distorted node), as do image children and
  `/** @sdf */ uniform shader edge;` (red = signed distance to the
  content's edge in px, positive inside; green/blue = its direction).
- Entrypoint EXACTLY `half4 main(float2 p)`; `p` is in node pixels
  (0..width, 0..height). `uniform float2 size;` is bound to the node size.
- `uniforms` work like a shader fill's (number, `[a,b]` vector, `"#hex"`
  colour). `uniform float time;` / `uniform float2 mouse;` are bound by the
  canvas (exports: `time = 0`, `mouse` far outside the node); never put
  `size`, `time` or `mouse` in `uniforms`. An image child takes its
  source in `uniforms`, as in a shader fill.
- Order: `background` passes run first (before the node paints), in list
  order; `object` passes then run in list order over the node. Output is
  cropped to the node box.

KNOWN-GOOD SNIPPETS:

1) Grayscale the node:

```
effects: [{ type: "shader",
  sksl: "uniform shader object; half4 main(float2 p){ half4 c = object.eval(p); half l = dot(c.rgb, half3(0.299, 0.587, 0.114)); return half4(l, l, l, c.a); }" }]
```

2) Pixelate (cell in px):

```
effects: [{ type: "shader",
  sksl: "uniform shader object; uniform float cell; half4 main(float2 p){ return object.eval((floor(p / cell) + 0.5) * cell); }",
  uniforms: { cell: 8 } }]
```

3) Glass ripple over the backdrop (give the node a translucent fill):

```
effects: [{ type: "shader",
  sksl: "uniform shader background; uniform float2 size; half4 main(float2 p){ float2 q = p + 6.0 * sin(p.yx / size.yx * 12.0); return background.eval(q); }" }]
```
