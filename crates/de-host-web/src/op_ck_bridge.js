// Thin CanvasKit FFI bridge for the Rust web shell.
//
// FFI glue only — flat, scalar-argument drawing functions the Rust
// `CanvasKitBackend` calls through wasm-bindgen. All drawing / layout / widget
// logic lives in Rust; this file just maps those calls onto CanvasKit
// `SkCanvas` ops. Depends on the compiled CanvasKit artifact
// (`/canvaskit/canvaskit.{js,wasm}`), not on any TS source.

function loadScript(src) {
  return new Promise((res, rej) => {
    if (window.CanvasKitInit) return res();
    const s = document.createElement('script');
    s.src = src;
    s.onload = res;
    s.onerror = () => rej(new Error('failed to load ' + src));
    document.head.appendChild(s);
  });
}

function copyBytes(u8) {
  return u8.buffer.slice(u8.byteOffset, u8.byteOffset + u8.byteLength);
}

let createWebImageCaches = null;

const TEXT_SCALE_STEPS_PER_OCTAVE = 4;

// Round UP to quarter-octave buckets (about 19% apart). This keeps cached
// browser-text bitmaps at or above the requested resolution while allowing
// adjacent continuous-zoom frames to reuse the same raster.
export function opCkQuantizeTextRasterScale(scale) {
  const finiteScale = Number.isFinite(scale) && scale > 1 ? scale : 1;
  const bucket = Math.ceil(
    Math.log2(finiteScale) * TEXT_SCALE_STEPS_PER_OCTAVE - 1e-9,
  );
  let quantized = 2 ** (bucket / TEXT_SCALE_STEPS_PER_OCTAVE);
  if (quantized < finiteScale) {
    quantized *= 2 ** (1 / TEXT_SCALE_STEPS_PER_OCTAVE);
  }
  return quantized;
}

export function setImageCacheFactory(factory) {
  createWebImageCaches = factory;
}

const CSS_GENERIC_FONT_FAMILIES = new Set([
  'serif',
  'sans-serif',
  'monospace',
  'cursive',
  'fantasy',
  'system-ui',
  'ui-serif',
  'ui-sans-serif',
  'ui-monospace',
  'ui-rounded',
  'math',
  'emoji',
  'fangsong',
  '-apple-system',
  'blinkmacsystemfont',
]);

// Family matching is ASCII-case-insensitive. Deliberately avoid locale-aware
// case conversion: font registry keys must be stable in every browser locale.
export function opCkNormalizeFontFamilyName(family) {
  return String(family || '').trim().replace(/[A-Z]/g, (ch) => String.fromCharCode(ch.charCodeAt(0) + 32));
}

// Parse a CSS font-family list without splitting commas contained in quoted
// family names. A backslash quotes the following character, which is enough
// for the escaped quotes and commas emitted by our HTML/Figma importers.
export function opCkParseFontFamilyStack(family) {
  const candidates = [];
  let current = '';
  let quote = '';
  let escaped = false;
  const push = () => {
    const candidate = current.trim();
    if (candidate) candidates.push(candidate);
    current = '';
  };
  for (const ch of String(family || '')) {
    if (escaped) {
      current += ch;
      escaped = false;
    } else if (ch === '\\') {
      escaped = true;
    } else if (quote) {
      if (ch === quote) quote = '';
      else current += ch;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
    } else if (ch === ',') {
      push();
    } else {
      current += ch;
    }
  }
  if (escaped) current += '\\';
  push();
  return candidates;
}

// Resolve registered families in authored CSS order. For one candidate an
// explicit browser import wins over the system face with the same name, and an
// app-bundled design font is the fallback below both — the same imported >
// system > bundled order the native resolver applies. A generic family is
// already available through the browser canvas path, so it terminates
// named-face lookup instead of skipping ahead in the stack.
// `bundledTypefaces` is optional so a caller that predates the bundled registry
// still resolves imported + system.
export function opCkResolveRegisteredTypeface(family, importedTypefaces, systemTypefacesByFamily, bundledTypefaces) {
  for (const candidate of opCkParseFontFamilyStack(family)) {
    const familyKey = opCkNormalizeFontFamilyName(candidate);
    if (!familyKey) continue;
    if (CSS_GENERIC_FONT_FAMILIES.has(familyKey)) return null;
    const imported = importedTypefaces.get(familyKey);
    if (imported) {
      return { key: `imported:${familyKey}`, familyKey, source: 'imported', tf: imported.tf };
    }
    const system = systemTypefacesByFamily.get(familyKey);
    if (system) {
      return { key: `system:${familyKey}`, familyKey, source: 'system', tf: system.tf };
    }
    const bundled = bundledTypefaces && bundledTypefaces.get(familyKey);
    if (bundled) {
      return { key: `bundled:${familyKey}`, familyKey, source: 'bundled', tf: bundled.tf };
    }
  }
  return null;
}

const opCkClampUnit = (value, fallback = 0) => {
  const number = Number(value);
  return Number.isFinite(number) ? Math.max(0, Math.min(1, number)) : fallback;
};

// Cache successful and failed RuntimeEffect compilations by exact SkSL source.
// The cache lives for one CanvasKit bridge session, so repainting never invokes
// the SkSL compiler again for a source it has already seen.
export function opCkCreateRuntimeEffectCache(CK) {
  const entries = new Map();
  return {
    get(sksl) {
      const source = String(sksl);
      if (entries.has(source)) return entries.get(source);

      let effect = null;
      let entry = null;
      try {
        effect = CK.RuntimeEffect.Make(source, () => {});
        if (effect) {
          const reflected = new Map();
          const uniformCount = Math.max(0, Math.trunc(Number(effect.getUniformCount()) || 0));
          for (let index = 0; index < uniformCount; index++) {
            reflected.set(effect.getUniformName(index), effect.getUniform(index));
          }
          entry = {
            effect,
            reflected,
            floatCount: Math.max(0, Math.trunc(Number(effect.getUniformFloatCount()) || 0)),
            children: opCkShaderChildren(source),
            childTags: opCkShaderChildTags(source),
          };
        }
      } catch (_error) {
        if (effect && effect.delete) effect.delete();
        entry = null;
      }
      entries.set(source, entry);
      return entry;
    },
  };
}

// Image children (#9): the names declared `uniform shader <name>;` in source
// order, outside comments. CanvasKit binds children by position and exposes
// no child reflection, so this mirrors `shader_children` in
// `de_editor_core::shader_images`; keep the two in step.
export function opCkShaderChildren(sksl) {
  const code = String(sksl).replace(/\/\*[\s\S]*?(?:\*\/|$)|\/\/[^\n]*/g, ' ');
  const names = [];
  for (const match of code.matchAll(/\buniform\s+shader\s+([A-Za-z_][A-Za-z0-9_]*)\b/g)) {
    names.push(match[1]);
  }
  return names;
}

// Shader textures (#165): `@sdf` / `@mipmap` in the doc comment right in
// front of each `uniform shader <name>;`, by name. Mirrors
// `de_editor_core::shader_textures::shader_child_specs`; keep in step.
export function opCkShaderChildTags(sksl) {
  const tags = new Map();
  const source = String(sksl);
  // A doc comment never spans past its own `*/`.
  const pattern = /(\/\*(?:(?!\*\/)[\s\S])*\*\/|(?:\/\/[^\n]*\n\s*)+)?\s*uniform\s+shader\s+([A-Za-z_][A-Za-z0-9_\s,]*)/g;
  for (const match of source.matchAll(pattern)) {
    const doc = match[1] || '';
    const tag = { sdf: /@sdf\b/.test(doc), mipmap: /@mipmap\b/.test(doc) };
    // Every name of `uniform shader a, b;` shares the comment.
    for (const name of match[2].split(',').map((n) => n.trim()).filter(Boolean)) {
      tags.set(name, tag);
    }
  }
  return tags;
}

// The kind a frame texture id carries in its high word (`(lo, hi)` u32
// halves), mirroring `de_editor_core::shader_textures::texture_kind`:
// 0 object, 1 background, 2 sdf; null for any other id.
export function opCkTextureKind(hi) {
  if ((hi >>> 28) !== 0xD) return null;
  const kind = (hi >>> 26) & 3;
  return kind <= 2 ? kind : null;
}

// The id an effect binds its `@sdf` child to: the pass's own capture.
export const OP_CK_OBJECT_SDF_TEXTURE = { lo: 0, hi: 0xD8000000 };

// Longest edge an SDF is computed at (`SDF_MAX_EDGE` in Rust).
const OP_CK_SDF_MAX_EDGE = 512;

// The signed distance field of an RGBA8 raster's alpha (edge at 50%), as
// RGBA floats: distance in texels (positive inside, less half a texel),
// then the unit gradient of increasing distance, then 1. The raster is
// sampled down to `OP_CK_SDF_MAX_EDGE` first; `factor` is how much.
// Mirrors `de_editor_core::shader_textures::signed_distance_field`.
export function opCkSignedDistanceField(rgba, width, height) {
  const step = Math.max(1, Math.ceil(Math.max(width, height) / OP_CK_SDF_MAX_EDGE));
  const w = Math.ceil(width / step);
  const h = Math.ceil(height / step);
  // One transparent texel around: past the raster is outside.
  const pw = w + 2;
  const ph = h + 2;
  const inside = new Uint8Array(pw * ph);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      let sum = 0;
      let n = 0;
      for (let sy = y * step; sy < Math.min((y + 1) * step, height); sy++) {
        for (let sx = x * step; sx < Math.min((x + 1) * step, width); sx++) {
          sum += rgba[(sy * width + sx) * 4 + 3];
          n++;
        }
      }
      inside[(y + 1) * pw + x + 1] = Math.floor(sum / Math.max(n, 1)) >= 128 ? 1 : 0;
    }
  }
  const transform1d = (f, d, n) => {
    const v = new Int32Array(n);
    const z = new Float64Array(n + 1);
    let k = 0;
    z[0] = -Infinity;
    z[1] = Infinity;
    const cross = (q, p) => ((f[q] + q * q) - (f[p] + p * p)) / (2 * q - 2 * p);
    for (let q = 1; q < n; q++) {
      let s = cross(q, v[k]);
      while (k > 0 && s <= z[k]) { k--; s = cross(q, v[k]); }
      k++;
      v[k] = q;
      z[k] = s;
      z[k + 1] = Infinity;
    }
    k = 0;
    for (let q = 0; q < n; q++) {
      while (z[k + 1] < q) k++;
      const dq = q - v[k];
      d[q] = dq * dq + f[v[k]];
    }
  };
  const squared = (target) => {
    const grid = new Float64Array(pw * ph);
    for (let i = 0; i < pw * ph; i++) grid[i] = inside[i] === target ? 0 : 1e20;
    const line = new Float64Array(Math.max(pw, ph));
    const out = new Float64Array(Math.max(pw, ph));
    for (let x = 0; x < pw; x++) {
      for (let y = 0; y < ph; y++) line[y] = grid[y * pw + x];
      transform1d(line, out, ph);
      for (let y = 0; y < ph; y++) grid[y * pw + x] = out[y];
    }
    for (let y = 0; y < ph; y++) {
      for (let x = 0; x < pw; x++) line[x] = grid[y * pw + x];
      transform1d(line, out, pw);
      for (let x = 0; x < pw; x++) grid[y * pw + x] = out[x];
    }
    return grid;
  };
  const toOutside = squared(0);
  const toInside = squared(1);
  const dist = new Float64Array(w * h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const p = (y + 1) * pw + x + 1;
      dist[y * w + x] = inside[p] ? Math.sqrt(toOutside[p]) - 0.5 : -(Math.sqrt(toInside[p]) - 0.5);
    }
  }
  const field = new Float32Array(w * h * 4);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const l = Math.max(x - 1, 0); const r = Math.min(x + 1, w - 1);
      const t = Math.max(y - 1, 0); const b = Math.min(y + 1, h - 1);
      const gx = (dist[y * w + r] - dist[y * w + l]) / Math.max(r - l, 1);
      const gy = (dist[b * w + x] - dist[t * w + x]) / Math.max(b - t, 1);
      const len = Math.hypot(gx, gy);
      const at = (y * w + x) * 4;
      field[at] = dist[y * w + x];
      field[at + 1] = len > 1e-6 ? gx / len : 0;
      field[at + 2] = len > 1e-6 ? gy / len : 0;
      field[at + 3] = 1;
    }
  }
  return { field, width: w, height: h, factor: step };
}

// A CanvasKit float image of `image`'s SDF, distances in units of
// `texelScale` texels, with the factor it was sampled down by
// (`{ image, factor }`); null when CanvasKit refuses.
export function opCkMakeSdfImage(CK, image, texelScale) {
  const width = image.width();
  const height = image.height();
  const pixels = image.readPixels(0, 0, {
    width, height,
    colorType: CK.ColorType.RGBA_8888,
    alphaType: CK.AlphaType.Unpremul,
    colorSpace: CK.ColorSpace.SRGB,
  });
  if (!pixels || width <= 0 || height <= 0) return null;
  const { field, width: w, height: h, factor } = opCkSignedDistanceField(pixels, width, height);
  const unit = factor / Math.max(texelScale, 1e-6);
  for (let i = 0; i < field.length; i += 4) field[i] *= unit;
  const made = CK.MakeImage({
    width: w, height: h,
    colorType: CK.ColorType.RGBA_F32,
    alphaType: CK.AlphaType.Unpremul,
    colorSpace: CK.ColorSpace.SRGB,
    // `MakeImage` copies bytes: hand it the floats' bytes, not the floats.
  }, new Uint8Array(field.buffer, field.byteOffset, field.byteLength), w * 16);
  return made ? { image: made, factor } : null;
}

// The paint image id a child uniform carries as four 16-bit chunks, split
// into the `(lo, hi)` u32 pair the image caches key on; null when the values
// are not an encoded id (`de_editor_core::shader_images::decode_image_id`).
export function opCkDecodeShaderImageId(values) {
  if (!values || values.length !== 4) return null;
  const chunks = Array.from(values, Number);
  if (!chunks.every((chunk) => Number.isInteger(chunk) && chunk >= 0 && chunk <= 0xffff)) {
    return null;
  }
  return { lo: chunks[0] + chunks[1] * 0x10000, hi: chunks[2] + chunks[3] * 0x10000 };
}

// One child shader per declared image child: the resident image stretched
// over the `boxW` x `boxH` fragCoord box with clamped edges, else transparent
// so the program still runs while the image decodes. `imageFor(lo, hi)`
// returns the cached CanvasKit image or null. The caller deletes the shaders.
export function opCkMakeShaderImageChildren(
  CK,
  entry,
  uniformNames,
  uniforms,
  uniformArities,
  boxW,
  boxH,
  imageFor,
  total = null,
  local = null,
) {
  // A frame texture maps from the device pixels it was made at (#165).
  const program = total && local ? opCkMat3Invert(opCkMat3Multiply(total, local)) : null;
  const bindings = new Map();
  let cursor = 0;
  for (let index = 0; index < uniformNames.length; index++) {
    const rawArity = Number(uniformArities[index]);
    const arity = Number.isSafeInteger(rawArity) && rawArity >= 0 ? rawArity : 0;
    const name = String(uniformNames[index]);
    if (!bindings.has(name) && cursor + arity <= uniforms.length) {
      bindings.set(name, uniforms.slice(cursor, cursor + arity));
    }
    cursor += arity;
  }
  return entry.children.map((child) => {
    const id = opCkDecodeShaderImageId(bindings.get(child));
    // `imageFor` returns an image, or a frame texture `{ image, kind }`
    // (#165), whose kind picks the tile mode.
    const found = id && imageFor ? imageFor(id.lo, id.hi) : null;
    const texture = found && found.image ? found : null;
    const image = texture ? texture.image : found;
    const imageW = image ? image.width() : 0;
    const imageH = image ? image.height() : 0;
    if (image && imageW > 0 && imageH > 0 && boxW > 0 && boxH > 0) {
      const matrix = texture && program
        ? opCkMat3Multiply(program, [texture.texel, 0, texture.dl, 0, texture.texel, texture.dt, 0, 0, 1])
        : [boxW / imageW, 0, 0, 0, boxH / imageH, 0, 0, 0, 1];
      const shader = image.makeShaderOptions(
        opCkTextureTile(CK, texture ? texture.kind : null),
        opCkTextureTile(CK, texture ? texture.kind : null),
        CK.FilterMode.Linear,
        opCkChildMipmap(CK, entry, child),
        matrix,
      );
      if (shader) return shader;
    }
    return CK.Shader.MakeColor(CK.TRANSPARENT, CK.ColorSpace.SRGB);
  });
}

// Tile mode past a texture's edge (`tile_mode` in the native backend): the
// backdrop mirrors, the node is transparent, a field or image clamps.
export function opCkTextureTile(CK, kind) {
  if (kind === 1) return CK.TileMode.Mirror;
  if (kind === 0) return CK.TileMode.Decal;
  return CK.TileMode.Clamp;
}

// Whether child `name` asks for mipmaps (`@mipmap`).
export function opCkChildMipmap(CK, entry, name) {
  const tags = entry.childTags && entry.childTags.get(name);
  return tags && tags.mipmap ? CK.MipmapMode.Linear : CK.MipmapMode.None;
}

// Bind the caller's name/value slices against CanvasKit's reflected uniform
// slots. Unknown names and arity mismatches are ignored like native Skia.
// A program with image children needs one shader per child (`children`);
// without them it fails and the caller paints the fallback.
export function opCkMakeRuntimeShader(
  entry,
  uniformNames,
  uniforms,
  uniformArities,
  localMatrix = null,
  children = null,
) {
  if (!entry) return null;
  try {
    const data = new Float32Array(entry.floatCount);
    let cursor = 0;
    for (let index = 0; index < uniformNames.length; index++) {
      const rawArity = Number(uniformArities[index]);
      const arity = Number.isSafeInteger(rawArity) && rawArity >= 0 ? rawArity : 0;
      const valuesStart = cursor;
      cursor += arity;
      const info = entry.reflected.get(String(uniformNames[index]));
      const expected = info ? Number(info.columns) * Number(info.rows) : 0;
      const slot = info ? Number(info.slot) : -1;
      // Integer uniforms (#165) travel as floats too: CanvasKit's
      // makeShader casts each `int*` slot from float before binding, so
      // the value only needs to be whole.
      if (!info || expected !== arity
          || !Number.isSafeInteger(slot) || slot < 0
          || slot + arity > data.length || valuesStart + arity > uniforms.length) {
        continue;
      }
      for (let value = 0; value < arity; value++) {
        const raw = uniforms[valuesStart + value];
        // Half away from zero, like Rust's `f32::round` on native.
        data[slot + value] = info.isInteger ? Math.sign(raw) * Math.round(Math.abs(raw)) : raw;
      }
    }
    if (entry.children && entry.children.length > 0) {
      if (!children || children.length !== entry.children.length) return null;
      return entry.effect.makeShaderWithChildren(data, children, localMatrix) || null;
    }
    return entry.effect.makeShader(data, localMatrix) || null;
  } catch (_error) {
    return null;
  }
}

// RuntimeEffect coordinates are local to the painted node. Preset shaders
// carry the document-space node size as an exact float2 uniform, so scale that
// coordinate space to the current rect as well as anchoring its origin. The
// cursor rules intentionally mirror uniform binding above: malformed arities
// consume no values, while unknown uniforms still advance by their arity.
export function opCkRuntimeEffectLocalMatrix(
  uniformNames,
  uniforms,
  uniformArities,
  x,
  y,
  w,
  h,
) {
  let cursor = 0;
  let size = null;
  for (let index = 0; index < uniformNames.length; index++) {
    const rawArity = Number(uniformArities[index]);
    const arity = Number.isSafeInteger(rawArity) && rawArity >= 0 ? rawArity : 0;
    const valuesStart = cursor;
    cursor += arity;
    if (size || String(uniformNames[index]) !== 'size' || arity !== 2
        || valuesStart + arity > uniforms.length) {
      continue;
    }
    size = [Number(uniforms[valuesStart]), Number(uniforms[valuesStart + 1])];
  }
  if (size && Number.isFinite(size[0]) && size[0] > 0
      && Number.isFinite(size[1]) && size[1] > 0) {
    return [w / size[0], 0, x, 0, h / size[1], y, 0, 0, 1];
  }
  return [1, 0, x, 0, 1, y, 0, 0, 1];
}

// `CK` and `imageFor` are only needed for programs with image children.
export function opCkMakeRuntimeShaderForRect(
  entry,
  uniformNames,
  uniforms,
  uniformArities,
  x,
  y,
  w,
  h,
  CK = null,
  imageFor = null,
  total = null,
) {
  const localMatrix = opCkRuntimeEffectLocalMatrix(uniformNames, uniforms, uniformArities, x, y, w, h);
  if (!entry || !entry.children || entry.children.length === 0 || !CK) {
    return opCkMakeRuntimeShader(entry, uniformNames, uniforms, uniformArities, localMatrix);
  }
  // The fragCoord box: the document-space `size` the local matrix scales
  // from (its x scale is w / size.x), else the rect itself.
  const boxW = w / localMatrix[0];
  const boxH = h / localMatrix[4];
  const children = opCkMakeShaderImageChildren(
    CK, entry, uniformNames, uniforms, uniformArities, boxW, boxH, imageFor, total, localMatrix,
  );
  try {
    return opCkMakeRuntimeShader(entry, uniformNames, uniforms, uniformArities, localMatrix, children);
  } finally {
    for (const child of children) if (child && child.delete) child.delete();
  }
}

// Row-major 3x3 matrix product `a * b`.
export function opCkMat3Multiply(a, b) {
  const out = new Array(9);
  for (let r = 0; r < 3; r++) {
    for (let c = 0; c < 3; c++) {
      out[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
    }
  }
  return out;
}

// Inverse of a row-major 3x3 matrix, or null when it is singular.
export function opCkMat3Invert(m) {
  const [a, b, c, d, e, f, g, h, i] = Array.from(m, Number);
  const A = e * i - f * h;
  const B = -(d * i - f * g);
  const C = d * h - e * g;
  const det = a * A + b * B + c * C;
  if (!Number.isFinite(det) || Math.abs(det) < 1e-12) return null;
  return [
    A / det, -(b * i - c * h) / det, (b * f - c * e) / det,
    B / det, (a * i - c * g) / det, -(a * f - c * d) / det,
    C / det, -(a * h - b * g) / det, (a * e - b * d) / det,
  ];
}

// Integer device-pixel box `[l, t, r, b]` covering the rect `x, y, w, h`
// under the total matrix `m`, clamped to a `maxW` x `maxH` surface; null
// when nothing is left.
export function opCkDeviceBounds(m, x, y, w, h, maxW, maxH) {
  const box = opCkDeviceBox(m, x, y, w, h);
  if (!box) return null;
  const clamped = [
    Math.max(0, box[0]), Math.max(0, box[1]), Math.min(maxW, box[2]), Math.min(maxH, box[3]),
  ];
  return clamped[2] > clamped[0] && clamped[3] > clamped[1] ? clamped : null;
}

// The integer device-pixel box `[l, t, r, b]` covering the rect, not
// clamped (a shader texture keeps the part off the surface); null when
// degenerate.
export function opCkDeviceBox(m, x, y, w, h) {
  let l = Infinity;
  let t = Infinity;
  let r = -Infinity;
  let b = -Infinity;
  for (const [px, py] of [[x, y], [x + w, y], [x, y + h], [x + w, y + h]]) {
    const z = m[6] * px + m[7] * py + m[8];
    if (!(Math.abs(z) > 1e-12)) return null;
    const dx = (m[0] * px + m[1] * py + m[2]) / z;
    const dy = (m[3] * px + m[4] * py + m[5]) / z;
    l = Math.min(l, dx); t = Math.min(t, dy); r = Math.max(r, dx); b = Math.max(b, dy);
  }
  const box = [Math.floor(l), Math.floor(t), Math.ceil(r), Math.ceil(b)];
  if (!box.every(Number.isFinite) || box[2] <= box[0] || box[3] <= box[1]) return null;
  return box;
}

// Shader effects (#6): the local matrix of a texture captured in device
// pixels whose top-left is device `(dl, dt)`, as a child of a program with
// local matrix `local` drawn under total matrix `total`. Maps image pixels
// into the program's node-local space: local^-1 * total^-1 * T(dl, dt).
export function opCkShaderEffectChildMatrix(total, local, dl, dt) {
  const inverse = opCkMat3Invert(opCkMat3Multiply(total, local));
  if (!inverse) return null;
  return opCkMat3Multiply(inverse, [1, 0, dl, 0, 1, dt, 0, 0, 1]);
}

// Shader-effect child names (#6); mirror `OBJECT_CHILD` / `BACKGROUND_CHILD`
// in `de_editor_core::shader_effects`; keep the two in step.
export const OP_CK_EFFECT_OBJECT_CHILD = 'object';
export const OP_CK_EFFECT_BACKGROUND_CHILD = 'background';

// Draw one shader-effect pass (#6) over the node box on `canvas` with
// `childName` bound to `image` (device pixels starting at `dl, dt`); the
// program's other children read transparent. `pass` carries the cache
// `entry`, the rect and the flattened uniforms. Returns false when
// CanvasKit refuses the program.
export function opCkDrawShaderEffectPass(CK, canvas, pass, image, childName, dl, dt, imageFor = null) {
  const { entry, x, y, w, h, names, values, arities } = pass;
  const local = opCkRuntimeEffectLocalMatrix(names, values, arities, x, y, w, h);
  const total = canvas.getTotalMatrix();
  const childMatrix = opCkShaderEffectChildMatrix(total, local, dl, dt);
  if (!childMatrix) return false;
  // Every other child (#165): the backdrop / images through their
  // uniforms, an `@sdf` child bound to the pass's capture as its field.
  const others = opCkMakeShaderImageChildren(
    CK, entry, names, values, arities, w / local[0], h / local[4], imageFor, total, local,
  );
  const bindings = new Map();
  let cursor = 0;
  for (let index = 0; index < names.length; index++) {
    const arity = Number(arities[index]) || 0;
    bindings.set(String(names[index]), values.slice(cursor, cursor + arity));
    cursor += arity;
  }
  let sdf = null;
  const children = entry.children.map((name, index) => {
    if (name === childName) {
      if (others[index] && others[index].delete) others[index].delete();
      return image.makeShaderOptions(
        CK.TileMode.Decal, CK.TileMode.Decal, CK.FilterMode.Linear,
        opCkChildMipmap(CK, entry, name), childMatrix,
      );
    }
    const id = opCkDecodeShaderImageId(bindings.get(name));
    const objectSdf = id && id.lo === OP_CK_OBJECT_SDF_TEXTURE.lo
      && id.hi === OP_CK_OBJECT_SDF_TEXTURE.hi;
    if (!objectSdf) return others[index];
    if (others[index] && others[index].delete) others[index].delete();
    if (!sdf) {
      const scale = Math.sqrt(Math.abs(total[0] * total[4] - total[1] * total[3]) * local[0] * local[4]);
      sdf = opCkMakeSdfImage(CK, image, scale);
    }
    const factor = sdf ? sdf.factor : 1;
    return sdf
      ? sdf.image.makeShaderOptions(
        CK.TileMode.Clamp, CK.TileMode.Clamp, CK.FilterMode.Linear, CK.MipmapMode.None,
        opCkMat3Multiply(childMatrix, [factor, 0, 0, 0, factor, 0, 0, 0, 1]),
      )
      : CK.Shader.MakeColor(CK.TRANSPARENT, CK.ColorSpace.SRGB);
  });
  let shader = null;
  try {
    shader = opCkMakeRuntimeShader(entry, names, values, arities, local, children);
  } finally {
    for (const child of children) if (child && child.delete) child.delete();
    if (sdf && sdf.image.delete) sdf.image.delete();
  }
  if (!shader) return false;
  const paint = new CK.Paint();
  paint.setShader(shader);
  try {
    canvas.drawRect(CK.XYWHRect(x, y, w, h), paint);
  } finally {
    paint.delete();
    if (shader.delete) shader.delete();
  }
  return true;
}

// Build the row-major vertex lattice consumed by CanvasKit.MakeVertices.
// Kept pure + exported so the geometry/index contract can run under Node
// without booting a WebGL surface. CanvasKit indices are u16, so grids larger
// than 65,535 vertices take the same visible first-colour fallback as malformed
// grids instead of wrapping indices and drawing corrupt triangles.
export function opCkBuildMeshGradientData(x, y, w, h, rows, cols, colors) {
  rows = Math.trunc(Number(rows));
  cols = Math.trunc(Number(cols));
  const vertexCount = rows * cols;
  if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(w) || !Number.isFinite(h)
      || rows < 2 || cols < 2 || !Number.isSafeInteger(vertexCount)
      || vertexCount > 0xffff || colors.length !== vertexCount * 4) {
    return null;
  }

  const positions = new Float32Array(vertexCount * 2);
  const rgba = new Float32Array(vertexCount * 4);
  for (let row = 0; row < rows; row++) {
    for (let colIndex = 0; colIndex < cols; colIndex++) {
      const vertex = row * cols + colIndex;
      positions[vertex * 2] = x + (colIndex / (cols - 1)) * w;
      positions[vertex * 2 + 1] = y + (row / (rows - 1)) * h;
      const colorOffset = vertex * 4;
      rgba[colorOffset] = opCkClampUnit(colors[colorOffset]);
      rgba[colorOffset + 1] = opCkClampUnit(colors[colorOffset + 1]);
      rgba[colorOffset + 2] = opCkClampUnit(colors[colorOffset + 2]);
      rgba[colorOffset + 3] = opCkClampUnit(colors[colorOffset + 3]);
    }
  }

  const indices = new Uint16Array((rows - 1) * (cols - 1) * 6);
  let index = 0;
  for (let row = 0; row < rows - 1; row++) {
    for (let colIndex = 0; colIndex < cols - 1; colIndex++) {
      const topLeft = row * cols + colIndex;
      const topRight = topLeft + 1;
      const bottomLeft = (row + 1) * cols + colIndex;
      const bottomRight = bottomLeft + 1;
      indices.set([topLeft, topRight, bottomLeft, topRight, bottomRight, bottomLeft], index);
      index += 6;
    }
  }
  return { positions, rgba, indices };
}

// Initialise CanvasKit on `canvasId`. Returns a bridge object the Rust backend
// drives. Text is rasterized with browser/system fonts by default; the Rust
// side can additionally register Local Font Access faces through
// registerSystemFont().
export async function opCkInit(canvasId) {
  await loadScript('/canvaskit/canvaskit.js');
  const CK = await CanvasKitInit({ locateFile: (f) => '/canvaskit/' + f });
  if (typeof createWebImageCaches !== 'function') {
    throw new Error('CanvasKit image cache factory was not configured');
  }
  let surface = CK.MakeWebGLCanvasSurface(canvasId);
  if (!surface) throw new Error('CanvasKit: MakeWebGLCanvasSurface returned null');
  let canvas = surface.getCanvas();
  // The surface `canvas` draws into: the window surface, or the offscreen
  // one an object shader-effect pass redirects into (#6).
  let drawSurface = surface;
  const effectRedirects = [];
  // Offscreen surfaces for object passes, one per nesting depth, reused
  // across frames and grown on demand (an animated effect would otherwise
  // allocate a GPU surface every frame).
  const effectScratch = [];
  const scratchSurface = (depth, w, h) => {
    const held = effectScratch[depth];
    if (held && held.w >= w && held.h >= h) return held.surface;
    if (held) held.surface.delete();
    const made = drawSurface.makeSurface({
      width: w,
      height: h,
      colorType: CK.ColorType.RGBA_8888,
      alphaType: CK.AlphaType.Premul,
      colorSpace: CK.ColorSpace.SRGB,
    });
    effectScratch[depth] = made ? { surface: made, w, h } : undefined;
    return made;
  };
  // Point drawing back at the window surface, dropping any object pass a
  // broken paint left open.
  const dropEffectRedirects = () => {
    if (effectRedirects.length === 0) return;
    effectRedirects.length = 0;
    canvas = surface.getCanvas();
    drawSurface = surface;
  };
  const el = document.getElementById(canvasId);
  const runtimeEffects = opCkCreateRuntimeEffectCache(CK);
  // Shader textures of the frame (#165), keyed `lo:hi`.
  const frameTextures = new Map();
  const releaseFrameTexture = (key) => {
    const held = frameTextures.get(key);
    if (held && held.image && held.image.delete) held.image.delete();
    frameTextures.delete(key);
  };
  // `dl, dt`: the texture's top-left in the device pixels of the canvas it
  // was made on; `texel`: device pixels per texel. SDFs stay across frames
  // (32 at most, `SDF_CAP` in Rust); node and backdrop textures are
  // released by their paint and never evicted.
  const installFrameTexture = (lo, hi, image, dl, dt, texel) => {
    const kind = opCkTextureKind(hi);
    if (kind === null || !image) {
      if (image && image.delete) image.delete();
      return;
    }
    if (kind === 2 && [...frameTextures.values()].filter((t) => t.kind === 2).length >= 32) {
      for (const [key, held] of [...frameTextures]) if (held.kind === 2) releaseFrameTexture(key);
    }
    releaseFrameTexture(`${lo}:${hi}`);
    frameTextures.set(`${lo}:${hi}`, { image, kind, dl, dt, texel });
  };
  const shaderChildFor = (lo, hi) => frameTextures.get(`${lo}:${hi}`) || imageCaches.fullImage(lo, hi);
  const drawShaderEffectPass = (pass, image, childName, dl, dt) =>
    opCkDrawShaderEffectPass(CK, canvas, pass, image, childName, dl, dt, shaderChildFor);
  // Close the object pass on top of `effectRedirects`: snapshot what the
  // node painted offscreen and draw the program over it on the parent.
  const finishObjectShaderEffect = () => {
    const pass = effectRedirects.pop();
    // The pooled surface can be larger than the box: snapshot the box.
    const image = pass.surface.makeImageSnapshot(Int32Array.of(0, 0, pass.cw, pass.ch));
    pass.canvas.restoreToCount(1);
    canvas = pass.parentCanvas;
    drawSurface = pass.parentSurface;
    try {
      if (image && !drawShaderEffectPass(pass, image, pass.childName, pass.dl, pass.dt)) {
        // Keep the node visible when the program can't be built.
        canvas.save();
        canvas.resetMatrix();
        canvas.drawImage(image, pass.dl, pass.dt, null);
        canvas.restore();
      }
    } finally {
      if (image && image.delete) image.delete();
    }
  };


  const systemTypefaces = [];
  const systemTypefaceKeys = new Set();
  const systemTypefacesByFamily = new Map();
  // User-imported font faces, keyed by normalized family name -> { tf, family }.
  // A named family is a deliberate single-typeface choice (mirrors how native
  // resolves a named family via FontMgr before any script fallback), so
  // imported text shapes the WHOLE run with one typeface instead of
  // re-segmenting by script.
  const importedTypefaces = new Map();
  // App-bundled OFL design faces, same shape as `importedTypefaces` but ranked
  // below system fonts: the browser has no bundled fonts of its own, so these
  // are fetched from the daemon at mount and registered here.
  const bundledTypefaces = new Map();
  const coverageCache = new Map();
  // Per-CHARACTER coverage segments for explicitly selected family text,
  // keyed on `(registry + family key, size, text)`. This used to
  // rebuild a throwaway CK.Font + call getGlyphIDs over the whole string on
  // EVERY drawText/measureTextFamilyStyled call, every frame — bounded FIFO
  // cache like `browserTextCache` below (values are plain JS arrays, no
  // wasm handles, so eviction is a plain `.delete()` on the Map).
  const REGISTERED_COVERAGE_CACHE_CAP = 512;
  const registeredCoverageCache = new Map();
  // Per-(registry + family key, size) CK.Font instances shared by the
  // family-aware draw + measure paths (`registeredFamilyFont`). Bounded,
  // full-wipe eviction mirroring `svgPathCache` below (these ARE wasm
  // handles and must be `.delete()`d before the Map entry is dropped).
  const REGISTERED_FONT_CACHE_CAP = 512;
  const registeredFamilyFontCache = new Map();
  // These caches key off the selected registry identity. Any imported or
  // system registry change invalidates them so later exact-name matches use
  // the newly registered face.
  const clearRegisteredFontCaches = () => {
    registeredCoverageCache.clear();
    for (const f of registeredFamilyFontCache.values()) f.delete();
    registeredFamilyFontCache.clear();
  };
  const browserTextCanvas = document.createElement('canvas');
  const browserTextCtx = browserTextCanvas.getContext('2d', { willReadFrequently: true });
  const browserTextCache = new Map();
  const clearBrowserTextCache = () => {
    for (const entry of browserTextCache.values()) {
      if (entry && entry.image && entry.image.delete) entry.image.delete();
    }
    browserTextCache.clear();
  };
  // Retain DPR so display changes can immediately invalidate stale handles;
  // per-draw supersampling comes from the current CanvasKit transform below.
  let textDpr = 1;
  const browserTextFontStack = [
    '-apple-system',
    'BlinkMacSystemFont',
    '"Segoe UI"',
    '"Helvetica Neue"',
    'Arial',
    '"Apple Color Emoji"',
    '"Segoe UI Emoji"',
    '"Noto Color Emoji"',
    '"PingFang SC"',
    '"PingFang TC"',
    '"Hiragino Sans"',
    '"Hiragino Kaku Gothic ProN"',
    '"Yu Gothic"',
    '"Yu Mincho"',
    '"Meiryo"',
    '"Apple SD Gothic Neo"',
    '"Malgun Gothic"',
    '"Noto Sans CJK SC"',
    '"Noto Sans CJK TC"',
    '"Noto Sans CJK JP"',
    '"Noto Sans CJK KR"',
    '"Kohinoor Devanagari"',
    '"Devanagari Sangam MN"',
    '"ITF Devanagari"',
    '"ITFDevanagari"',
    '"Mukta Mahee"',
    '"MuktaMahee"',
    '"Noto Sans Devanagari"',
    '"Nirmala UI"',
    '"Mangal"',
    '"SF Hebrew"',
    '"SFHebrew"',
    '"Arial Hebrew"',
    '"Geeza Pro"',
    '"Al Nile"',
    '"SF Georgian"',
    '"SFGeorgian"',
    '"Thonburi"',
    '"Sukhumvit Set"',
    '"SukhumvitSet"',
    '"Noto Sans Thai"',
    '"Noto Sans Thai UI"',
    '"Leelawadee UI"',
    '"Arial Unicode MS"',
    '"Noto Sans"',
    'sans-serif',
  ].join(', ');

  // CJK range check (Han / Hiragana / Katakana / Hangul / fullwidth).
  const hasCjk = (t) => { for (const ch of t) { const c = ch.codePointAt(0); if ((c >= 0x2e80 && c <= 0x9fff) || (c >= 0xac00 && c <= 0xd7a3) || (c >= 0xff00 && c <= 0xffef) || (c >= 0x3000 && c <= 0x30ff)) return true; } return false; };
  const hasSystemFallbackText = (t) => {
    for (const ch of t) {
      const c = ch.codePointAt(0);
      if (c > 0x7f) return true;
    }
    return false;
  };
  const normalizedFamily = opCkNormalizeFontFamilyName;
  const familyIncludes = (family, parts) => {
    const f = normalizedFamily(family);
    return parts.some((part) => f.includes(part.toLowerCase()));
  };
  const isEmojiFamily = (family) => {
    const f = normalizedFamily(family);
    return f.includes('emoji') || f.includes('color symbol');
  };
  const isCjkFamily = (family) => {
    return familyIncludes(family, ['PingFang', 'Hiragino Sans', 'Hiragino Kaku Gothic', 'Heiti', 'STHeiti', 'Songti', 'Noto Sans CJK', 'Noto Sans SC', 'Noto Sans TC', 'Noto Sans JP', 'Noto Sans KR', 'Source Han Sans', 'Microsoft YaHei', 'Microsoft JhengHei', 'SimHei', 'SimSun', 'Yu Gothic', 'Yu Mincho', 'Meiryo', 'Malgun Gothic', 'AppleGothic', 'Nanum Gothic', 'Apple SD Gothic Neo', 'Arial Unicode MS']);
  };
  const isTextFallbackFamily = (family) => {
    return isEmojiFamily(family) || isCjkFamily(family) || familyIncludes(family, ['Arial', 'Arial Unicode MS', 'Helvetica Neue', 'SF Pro', '.SF NS', 'Segoe UI', 'Segoe UI Historic', 'Segoe UI Symbol', 'Apple Symbols', 'Noto Sans', 'Kohinoor Devanagari', 'Devanagari Sangam MN', 'ITFDevanagari', 'ITF Devanagari', 'MuktaMahee', 'Mukta Mahee', 'Noto Sans Devanagari', 'Nirmala UI', 'Mangal', 'SFGeorgian', 'SF Georgian', 'SFHebrew', 'SF Hebrew', 'Thonburi', 'Sukhumvit', 'Noto Sans Thai', 'Leelawadee UI']);
  };
  // Emoji codepoint (pictographs / symbols / dingbats / regional) + attaching
  // modifiers (variation selector, ZWJ, keycap, skin tone) that extend a run.
  const isEmojiCp = (c) => (c >= 0x1f000 && c <= 0x1faff) || (c >= 0x2600 && c <= 0x27bf) || (c >= 0x2b00 && c <= 0x2bff) || (c >= 0x1f1e6 && c <= 0x1f1ff);
  const isEmojiMod = (c) => (c >= 0xfe00 && c <= 0xfe0f) || c === 0x200d || c === 0x20e3 || (c >= 0x1f3fb && c <= 0x1f3ff);
  // Split text into consecutive {text, emoji} runs so each draws with the right
  // typeface (CanvasKit drawText is single-typeface, no per-glyph fallback).
  const segments = (t) => {
    const out = []; let cur = '', curEmoji = null;
    for (const ch of t) {
      const c = ch.codePointAt(0);
      const e = isEmojiMod(c) ? (curEmoji === null ? false : curEmoji) : isEmojiCp(c);
      if (curEmoji === null) { curEmoji = e; cur = ch; }
      else if (e === curEmoji) { cur += ch; }
      else { out.push({ text: cur, emoji: curEmoji }); cur = ch; curEmoji = e; }
    }
    if (cur) out.push({ text: cur, emoji: curEmoji });
    return out;
  };

  const col = (r, g, b, a) => CK.Color4f(r, g, b, a);
  const isPaintStyle = (style) => Boolean(style && typeof style.value !== 'undefined');
  const setPaintStyle = (paint, style) => { if (isPaintStyle(style)) paint.setStyle(style); };
  // Long-lived fill/stroke paints reused across every plain-color primitive
  // draw (fillRect, fillRRect, ovals, lines, polygons, SVG path fills) —
  // a full-editor repaint issued hundreds of `new CK.Paint()` + `.delete()`
  // pairs per frame otherwise. Every `fillPaint`/`strokePaint` call resets
  // ALL mutable properties (color, antialias, style, stroke width/cap/join)
  // it ever sets, so no state leaks between unrelated call sites even though
  // a couple of callers (drawText) further mutate the returned paint (e.g.
  // StrokeAndFill for bold synth) after fetching it — the next fetch wipes
  // that back to a clean Fill/Stroke paint. Callers must NOT `.delete()`
  // these two objects. One-off-effect paints (shaders, blend modes, mask
  // filters/blur) are excluded from this cache by design — see
  // `shaderPaint` and `fillInnerShadowSvgPath`'s `cut` paint.
  const cachedFillPaint = new CK.Paint();
  const cachedStrokePaint = new CK.Paint();
  const fillPaint = (r, g, b, a) => { const p = cachedFillPaint; p.setColor(col(r, g, b, a)); p.setAntiAlias(true); setPaintStyle(p, CK.PaintStyle.Fill); return p; };
  // A node's authored cap / join / dash (#89), set by `setStrokeStyle`
  // while the Rust painter is inside that node and cleared after. Cap and
  // join arrive as indices into the tables below (`canvaskit::stroke_style`).
  let strokeStyle = null;
  const STROKE_CAPS = [CK.StrokeCap.Butt, CK.StrokeCap.Round, CK.StrokeCap.Square];
  const STROKE_JOINS = [CK.StrokeJoin.Miter, CK.StrokeJoin.Round, CK.StrokeJoin.Bevel];
  const strokePaint = (r, g, b, a, w) => {
    const p = cachedStrokePaint; p.setColor(col(r, g, b, a)); p.setAntiAlias(true); setPaintStyle(p, CK.PaintStyle.Stroke); p.setStrokeWidth(w); p.setStrokeCap(CK.StrokeCap.Round); p.setStrokeJoin(CK.StrokeJoin.Round);
    // `clearStrokeStyle` resets the path effect, so unstyled chrome
    // strokes skip these calls entirely.
    if (strokeStyle) {
      p.setPathEffect(strokeStyle.effect);
      p.setStrokeCap(STROKE_CAPS[strokeStyle.cap]);
      p.setStrokeJoin(STROKE_JOINS[strokeStyle.join]);
    }
    return p;
  };
  // Dedicated (allocated + `.delete()`d) fill paint for the one call site
  // that needs a SECOND live fill paint while `drawText`'s cached `p` (see
  // above) is already in scope: `drawScriptRun` is invoked both standalone
  // AND nested inside `drawText`'s per-segment loop, so it cannot safely
  // share the single cached fill paint without the outer and inner draws
  // fighting over the same object mid-loop. Kept as a plain allocation
  // rather than a second cache since text draws are far less frequent than
  // the primitive-shape hot path this task targets.
  const allocFillPaint = (r, g, b, a) => { const p = new CK.Paint(); p.setColor(col(r, g, b, a)); p.setAntiAlias(true); setPaintStyle(p, CK.PaintStyle.Fill); return p; };
  // Quote the authored family so names with spaces stay one CSS token, and
  // strip quote/backslash so a family name can't break out of the shorthand.
  // Only families the BROWSER knows resolve here; faces registered into
  // CanvasKit as raw bytes are invisible to CSS and fall through the stack.
  const cssFamilyToken = (family) => {
    const name = String(family || '').trim().replace(/["\\]/g, '');
    return name ? `"${name}", ` : '';
  };
  const browserTextFont = (sz, weight, italic, family) => `${italic ? 'italic ' : ''}${Math.max(100, Math.min(900, Math.round(weight || 400)))} ${Math.max(1, sz)}px ${cssFamilyToken(family)}${browserTextFontStack}`;
  const shouldUseBrowserTextFallback = (_t, _emojiRun) => Boolean(browserTextCtx);
  // Complex scripts (Arabic and friends — see `de_editor_core::text_script`,
  // which owns the predicate for BOTH hosts). CanvasKit's `canvas.drawText`
  // is a 1:1 cmap lookup: no bidi reordering, no contextual joining, so it
  // paints Arabic in storage order with isolated letterforms. The browser's
  // own 2D text engine does both correctly and `drawBrowserText` already
  // wraps it, so complex runs are sent there instead.
  //
  // They must go WHOLE. `drawScriptRun` splits a run into script segments and
  // advances left to right, so it would resolve bidi within each segment and
  // then lay the segments out in storage order — a mixed digit/Arabic run
  // like "50 ك.م" would still come out wrong.
  const shapedTextUnavailable = () => !browserTextCtx;
  const allSegmentsUseBrowserTextFallback = (segs) => segs.length > 0 && segs.every((seg) => shouldUseBrowserTextFallback(seg.text, seg.emoji));
  const browserTextMeasure = (t, sz, weight = 400, italic = false, family = '') => {
    if (!browserTextCtx) return 0;
    browserTextCtx.font = browserTextFont(sz, weight, italic, family);
    return browserTextCtx.measureText(t).width;
  };
  const effectiveTextScale = () => {
    const m = canvas.getTotalMatrix();
    const xScale = Math.hypot(m[0], m[3]);
    const yScale = Math.hypot(m[1], m[4]);
    return Math.max(1, xScale, yScale);
  };
  // Continuous zoom used to put the exact floating-point transform in every
  // browser-text cache key. Quantization lets adjacent frames reuse bitmaps.
  const browserTextRasterScale = () =>
    opCkQuantizeTextRasterScale(effectiveTextScale());
  // Text uses a white coverage mask keyed without colour/alpha, then receives
  // a SrcIn tint at draw time so differently coloured runs share a bitmap.
  // Emoji retain their legacy RGBA-keyed, untinted raster path because colour
  // glyphs can ignore fillStyle. Cache-hit reinsertion keeps eviction LRU.
  const browserTextImage = (t, sz, weight, italic, emoji, r, g, b, a, family = '') => {
    if (!browserTextCtx) return null;
    const ss = browserTextRasterScale();
    // `family` participates in the key: the same string shaped in two
    // different families is two different bitmaps.
    const key = emoji
      ? ['e', t, sz, weight, italic ? 1 : 0, r, g, b, a, ss, family].join('\n')
      : [t, sz, weight, italic ? 1 : 0, ss, family].join('\n');
    const hit = browserTextCache.get(key);
    if (hit) {
      browserTextCache.delete(key);
      browserTextCache.set(key, hit);
      return hit;
    }
    const font = browserTextFont(sz, weight, italic, family);
    browserTextCtx.font = font;
    const metrics = browserTextCtx.measureText(t);
    // Logical (CSS-px) box the glyphs occupy; positioning stays in CSS units.
    const width = Math.max(1, Math.ceil(metrics.width + 4));
    const ascent = Math.ceil(metrics.actualBoundingBoxAscent || sz * 0.8);
    const descent = Math.ceil(metrics.actualBoundingBoxDescent || sz * 0.25);
    const baseline = ascent + 2;
    const height = Math.max(1, baseline + descent + 2);
    // Back the offscreen canvas with `ss`x pixels and scale drawing so the
    // rasterized glyphs carry device-resolution detail. Setting canvas.width/
    // height resets the 2D context, so (re)apply font/baseline/fill + the
    // supersample transform here.
    browserTextCanvas.width = Math.max(1, Math.ceil(width * ss));
    browserTextCanvas.height = Math.max(1, Math.ceil(height * ss));
    browserTextCtx.setTransform(ss, 0, 0, ss, 0, 0);
    browserTextCtx.clearRect(0, 0, width, height);
    browserTextCtx.font = font;
    browserTextCtx.textBaseline = 'alphabetic';
    // Emoji bake their colour (legacy path); text rasters a WHITE mask that is
    // tinted at draw time.
    browserTextCtx.fillStyle = emoji
      ? `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${a})`
      : 'rgba(255, 255, 255, 1)';
    browserTextCtx.fillText(t, 2, baseline);
    browserTextCtx.setTransform(1, 0, 0, 1, 0, 0);
    const image = CK.MakeImageFromCanvasImageSource(browserTextCanvas);
    if (!image) return null;
    const entry = { image, width: metrics.width, baseline, ss, emoji: Boolean(emoji) };
    browserTextCache.set(key, entry);
    if (browserTextCache.size > 512) {
      const firstKey = browserTextCache.keys().next().value;
      const old = browserTextCache.get(firstKey);
      if (old && old.image && old.image.delete) old.image.delete();
      browserTextCache.delete(firstKey);
    }
    return entry;
  };
  // Bounded rgb→ColorFilter cache for tinting the WHITE text raster at draw.
  // `MakeBlend(color, SrcIn)` recolours the mask to `color × coverage`; alpha
  // is NOT part of the filter (it rides the paint via setAlphaf), so the key is
  // rgb only and one filter serves every opacity of a colour. Filters are wasm
  // handles — LRU with a hard cap, `.delete()`ing the evicted handle (mirrors
  // `svgPathCache`'s wasm-object hygiene). Cap is small: distinct text colours
  // on screen are few, so this never thrashes the way the old RGBA raster key
  // did. An evicted filter was already consumed by its draw call (immediate
  // mode), and `cachedTintPaint` re-`setColorFilter`s before every draw, so a
  // deleted handle is never dereferenced.
  const TINT_FILTER_CACHE_CAP = 64;
  const tintFilterCache = new Map();
  const tintColorFilter = (r, g, b) => {
    if (!(CK.ColorFilter && CK.ColorFilter.MakeBlend)) return null;
    const key = r + '\n' + g + '\n' + b;
    const hit = tintFilterCache.get(key);
    if (hit) {
      tintFilterCache.delete(key);
      tintFilterCache.set(key, hit);
      return hit;
    }
    const cf = CK.ColorFilter.MakeBlend(col(r, g, b, 1), CK.BlendMode.SrcIn);
    if (!cf) return null;
    tintFilterCache.set(key, cf);
    if (tintFilterCache.size > TINT_FILTER_CACHE_CAP) {
      const firstKey = tintFilterCache.keys().next().value;
      const old = tintFilterCache.get(firstKey);
      if (old && old.delete) old.delete();
      tintFilterCache.delete(firstKey);
    }
    return cf;
  };
  // Dedicated long-lived paint that tints the white text raster. Kept SEPARATE
  // from cachedFillPaint / cachedStrokePaint because drawBrowserText runs while
  // drawText's / drawScriptRun's fill paint `p` is already live in an outer
  // loop — sharing would corrupt that paint mid-run (the same reason
  // allocFillPaint exists). antiAlias stays at the CK.Paint default (false) so
  // image sampling matches the prior no-paint drawImage exactly.
  const cachedTintPaint = new CK.Paint();
  const drawBrowserText = (t, x, y, sz, weight, italic, r, g, b, a, emoji, family = '') => {
    const entry = browserTextImage(t, sz, weight, italic, emoji, r, g, b, a, family);
    if (!entry) return 0;
    // Emoji rasters bake their own colour and draw UNTINTED (legacy path);
    // text rasters are white masks tinted here. For text: tint to (r,g,b) via a
    // cached SrcIn ColorFilter with opacity `a` riding the paint — SrcIn keeps
    // alpha = coverage × a and rgb = colour, pixel-identical to the old
    // baked-colour raster.
    let paint = null;
    if (!entry.emoji) {
      paint = cachedTintPaint;
      paint.setColorFilter(tintColorFilter(r, g, b) || null);
      paint.setAlphaf(a < 0 ? 0 : a > 1 ? 1 : a);
    }
    const ss = entry.ss || 1;
    if (ss !== 1) {
      // The bitmap is `ss`x oversampled; place it in logical space at
      // (x-2, y-baseline) then scale down by `ss` so its device footprint
      // matches the intended CSS box at native resolution.
      canvas.save();
      canvas.translate(x - 2, y - entry.baseline);
      canvas.scale(1 / ss, 1 / ss);
      if (paint) canvas.drawImage(entry.image, 0, 0, paint);
      else canvas.drawImage(entry.image, 0, 0);
      canvas.restore();
    } else if (paint) {
      canvas.drawImage(entry.image, x - 2, y - entry.baseline, paint);
    } else {
      canvas.drawImage(entry.image, x - 2, y - entry.baseline);
    }
    return entry.width;
  };
  const shaderPaint = (shader) => { const p = new CK.Paint(); p.setAntiAlias(true); setPaintStyle(p, CK.PaintStyle.Fill); p.setShader(shader); return p; };
  const firstStopColor = (stops, opacity) => stops.length >= 5
    ? col(
      opCkClampUnit(stops[1]),
      opCkClampUnit(stops[2]),
      opCkClampUnit(stops[3]),
      opCkClampUnit(stops[4]) * opCkClampUnit(opacity),
    )
    : col(0, 0, 0, 0);
  const gradientStops = (stops, opacity) => {
    const colors = [];
    const offsets = [];
    const alpha = opCkClampUnit(opacity);
    for (let i = 0; i + 4 < stops.length; i += 5) {
      offsets.push(opCkClampUnit(stops[i]));
      colors.push(col(
        opCkClampUnit(stops[i + 1]),
        opCkClampUnit(stops[i + 2]),
        opCkClampUnit(stops[i + 3]),
        opCkClampUnit(stops[i + 4]) * alpha,
      ));
    }
    return { colors, offsets };
  };
  const linearGradientPoints = (x, y, w, h, angleDeg) => {
    const rad = (angleDeg - 90) * Math.PI / 180;
    const cx = x + w / 2;
    const cy = y + h / 2;
    const dx = Math.cos(rad) * w / 2;
    const dy = Math.sin(rad) * h / 2;
    return { start: [cx - dx, cy - dy], end: [cx + dx, cy + dy] };
  };
  const uniformRRect = (x, y, w, h, radius) => CK.RRectXY(
    CK.LTRBRect(x, y, x + w, y + h),
    Math.max(0, radius),
    Math.max(0, radius),
  );
  const perCornerRRect = (x, y, w, h, topLeft, topRight, bottomRight, bottomLeft) => Float32Array.of(
    x, y, x + w, y + h,
    Math.max(0, topLeft), Math.max(0, topLeft),
    Math.max(0, topRight), Math.max(0, topRight),
    Math.max(0, bottomRight), Math.max(0, bottomRight),
    Math.max(0, bottomLeft), Math.max(0, bottomLeft),
  );
  const drawGradientRRect = (rrect, shader, fallbackColor) => {
    if (!shader) {
      canvas.drawRRect(rrect, fillPaint(...fallbackColor));
      return;
    }
    const paint = shaderPaint(shader);
    try {
      canvas.drawRRect(rrect, paint);
    } finally {
      paint.delete();
      if (shader.delete) shader.delete();
    }
  };
  const drawRuntimeEffectRRect = (
    rrect,
    x,
    y,
    w,
    h,
    sksl,
    uniformNames,
    uniforms,
    uniformArities,
    opacity,
    fallback,
  ) => {
    const alpha = opCkClampUnit(opacity);
    const shader = opCkMakeRuntimeShaderForRect(
      runtimeEffects.get(sksl),
      uniformNames,
      uniforms,
      uniformArities,
      x,
      y,
      w,
      h,
      CK,
      shaderChildFor,
      canvas.getTotalMatrix(),
    );
    if (!shader) {
      canvas.drawRRect(rrect, fillPaint(
        opCkClampUnit(fallback[0]),
        opCkClampUnit(fallback[1]),
        opCkClampUnit(fallback[2]),
        opCkClampUnit(fallback[3]) * alpha,
      ));
      return;
    }
    const paint = shaderPaint(shader);
    paint.setAlphaf(alpha);
    try {
      canvas.drawRRect(rrect, paint);
    } finally {
      paint.delete();
      if (shader.delete) shader.delete();
    }
  };
  const makeLinearGradient = (x, y, w, h, stops, angleDeg, opacity) => {
    const gradient = gradientStops(stops, opacity);
    if (!gradient.colors.length) return null;
    const points = linearGradientPoints(x, y, w, h, angleDeg);
    try {
      // Flag 1 requests interpolation in premultiplied colour space, matching
      // the native Skia backend and avoiding halos around transparent stops.
      return CK.Shader.MakeLinearGradient(
        points.start,
        points.end,
        gradient.colors,
        gradient.offsets,
        CK.TileMode.Clamp,
        null,
        1,
      );
    } catch (_error) {
      return null;
    }
  };
  const makeRadialGradient = (x, y, w, h, stops, cxFrac, cyFrac, radiusFrac, opacity) => {
    const gradient = gradientStops(stops, opacity);
    if (!gradient.colors.length) return null;
    const center = [
      x + w * opCkClampUnit(cxFrac),
      y + h * opCkClampUnit(cyFrac),
    ];
    const radius = Math.max(0.01, Math.max(w, h) * opCkClampUnit(radiusFrac));
    try {
      return CK.Shader.MakeRadialGradient(
        center,
        radius,
        gradient.colors,
        gradient.offsets,
        CK.TileMode.Clamp,
        null,
        1,
      );
    } catch (_error) {
      return null;
    }
  };
  // Angular (conic) gradient. Skia's sweep starts at 3 o'clock; the
  // canonical rotation is CSS conic's (0deg = 12 o'clock, clockwise), so
  // the local matrix turns the sweep by rotation - 90deg about the centre.
  const makeAngularGradient = (x, y, w, h, stops, cxFrac, cyFrac, rotationDeg, opacity) => {
    const gradient = gradientStops(stops, opacity);
    if (!gradient.colors.length) return null;
    const cx = x + w * opCkClampUnit(cxFrac);
    const cy = y + h * opCkClampUnit(cyFrac);
    const rotation = Number.isFinite(rotationDeg) ? rotationDeg : 0;
    try {
      const local = CK.Matrix.rotated(((rotation - 90) * Math.PI) / 180, cx, cy);
      return CK.Shader.MakeSweepGradient(
        cx,
        cy,
        gradient.colors,
        gradient.offsets,
        CK.TileMode.Clamp,
        local,
        1,
        0,
        360,
      );
    } catch (_error) {
      return null;
    }
  };
  const drawMeshGradientRRect = (rrect, x, y, w, h, rows, cols, colors, opacity) => {
    const alpha = opCkClampUnit(opacity);
    const fallback = colors.length >= 4
      ? [
        opCkClampUnit(colors[0]),
        opCkClampUnit(colors[1]),
        opCkClampUnit(colors[2]),
        opCkClampUnit(colors[3]) * alpha,
      ]
      : [0, 0, 0, 0];
    const mesh = opCkBuildMeshGradientData(x, y, w, h, rows, cols, colors);
    if (!mesh || typeof CK.MakeVertices !== 'function') {
      canvas.drawRRect(rrect, fillPaint(...fallback));
      return;
    }

    let vertices = null;
    try {
      vertices = CK.MakeVertices(
        CK.VertexMode.Triangles,
        mesh.positions,
        null,
        mesh.rgba,
        mesh.indices,
        true,
      );
    } catch (_error) {
      vertices = null;
    }
    if (!vertices) {
      canvas.drawRRect(rrect, fillPaint(...fallback));
      return;
    }

    const paint = allocFillPaint(1, 1, 1, alpha);
    canvas.save();
    try {
      canvas.clipRRect(rrect, CK.ClipOp.Intersect, true);
      canvas.drawVertices(vertices, CK.BlendMode.Modulate, paint);
    } finally {
      canvas.restore();
      paint.delete();
      vertices.delete();
    }
  };
  const fontCovers = (entry, text) => {
    if (!entry || !entry.tf || !text) return false;
    const cacheKey = entry.key + '\n' + text;
    if (coverageCache.has(cacheKey)) return coverageCache.get(cacheKey);
    const f = new CK.Font(entry.tf, 16);
    const ids = f.getGlyphIDs(text);
    const ok = ids.length > 0 && ids.every((id) => id !== 0);
    f.delete();
    coverageCache.set(cacheKey, ok);
    return ok;
  };
  const systemTypefaceFor = (t, emojiRun) => {
    const preferred = emojiRun
      ? systemTypefaces.filter((entry) => entry.emoji)
      : hasCjk(t)
        ? systemTypefaces.filter((entry) => entry.cjk)
        : hasSystemFallbackText(t)
          ? systemTypefaces.filter((entry) => entry.textFallback)
          : [];
    for (const entry of preferred) {
      if (fontCovers(entry, t)) return entry.tf;
    }
    if (!emojiRun && hasSystemFallbackText(t)) {
      for (const entry of systemTypefaces) {
        if (fontCovers(entry, t)) return entry.tf;
      }
    }
    return null;
  };
  const tfFor = (t, emojiRun) => systemTypefaceFor(t, emojiRun) || CK.Typeface.GetDefault();
  const runWidth = (f, s) => { const ids = f.getGlyphIDs(s); return f.getGlyphWidths(ids).reduce((a, v) => a + v, 0); };
  // Normalize the primary family for imported-font registry mutations. Text
  // lookup itself uses every authored candidate via `familyTypefaceEntry`.
  const primaryFamilyKey = (family) => {
    const first = opCkParseFontFamilyStack(family)[0] || '';
    if (!first) return '';
    const key = normalizedFamily(first);
    return CSS_GENERIC_FONT_FAMILIES.has(key) ? '' : key;
  };
  // Resolve an explicit imported/system face in authored stack order. Generic
  // candidates intentionally return null and continue through browser text.
  const familyTypefaceEntry = (family) => {
    return opCkResolveRegisteredTypeface(family, importedTypefaces, systemTypefacesByFamily, bundledTypefaces);
  };
  // Shared "typeface + font for (family, sz)" so the family-aware draw and
  // measure paths build the SAME CK.Font and agree to sub-pixel. Cached per
  // `(registry + family key, size)` — this used to build + discard a
  // fresh CK.Font on every selected segment of every draw/measure call.
  // Iterations that use this font always fully consume it (draw or measure)
  // before the next one is fetched, so a single shared instance per key is
  // safe to reuse across loop iterations and across calls. Skew (italic) is
  // reset unconditionally on every fetch since the same cached Font may be
  // reused for an italic run and then a later upright run. Callers must NOT
  // `.delete()` the returned Font; eviction (mirroring `svgPathCache`) frees
  // every cached wasm Font before wiping the map. Invalidated wholesale by
  // `clearRegisteredFontCaches` whenever either font registry changes.
  const registeredFamilyFont = (key, tf, sz, italic) => {
    const cacheKey = key + '\n' + sz;
    let f = registeredFamilyFontCache.get(cacheKey);
    if (!f) {
      f = new CK.Font(tf, sz);
      if (registeredFamilyFontCache.size >= REGISTERED_FONT_CACHE_CAP) {
        for (const v of registeredFamilyFontCache.values()) v.delete();
        registeredFamilyFontCache.clear();
      }
      registeredFamilyFontCache.set(cacheKey, f);
    }
    f.setSkewX(italic ? -0.25 : 0);
    return f;
  };
  // Split a run into maximal {text, registered} segments by whether the named
  // typeface has a glyph for each char (glyph id 0 = .notdef = uncovered). This
  // is per-CHARACTER, so a mixed run keeps the selected face for the chars it
  // covers and falls back (system/CJK/emoji) only for the rest — matching how
  // native resolves a named family per character, instead of dropping the
  // imported family for the whole run. `getGlyphIDs` returns one id per
  // codepoint, so it aligns with the codepoint iteration; if the counts don't
  // line up we conservatively treat the whole run as uncovered.
  const registeredCoverageSegments = (key, tf, sz, t) => {
    const cacheKey = key + '\n' + sz + '\n' + t;
    const cached = registeredCoverageCache.get(cacheKey);
    if (cached) return cached;
    const cps = Array.from(t);
    let segs;
    if (cps.length === 0) {
      segs = [];
    } else {
      const f = new CK.Font(tf, sz);
      let ids = null;
      try {
        ids = f.getGlyphIDs(t);
      } catch (e) {
        ids = null;
      }
      f.delete();
      if (!ids || ids.length !== cps.length) {
        segs = [{ text: t, registered: false }];
      } else {
        const out = [];
        let cur = '';
        let curRegistered = null;
        for (let i = 0; i < cps.length; i++) {
          const registered = ids[i] !== 0;
          if (curRegistered === null) {
            curRegistered = registered;
            cur = cps[i];
          } else if (registered === curRegistered) {
            cur += cps[i];
          } else {
            out.push({ text: cur, registered: curRegistered });
            cur = cps[i];
            curRegistered = registered;
          }
        }
        if (cur) out.push({ text: cur, registered: curRegistered });
        segs = out;
      }
    }
    registeredCoverageCache.set(cacheKey, segs);
    if (registeredCoverageCache.size > REGISTERED_COVERAGE_CACHE_CAP) {
      const firstKey = registeredCoverageCache.keys().next().value;
      registeredCoverageCache.delete(firstKey);
    }
    return segs;
  };
  // Draw a run via the script-segmented fallback (system / CJK / emoji /
  // browser-canvas), returning the advance consumed. Shared by the
  // family-blind path and the uncovered segments of a family-aware run, so the
  // two stay identical. Mirrors `measureTextStyled` advance-for-advance.
  const drawScriptRun = (t, x, y, sz, weight, italic, r, g, b, a) => {
    const segs = segments(t);
    if (segs.length === 0) return 0;
    if (allSegmentsUseBrowserTextFallback(segs)) {
      let cx = x;
      for (const seg of segs) cx += drawBrowserText(seg.text, cx, y, sz, weight, italic, r, g, b, a, seg.emoji);
      return cx - x;
    }
    const p = allocFillPaint(r, g, b, a);
    if (weight >= 600 && isPaintStyle(CK.PaintStyle.StrokeAndFill)) {
      setPaintStyle(p, CK.PaintStyle.StrokeAndFill);
      p.setStrokeWidth(sz * 0.06);
    }
    let cx = x;
    for (const seg of segs) {
      if (shouldUseBrowserTextFallback(seg.text, seg.emoji)) {
        cx += drawBrowserText(seg.text, cx, y, sz, weight, italic, r, g, b, a, seg.emoji);
        continue;
      }
      const f = new CK.Font(tfFor(seg.text, seg.emoji), sz);
      if (italic && !seg.emoji) f.setSkewX(-0.25);
      canvas.drawText(seg.text, cx, y, p, f);
      cx += runWidth(f, seg.text);
      f.delete();
    }
    p.delete();
    return cx - x;
  };
  const pathIsFinite = (bounds) => bounds && bounds.length >= 4 && bounds.every((v) => Number.isFinite(v));
  const fitPathToRect = (path, x, y, w, h) => {
    if (!Number.isFinite(w) || !Number.isFinite(h) || w < 0 || h < 0) {
      path.transform(CK.Matrix.translated(x, y));
      return path;
    }
    // Match native Skia's `compute_tight_bounds`: control points may sit far
    // outside the actual Bezier curve, so loose bounds can visibly shrink or
    // offset a fitted path. Older CanvasKit builds lack the tight API; retain
    // `getBounds` as a compatibility fallback.
    let bounds = null;
    if (typeof path.computeTightBounds === 'function') {
      try { bounds = path.computeTightBounds(); } catch (_error) { bounds = null; }
    }
    if (!pathIsFinite(bounds)) {
      try { bounds = path.getBounds(); } catch (_error) { bounds = null; }
    }
    if (!pathIsFinite(bounds)) {
      path.transform(CK.Matrix.translated(x, y));
      return path;
    }
    const nativeW = bounds[2] - bounds[0];
    const nativeH = bounds[3] - bounds[1];
    // A flat path (straight horizontal / vertical line) has no extent on one
    // axis, so a zero box side there is expected and the other axis still
    // fits; only a zero side the path actually spans falls back.
    const degenerateSide = (side, native) => side === 0 && Math.abs(native) > 0.01;
    if (degenerateSide(w, nativeW) || degenerateSide(h, nativeH)) {
      path.transform(CK.Matrix.translated(x, y));
      return path;
    }
    const sx = Math.abs(nativeW) > 0.01 ? w / nativeW : 1;
    const sy = Math.abs(nativeH) > 0.01 ? h / nativeH : 1;
    const tx = x - bounds[0] * sx;
    const ty = y - bounds[1] * sy;
    path.transform(CK.Matrix.multiply(CK.Matrix.translated(tx, ty), CK.Matrix.scaled(sx, sy)));
    return path;
  };

  // Parsed-SVG-path cache. `CK.Path.MakeFromSVGString` is the dominant per-icon
  // cost, and every chrome icon / lucide glyph / brand logo / vector node
  // re-parsed its `d` string on EVERY frame (mirrors what the native backend
  // avoids via `svg_path_cache`). Cache the parsed, untransformed path keyed on
  // `d`; callers draw a `.copy()` they transform + delete, leaving the cached
  // original pristine. Bounded — on overflow drop all (a full refill is one
  // frame) and delete the wasm-heap paths so they don't leak.
  const SVG_PATH_CACHE_CAP = 1024;
  const svgPathCache = new Map();
  const cachedSvgPath = (d) => {
    let base = svgPathCache.get(d);
    if (!base) {
      base = CK.Path.MakeFromSVGString(d);
      if (!base) return null;
      if (svgPathCache.size >= SVG_PATH_CACHE_CAP) {
        for (const v of svgPathCache.values()) v.delete();
        svgPathCache.clear();
      }
      svgPathCache.set(d, base);
    }
    return base.copy();
  };

  const imageCaches = createWebImageCaches(CK);

  // Figma maps node-normalized coordinates to normalized image UV. Image
  // shaders consume the inverse, mapping image pixels into the destination
  // rect: node_rect * inverse(figma) * inverse(image_dimensions).
  const figmaImageLocalMatrix = (x, y, w, h, imageW, imageH, transform) => {
    if (transform.length !== 6 || !(w > 0) || !(h > 0) || !(imageW > 0) || !(imageH > 0)) return null;
    const [a, b, tx, c, d, ty] = transform;
    const det = a * d - b * c;
    if (!Number.isFinite(det) || Math.abs(det) <= Number.EPSILON) return null;
    const invDet = 1 / det;
    const ia = d * invDet;
    const ib = -b * invDet;
    const ic = -c * invDet;
    const id = a * invDet;
    const itx = (b * ty - d * tx) * invDet;
    const ity = (c * tx - a * ty) * invDet;
    return Float32Array.of(
      w * ia / imageW, w * ib / imageH, x + w * itx,
      h * ic / imageW, h * id / imageH, y + h * ity,
      0, 0, 1,
    );
  };

  const imageAdjustmentMatrix = (values) => {
    if (values.length !== 7 || values.every((v) => v === 0)) return null;
    const exp = values[0] / 100;
    const con = values[1] / 100;
    const sat = values[2] / 100;
    const temp = values[3] / 100;
    const tint = values[4] / 100;
    const hi = values[5] / 100;
    const sh = values[6] / 100;
    const e = 1 + exp * 1.5;
    const contrast = 1 + con;
    const contrastOffset = 0.5 * (1 - contrast);
    const saturation = 1 + sat;
    const [lr, lg, lb] = [0.2126, 0.7152, 0.0722];
    const [sr, sg, sb] = [(1 - saturation) * lr, (1 - saturation) * lg, (1 - saturation) * lb];
    const f = contrast * e;
    const common = (hi + sh * 0.5) * 0.1;
    return Float32Array.of(
      f * (sr + saturation), f * sg, f * sb, 0, contrastOffset + temp * 0.15 + common,
      f * sr, f * (sg + saturation), f * sb, 0, contrastOffset + tint * 0.15 + common,
      f * sr, f * sg, f * (sb + saturation), 0, contrastOffset - temp * 0.15 + common,
      0, 0, 0, 1, 0,
    );
  };

  const blendModeForCode = (blendMode) => [
    CK.BlendMode.SrcOver,
    CK.BlendMode.Darken,
    CK.BlendMode.Multiply,
    CK.BlendMode.Screen,
    CK.BlendMode.Overlay,
    CK.BlendMode.Lighten,
    CK.BlendMode.Difference,
    CK.BlendMode.Hue,
    CK.BlendMode.Saturation,
    CK.BlendMode.Color,
    CK.BlendMode.Luminosity,
    CK.BlendMode.SoftLight,
    CK.BlendMode.ColorDodge,
    CK.BlendMode.ColorBurn,
    CK.BlendMode.HardLight,
    CK.BlendMode.Exclusion,
  ][blendMode] || CK.BlendMode.SrcOver;

  // Linear burn (16) and linear dodge (17) have no CanvasKit built-in; they
  // run as runtime blenders with the same programs as the native backend
  // (`de-host-native/src/backend/skia/blender.rs`). Compiled on first use;
  // `null` records a failed compile, which falls back to the closest
  // built-in (Multiply / Plus).
  const LINEAR_BLEND_SKSL = {
    16: 'half4 main(half4 src, half4 dst) {' +
      'half3 both = max(src.rgb * dst.a + dst.rgb * src.a - src.a * dst.a, half3(0));' +
      'return half4(src.rgb * (1 - dst.a) + dst.rgb * (1 - src.a) + both,' +
      'src.a + dst.a * (1 - src.a));}',
    17: 'half4 main(half4 src, half4 dst) {' +
      'half3 both = min(src.rgb * dst.a + dst.rgb * src.a, half3(src.a * dst.a));' +
      'return half4(src.rgb * (1 - dst.a) + dst.rgb * (1 - src.a) + both,' +
      'src.a + dst.a * (1 - src.a));}',
  };
  const LINEAR_BLEND_FALLBACK = { 16: 'Multiply', 17: 'Plus' };
  const linearBlenders = {};
  const linearBlender = (code) => {
    if (!(code in LINEAR_BLEND_SKSL)) return null;
    if (!(code in linearBlenders)) {
      let blender = null;
      try {
        const effect = CK.RuntimeEffect && CK.RuntimeEffect.MakeForBlender
          ? CK.RuntimeEffect.MakeForBlender(LINEAR_BLEND_SKSL[code])
          : null;
        blender = effect ? effect.makeBlender([]) : null;
      } catch (_) {
        blender = null;
      }
      linearBlenders[code] = blender;
    }
    return linearBlenders[code];
  };
  // Set a scene blend-mode code on `paint`.
  const applyBlend = (paint, code) => {
    const blender = linearBlender(code);
    if (blender && paint.setBlender) {
      paint.setBlender(blender);
    } else if (code in LINEAR_BLEND_FALLBACK) {
      paint.setBlendMode(CK.BlendMode[LINEAR_BLEND_FALLBACK[code]]);
    } else {
      paint.setBlendMode(blendModeForCode(code));
    }
  };

  const drawImageRectLinear = (image, src, dst, paint) => {
    if (canvas.drawImageRectOptions) {
      canvas.drawImageRectOptions(image, src, dst, CK.FilterMode.Linear, CK.MipmapMode.None, paint);
    } else {
      canvas.drawImageRect(image, src, dst, paint, false);
    }
  };

  return {
    beginFrame() {
      // Discard any leaked save / clip / matrix state from a prior (possibly
      // unbalanced) paint pass, then open a clean frame baseline. This mirrors
      // the native backend's per-frame `reset_matrix()`: without it a single
      // unbalanced save/clip in the widget composition would accumulate across
      // repaints and corrupt z-order / clipping (e.g. a frame fill bleeding
      // over later layers).
      dropEffectRedirects();
      while (canvas.getSaveCount() > 1) canvas.restore();
      canvas.save();
    },
    endFrame() {
      dropEffectRedirects();
      while (canvas.getSaveCount() > 1) canvas.restore();
      surface.flush();
    },
    clear(r, g, b, a) { canvas.clear(col(r, g, b, a)); },
    // #164: the compiler message for `sksl`, or null when it compiles. Not
    // cached: the host asks once per source and keeps the answer.
    shaderCompileError(sksl) {
      let message = null;
      try {
        const effect = CK.RuntimeEffect.Make(String(sksl), (error) => {
          message = String(error);
        });
        if (effect) {
          effect.delete();
          return null;
        }
      } catch (error) {
        message = String(error);
      }
      return message || 'SkSL compile failed';
    },

    fillRect(x, y, w, h, r, g, b, a) { const p = fillPaint(r, g, b, a); canvas.drawRect(CK.LTRBRect(x, y, x + w, y + h), p); },
    strokeRect(x, y, w, h, r, g, b, a, sw) { const p = strokePaint(r, g, b, a, sw); canvas.drawRect(CK.LTRBRect(x, y, x + w, y + h), p); },
    fillRoundRect(x, y, w, h, rad, r, g, b, a) { const p = fillPaint(r, g, b, a); canvas.drawRRect(CK.RRectXY(CK.LTRBRect(x, y, x + w, y + h), rad, rad), p); },
    fillRoundRectPerCorner(x, y, w, h, tl, tr, br, bl, r, g, b, a) {
      const rr = Float32Array.of(x, y, x + w, y + h, tl, tl, tr, tr, br, br, bl, bl);
      const p = fillPaint(r, g, b, a); canvas.drawRRect(rr, p);
    },
    fillRoundRectLinearGradient(x, y, w, h, radius, stops, angleDeg, opacity) {
      if (stops.length < 5) return;
      drawGradientRRect(
        uniformRRect(x, y, w, h, radius),
        makeLinearGradient(x, y, w, h, stops, angleDeg, opacity),
        firstStopColor(stops, opacity),
      );
    },
    fillRoundRectLinearGradientPerCorner(x, y, w, h, tl, tr, br, bl, stops, angleDeg, opacity) {
      if (stops.length < 5) return;
      drawGradientRRect(
        perCornerRRect(x, y, w, h, tl, tr, br, bl),
        makeLinearGradient(x, y, w, h, stops, angleDeg, opacity),
        firstStopColor(stops, opacity),
      );
    },
    fillRoundRectRadialGradient(x, y, w, h, radius, stops, cxFrac, cyFrac, radiusFrac, opacity) {
      if (stops.length < 5) return;
      drawGradientRRect(
        uniformRRect(x, y, w, h, radius),
        makeRadialGradient(x, y, w, h, stops, cxFrac, cyFrac, radiusFrac, opacity),
        firstStopColor(stops, opacity),
      );
    },
    fillRoundRectRadialGradientPerCorner(x, y, w, h, tl, tr, br, bl, stops, cxFrac, cyFrac, radiusFrac, opacity) {
      if (stops.length < 5) return;
      drawGradientRRect(
        perCornerRRect(x, y, w, h, tl, tr, br, bl),
        makeRadialGradient(x, y, w, h, stops, cxFrac, cyFrac, radiusFrac, opacity),
        firstStopColor(stops, opacity),
      );
    },
    fillRoundRectAngularGradient(x, y, w, h, radius, stops, cxFrac, cyFrac, rotationDeg, opacity) {
      if (stops.length < 5) return;
      drawGradientRRect(
        uniformRRect(x, y, w, h, radius),
        makeAngularGradient(x, y, w, h, stops, cxFrac, cyFrac, rotationDeg, opacity),
        firstStopColor(stops, opacity),
      );
    },
    fillRoundRectAngularGradientPerCorner(x, y, w, h, tl, tr, br, bl, stops, cxFrac, cyFrac, rotationDeg, opacity) {
      if (stops.length < 5) return;
      drawGradientRRect(
        perCornerRRect(x, y, w, h, tl, tr, br, bl),
        makeAngularGradient(x, y, w, h, stops, cxFrac, cyFrac, rotationDeg, opacity),
        firstStopColor(stops, opacity),
      );
    },
    fillRoundRectMeshGradient(x, y, w, h, radius, rows, cols, colors, opacity) {
      if (colors.length < 4) return;
      drawMeshGradientRRect(
        uniformRRect(x, y, w, h, radius),
        x, y, w, h, rows, cols, colors, opacity,
      );
    },
    fillRoundRectMeshGradientPerCorner(x, y, w, h, tl, tr, br, bl, rows, cols, colors, opacity) {
      if (colors.length < 4) return;
      drawMeshGradientRRect(
        perCornerRRect(x, y, w, h, tl, tr, br, bl),
        x, y, w, h, rows, cols, colors, opacity,
      );
    },
    fillRoundRectShader(x, y, w, h, radius, sksl, uniformNames, uniforms, uniformArities, opacity, fr, fg, fb, fa) {
      drawRuntimeEffectRRect(
        uniformRRect(x, y, w, h, radius),
        x,
        y,
        w,
        h,
        sksl,
        uniformNames,
        uniforms,
        uniformArities,
        opacity,
        [fr, fg, fb, fa],
      );
    },
    fillRoundRectShaderPerCorner(x, y, w, h, tl, tr, br, bl, sksl, uniformNames, uniforms, uniformArities, opacity, fr, fg, fb, fa) {
      drawRuntimeEffectRRect(
        perCornerRRect(x, y, w, h, tl, tr, br, bl),
        x,
        y,
        w,
        h,
        sksl,
        uniformNames,
        uniforms,
        uniformArities,
        opacity,
        [fr, fg, fb, fa],
      );
    },
    strokeRoundRect(x, y, w, h, rad, r, g, b, a, sw) { const p = strokePaint(r, g, b, a, sw); canvas.drawRRect(CK.RRectXY(CK.LTRBRect(x, y, x + w, y + h), rad, rad), p); },
    strokeRoundRectPerCorner(x, y, w, h, tl, tr, br, bl, r, g, b, a, sw) {
      const rr = Float32Array.of(x, y, x + w, y + h, tl, tl, tr, tr, br, br, bl, bl);
      const p = strokePaint(r, g, b, a, sw); canvas.drawRRect(rr, p);
    },
    fillOval(x, y, w, h, r, g, b, a) { const p = fillPaint(r, g, b, a); canvas.drawOval(CK.LTRBRect(x, y, x + w, y + h), p); },
    strokeOval(x, y, w, h, r, g, b, a, sw) { const p = strokePaint(r, g, b, a, sw); canvas.drawOval(CK.LTRBRect(x, y, x + w, y + h), p); },
    strokeLine(x1, y1, x2, y2, r, g, b, a, sw) { const p = strokePaint(r, g, b, a, sw); canvas.drawLine(x1, y1, x2, y2, p); },
    // One closed path, so joins apply and a dash runs around the corners.
    strokePolygon(pts, r, g, b, a, sw) {
      const path = new CK.Path(); path.moveTo(pts[0], pts[1]);
      for (let i = 2; i < pts.length; i += 2) path.lineTo(pts[i], pts[i + 1]);
      path.close();
      const p = strokePaint(r, g, b, a, sw); canvas.drawPath(path, p); path.delete();
    },
    setStrokeStyle(cap, join, dash, offset) {
      if (strokeStyle && strokeStyle.effect) strokeStyle.effect.delete();
      const effect = dash.length >= 2 ? CK.PathEffect.MakeDash(Array.from(dash), offset) : null;
      strokeStyle = { cap, join, effect };
    },
    clearStrokeStyle() {
      if (strokeStyle && strokeStyle.effect) strokeStyle.effect.delete();
      strokeStyle = null;
      cachedStrokePaint.setPathEffect(null);
    },

    fillPolygon(pts, r, g, b, a) {
      const path = new CK.Path(); path.moveTo(pts[0], pts[1]);
      for (let i = 2; i < pts.length; i += 2) path.lineTo(pts[i], pts[i + 1]);
      path.close();
      const p = fillPaint(r, g, b, a); canvas.drawPath(path, p); path.delete();
    },
    // SVG path d-string scaled by `size/viewbox` and translated to (tx,ty).
    strokeSvgPath(d, tx, ty, scale, r, g, b, a, sw) {
      const path = cachedSvgPath(d); if (!path) return;
      const m = CK.Matrix.multiply(CK.Matrix.translated(tx, ty), CK.Matrix.scaled(scale, scale));
      path.transform(m);
      const p = strokePaint(r, g, b, a, sw); canvas.drawPath(path, p); path.delete();
    },
    fillSvgPath(d, tx, ty, scale, evenOdd, r, g, b, a) {
      const path = cachedSvgPath(d); if (!path) return;
      if (evenOdd) path.setFillType(CK.FillType.EvenOdd);
      const m = CK.Matrix.multiply(CK.Matrix.translated(tx, ty), CK.Matrix.scaled(scale, scale));
      path.transform(m);
      const p = fillPaint(r, g, b, a); canvas.drawPath(path, p); path.delete();
    },
    fillSvgPathInRect(d, x, y, w, h, evenOdd, r, g, b, a) {
      const path = cachedSvgPath(d); if (!path) return;
      if (evenOdd) path.setFillType(CK.FillType.EvenOdd);
      fitPathToRect(path, x, y, w, h);
      const p = fillPaint(r, g, b, a); canvas.drawPath(path, p); path.delete();
    },
    strokeSvgPathInRect(d, x, y, w, h, r, g, b, a, sw) {
      const path = cachedSvgPath(d); if (!path) return;
      fitPathToRect(path, x, y, w, h);
      const p = strokePaint(r, g, b, a, sw); canvas.drawPath(path, p); path.delete();
    },
    fillSvgPathInRectLinearGradient(d, x, y, w, h, evenOdd, stops, angleDeg, opacity) {
      const path = cachedSvgPath(d); if (!path) return;
      if (evenOdd) path.setFillType(CK.FillType.EvenOdd);
      fitPathToRect(path, x, y, w, h);
      if (stops.length < 5) { path.delete(); return; }
      const shader = makeLinearGradient(x, y, w, h, stops, angleDeg, opacity);
      if (shader) {
        // shaderPaint always allocates fresh (one-off effect, excluded from
        // the shared cache) — this instance owns its own delete.
        const p = shaderPaint(shader);
        canvas.drawPath(path, p);
        p.delete();
        if (shader.delete) shader.delete();
      } else {
        const p = fillPaint(...firstStopColor(stops, opacity));
        canvas.drawPath(path, p);
      }
      path.delete();
    },
    fillSvgPathInRectRadialGradient(d, x, y, w, h, evenOdd, stops, cxFrac, cyFrac, radiusFrac, opacity) {
      const path = cachedSvgPath(d); if (!path) return;
      if (evenOdd) path.setFillType(CK.FillType.EvenOdd);
      fitPathToRect(path, x, y, w, h);
      if (stops.length < 5) { path.delete(); return; }
      const shader = makeRadialGradient(x, y, w, h, stops, cxFrac, cyFrac, radiusFrac, opacity);
      if (shader) {
        // shaderPaint always allocates fresh (one-off effect, excluded from
        // the shared cache) — this instance owns its own delete.
        const p = shaderPaint(shader);
        canvas.drawPath(path, p);
        p.delete();
        if (shader.delete) shader.delete();
      } else {
        const p = fillPaint(...firstStopColor(stops, opacity));
        canvas.drawPath(path, p);
      }
      path.delete();
    },
    fillSvgPathInRectAngularGradient(d, x, y, w, h, evenOdd, stops, cxFrac, cyFrac, rotationDeg, opacity) {
      const path = cachedSvgPath(d); if (!path) return;
      if (evenOdd) path.setFillType(CK.FillType.EvenOdd);
      fitPathToRect(path, x, y, w, h);
      if (stops.length < 5) { path.delete(); return; }
      const shader = makeAngularGradient(x, y, w, h, stops, cxFrac, cyFrac, rotationDeg, opacity);
      if (shader) {
        const p = shaderPaint(shader);
        canvas.drawPath(path, p);
        p.delete();
        if (shader.delete) shader.delete();
      } else {
        const p = fillPaint(...firstStopColor(stops, opacity));
        canvas.drawPath(path, p);
      }
      path.delete();
    },
    fillInnerShadowSvgPath(d, x, y, w, h, evenOdd, offsetX, offsetY, blur, r, g, b, a) {
      const path = cachedSvgPath(d); if (!path) return;
      if (evenOdd) path.setFillType(CK.FillType.EvenOdd);
      fitPathToRect(path, x, y, w, h);
      const offsetPath = cachedSvgPath(d);
      if (!offsetPath) { path.delete(); return; }
      if (evenOdd) offsetPath.setFillType(CK.FillType.EvenOdd);
      fitPathToRect(offsetPath, x, y, w, h);
      offsetPath.transform(CK.Matrix.translated(offsetX, offsetY));

      canvas.save();
      canvas.clipPath(path, CK.ClipOp.Intersect, true);
      canvas.saveLayer(null, CK.LTRBRect(x, y, x + w, y + h));

      const fill = fillPaint(r, g, b, a);
      canvas.drawPath(path, fill);

      // Dedicated (allocated + deleted) paint: `cut` carries a one-off blend
      // mode + blur mask filter, so it is excluded from the shared fill-
      // paint cache by design — reusing the cache here would leak DstOut /
      // the blur mask into every later `fillPaint()` caller.
      const cut = allocFillPaint(0, 0, 0, 1);
      cut.setBlendMode(CK.BlendMode.DstOut);
      let mask = null;
      const sigma = blur * 0.5;
      if (sigma > 0 && CK.MaskFilter && CK.MaskFilter.MakeBlur) {
        mask = CK.MaskFilter.MakeBlur(CK.BlurStyle.Normal, sigma, false);
        if (mask) cut.setMaskFilter(mask);
      }
      canvas.drawPath(offsetPath, cut);

      cut.delete(); if (mask && mask.delete) mask.delete();
      canvas.restore();
      canvas.restore();
      offsetPath.delete(); path.delete();
    },

    imageDecoded(imageIdLo, imageIdHi, maxEdgePx) {
      return imageCaches.hasFullImage(imageIdLo, imageIdHi, maxEdgePx);
    },

    decodeImage(imageIdLo, imageIdHi, encoded, maxEdgePx) {
      return imageCaches.installFullImage(imageIdLo, imageIdHi, encoded, maxEdgePx);
    },

    drawImageThumb(imageIdLo, imageIdHi, x, y, w, h, jpeg) {
      imageCaches.drawThumbnailCover(canvas, imageIdLo, imageIdHi, jpeg, x, y, w, h);
    },

    drawImageWithOptions(imageIdLo, imageIdHi, x, y, w, h, mode, transform, adjustments, opacity, cornerRadius, blendMode, originalWidth, originalHeight, tileScale) {
      const image = imageCaches.fullImage(imageIdLo, imageIdHi);
      if (!image || !(w > 0) || !(h > 0)) return;
      const imageW = image.width();
      const imageH = image.height();
      if (!(imageW > 0) || !(imageH > 0)) return;
      const dst = CK.LTRBRect(x, y, x + w, y + h);
      const src = CK.LTRBRect(0, 0, imageW, imageH);
      const paint = new CK.Paint();
      paint.setAntiAlias(true);
      paint.setAlphaf(Math.max(0, Math.min(1, opacity)));
      applyBlend(paint, blendMode);
      const matrix = imageAdjustmentMatrix(adjustments);
      const colorFilter = matrix && CK.ColorFilter && CK.ColorFilter.MakeMatrix
        ? CK.ColorFilter.MakeMatrix(matrix)
        : null;
      if (colorFilter) paint.setColorFilter(colorFilter);

      canvas.save();
      if (cornerRadius > 0.5) {
        canvas.clipRRect(CK.RRectXY(dst, cornerRadius, cornerRadius), CK.ClipOp.Intersect, true);
      }
      let shader = null;
      const local = mode === 3 ? null : figmaImageLocalMatrix(x, y, w, h, imageW, imageH, transform);
      if (local) {
        canvas.clipRect(dst, CK.ClipOp.Intersect, true);
        const tileMode = typeof CK.TileMode.Decal !== 'undefined' ? CK.TileMode.Decal : CK.TileMode.Clamp;
        shader = image.makeShaderOptions(tileMode, tileMode, CK.FilterMode.Linear, CK.MipmapMode.None, local);
        if (shader) {
          paint.setShader(shader);
          canvas.drawRect(dst, paint);
        }
      }
      if (!shader) {
        if (mode === 1) {
          const scale = Math.min(w / imageW, h / imageH);
          const dw = imageW * scale;
          const dh = imageH * scale;
          drawImageRectLinear(image, src, CK.LTRBRect(x + (w - dw) / 2, y + (h - dh) / 2, x + (w + dw) / 2, y + (h + dh) / 2), paint);
        } else if (mode === 3) {
          canvas.clipRect(dst, CK.ClipOp.Intersect, true);
          const sourceW = Number.isFinite(originalWidth) && originalWidth > 0 ? originalWidth : imageW;
          const sourceH = Number.isFinite(originalHeight) && originalHeight > 0 ? originalHeight : imageH;
          const authoredTileScale = Number.isFinite(tileScale) && tileScale > 0 ? tileScale : 1.0;
          const maxRepeatsPerAxis = 128;
          const safeTileScale = Math.max(
            authoredTileScale,
            1 / sourceW,
            1 / sourceH,
            Math.abs(w) / (sourceW * maxRepeatsPerAxis),
            Math.abs(h) / (sourceH * maxRepeatsPerAxis),
          );
          const tileW = sourceW * safeTileScale;
          const tileH = sourceH * safeTileScale;
          const centeredX = x + (w - tileW) / 2;
          const centeredY = y + (h - tileH) / 2;
          const startX = centeredX - Math.max(0, Math.ceil((centeredX - x) / tileW)) * tileW;
          const startY = centeredY - Math.max(0, Math.ceil((centeredY - y) / tileH)) * tileH;
          let iy = startY;
          while (iy < y + h) {
            let ix = startX;
            while (ix < x + w) {
              drawImageRectLinear(image, src, CK.LTRBRect(ix, iy, ix + tileW, iy + tileH), paint);
              const nextX = ix + tileW;
              if (!(nextX > ix)) break;
              ix = nextX;
            }
            const nextY = iy + tileH;
            if (!(nextY > iy)) break;
            iy = nextY;
          }
        } else if (mode === 0 || mode === 2) {
          const scale = Math.max(w / imageW, h / imageH);
          const dw = imageW * scale;
          const dh = imageH * scale;
          canvas.clipRect(dst, CK.ClipOp.Intersect, true);
          drawImageRectLinear(image, src, CK.LTRBRect(x + (w - dw) / 2, y + (h - dh) / 2, x + (w + dw) / 2, y + (h + dh) / 2), paint);
        } else {
          drawImageRectLinear(image, src, dst, paint);
        }
      }
      canvas.restore();
      paint.delete();
      if (shader && shader.delete) shader.delete();
      if (colorFilter && colorFilter.delete) colorFilter.delete();
    },

    drawText(t, family, x, y, sz, weight, italic, r, g, b, a) {
      if (!t) return;
      // Per-CHARACTER family resolution: chars the selected named face covers draw
      // with it; the rest fall to the script-segmented path — so a mixed run
      // keeps the named family where it applies and never renders tofu,
      // matching native. Draw + measure split on the SAME registeredCoverage
      // segments and share drawScriptRun/registeredFamilyFont, so advances agree.
      const familyEntry = familyTypefaceEntry(family);
      const covSegs = familyEntry ? registeredCoverageSegments(familyEntry.key, familyEntry.tf, sz, t) : null;
      if (!covSegs || (covSegs.length === 1 && !covSegs[0].registered)) {
        drawScriptRun(t, x, y, sz, weight, italic, r, g, b, a);
        return;
      }
      const p = fillPaint(r, g, b, a);
      if (weight >= 600 && isPaintStyle(CK.PaintStyle.StrokeAndFill)) {
        setPaintStyle(p, CK.PaintStyle.StrokeAndFill);
        p.setStrokeWidth(sz * 0.06);
      }
      let cx = x;
      for (const seg of covSegs) {
        if (seg.registered) {
          const f = registeredFamilyFont(familyEntry.key, familyEntry.tf, sz, italic);
          canvas.drawText(seg.text, cx, y, p, f);
          cx += runWidth(f, seg.text);
        } else {
          cx += drawScriptRun(seg.text, cx, y, sz, weight, italic, r, g, b, a);
        }
      }
    },
    drawShapedText(t, family, x, y, sz, weight, italic, r, g, b, a) {
      if (!t) return;
      if (shapedTextUnavailable()) {
        // No 2D context (headless embed / hostile sandbox). The segmented
        // path at least paints glyphs rather than nothing, though it cannot
        // reorder or join them.
        drawScriptRun(t, x, y, sz, weight, italic, r, g, b, a);
        return;
      }
      drawBrowserText(t, x, y, sz, weight, italic, r, g, b, a, false, family);
    },
    measureShapedText(t, family, sz, weight, italic) {
      if (!t) return 0;
      // Must mirror `drawShapedText` exactly — measuring on one engine and
      // painting on another is what desyncs wrapping and caret geometry.
      if (shapedTextUnavailable()) {
        return this.measureTextFamilyStyled(t, family, sz, weight, italic);
      }
      return browserTextMeasure(t, sz, weight, italic, family);
    },
    measureText(t, sz) {
      return this.measureTextStyled(t, sz, 400, false);
    },
    textAscent(family, sz, weight) {
      const familyEntry = familyTypefaceEntry(family);
      const font = new CK.Font(familyEntry ? familyEntry.tf : tfFor('M', false), sz);
      let ascent = sz * 0.8;
      if (font.getMetrics) {
        const metrics = font.getMetrics();
        const candidate = metrics && Number.isFinite(metrics.ascent) ? -metrics.ascent : NaN;
        if (Number.isFinite(candidate) && candidate > 0) ascent = candidate;
      }
      font.delete();
      return ascent;
    },
    textFirstBaseline(t, family, sz, weight, italic, lineHeight) {
      const firstBreak = String(t || '').search(/[\r\n]/);
      const line = firstBreak < 0 ? String(t || '') : String(t || '').slice(0, firstBreak);
      const sample = line || 'M';
      const familyEntry = familyTypefaceEntry(family);
      const covSegs = familyEntry
        ? registeredCoverageSegments(familyEntry.key, familyEntry.tf, sz, sample)
        : null;
      let ascent = 0;
      let descent = 0;
      const includeBrowserMetrics = (text) => {
        if (!browserTextCtx) return false;
        browserTextCtx.font = browserTextFont(sz, weight, italic);
        const metrics = browserTextCtx.measureText(text);
        const fontAscent = Number(metrics.fontBoundingBoxAscent);
        const fontDescent = Number(metrics.fontBoundingBoxDescent);
        const inkAscent = Number(metrics.actualBoundingBoxAscent);
        const inkDescent = Number(metrics.actualBoundingBoxDescent);
        ascent = Math.max(ascent,
          Number.isFinite(fontAscent) ? fontAscent : 0,
          Number.isFinite(inkAscent) ? inkAscent : 0);
        descent = Math.max(descent,
          Number.isFinite(fontDescent) ? fontDescent : 0,
          Number.isFinite(inkDescent) ? inkDescent : 0);
        return true;
      };
      const includeFontMetrics = (font) => {
        if (!font || !font.getMetrics) return;
        const metrics = font.getMetrics();
        if (!metrics) return;
        const candidateAscent = -Number(metrics.ascent);
        const candidateDescent = Number(metrics.descent);
        if (Number.isFinite(candidateAscent)) ascent = Math.max(ascent, candidateAscent);
        if (Number.isFinite(candidateDescent)) descent = Math.max(descent, candidateDescent);
      };
      const includeFallback = (text) => {
        for (const seg of segments(text)) {
          if (includeBrowserMetrics(seg.text)) continue;
          const font = new CK.Font(tfFor(seg.text, seg.emoji), sz);
          includeFontMetrics(font);
          font.delete();
        }
      };
      if (covSegs && covSegs.some((seg) => seg.registered)) {
        for (const seg of covSegs) {
          if (seg.registered) {
            includeFontMetrics(registeredFamilyFont(familyEntry.key, familyEntry.tf, sz, italic));
          } else {
            includeFallback(seg.text);
          }
        }
      } else {
        includeFallback(sample);
      }
      if (!(ascent > 0)) ascent = sz * 0.8;
      if (!(descent > 0)) descent = sz * 0.2;
      const naturalHeight = ascent + descent;
      const targetHeight = lineHeight > 0 ? sz * lineHeight : naturalHeight;
      const baseline = ascent + (targetHeight - naturalHeight) / 2;
      return Number.isFinite(baseline) && baseline > 0 ? baseline : sz * 0.8;
    },
    measureTextFamilyStyled(t, family, sz, weight, italic) {
      const familyEntry = familyTypefaceEntry(family);
      const covSegs = familyEntry ? registeredCoverageSegments(familyEntry.key, familyEntry.tf, sz, t) : null;
      if (!covSegs || (covSegs.length === 1 && !covSegs[0].registered)) {
        // No registered family (or it covers nothing): the family-blind
        // script-segmented measure — the SAME path drawText falls to.
        return this.measureTextStyled(t, sz, weight, italic);
      }
      let w = 0;
      for (const seg of covSegs) {
        if (seg.registered) {
          const f = registeredFamilyFont(familyEntry.key, familyEntry.tf, sz, italic);
          w += runWidth(f, seg.text);
        } else {
          w += this.measureTextStyled(seg.text, sz, weight, italic);
        }
      }
      return w;
    },
    measureTextStyled(t, sz, weight, italic) {
      let w = 0;
      for (const seg of segments(t)) {
        if (shouldUseBrowserTextFallback(seg.text, seg.emoji)) {
          w += browserTextMeasure(seg.text, sz, weight, italic);
          continue;
        }
        const f = new CK.Font(tfFor(seg.text, seg.emoji), sz);
        if (italic && !seg.emoji) f.setSkewX(-0.25);
        w += runWidth(f, seg.text);
        f.delete();
      }
      return w;
    },
    /// Measure a multi-run paragraph with optional line-wrapping and return
    /// [width, height, line_count, baseline] to match the layout engine's contract.
    ///
    /// Runs are arrays passed from Rust: texts[], families[], sizes[], weights[],
    /// italics[] (u8 bools), spacing[] (letter_spacing per run).
    /// maxWidth is the wrap budget; -1 means natural single-line extent.
    /// lineHeight is the multiplier (0 = engine default = 1.3).
    ///
    /// Wrapping strategy: measure each run segment-by-segment (same as the
    /// design-canvas draw path), break on newlines, and when a run exceeds the
    /// budget, measure character by character to find the break point.
    measureParagraph(texts, families, sizes, weights, italics, spacing, maxWidth, lineHeight) {
      if (!texts || texts.length === 0) {
        return Float32Array.of(0, 0, 0, 0);
      }

      // Collect runs into a flat array structure.
      const runs = [];
      let hasNewline = false;
      for (let i = 0; i < texts.length; i++) {
        const text = String(texts[i] || '');
        const family = String(families[i] || '');
        const sz = Number(sizes[i]) || 16;
        const weight = Number(weights[i]) || 400;
        const italic = Boolean(italics[i]);
        const letterSpacing = Number(spacing[i]) || 0;
        if (text.includes('\n')) hasNewline = true;
        runs.push({ text, family, sz, weight, italic, letterSpacing });
      }

      // Compute baseline and natural height metrics from the largest/heaviest run.
      let maxSz = 0;
      let ascent = 0;
      let descent = 0;
      for (const run of runs) {
        maxSz = Math.max(maxSz, run.sz);
        if (browserTextCtx) {
          browserTextCtx.font = browserTextFont(run.sz, run.weight, run.italic, run.family);
          const metrics = browserTextCtx.measureText('M');
          const fontAsc = Number(metrics.fontBoundingBoxAscent);
          const fontDesc = Number(metrics.fontBoundingBoxDescent);
          const inkAsc = Number(metrics.actualBoundingBoxAscent);
          const inkDesc = Number(metrics.actualBoundingBoxDescent);
          ascent = Math.max(ascent,
            Number.isFinite(fontAsc) ? fontAsc : 0,
            Number.isFinite(inkAsc) ? inkAsc : 0);
          descent = Math.max(descent,
            Number.isFinite(fontDesc) ? fontDesc : 0,
            Number.isFinite(inkDesc) ? inkDesc : 0);
        }
      }
      if (!(ascent > 0)) ascent = maxSz * 0.8;
      if (!(descent > 0)) descent = maxSz * 0.2;
      const naturalLineHeight = ascent + descent;
      const finalLineHeight = lineHeight > 0 ? maxSz * lineHeight : naturalLineHeight;
      const baseline = ascent + (finalLineHeight - naturalLineHeight) / 2;

      // Helper to measure a single run with explicit text, sz, weight, italic, family.
      const measureRunWidth = (runText, runSz, runWeight, runItalic, runFamily) => {
        return this.measureTextFamilyStyled(runText, runFamily, runSz, runWeight, runItalic);
      };

      // If no max width or no newlines and no need to wrap, return natural width.
      if (maxWidth < 0) {
        let w = 0;
        for (const run of runs) {
          w += measureRunWidth(run.text, run.sz, run.weight, run.italic, run.family);
          // Add letter spacing: # of chars * spacing.
          if (run.letterSpacing !== 0) {
            w += run.text.length * run.letterSpacing;
          }
        }
        // Count explicit lines from newlines.
        let lineCount = 1;
        for (const run of runs) {
          lineCount += (run.text.match(/\n/g) || []).length;
        }
        const height = maxSz * (lineHeight > 0 ? lineHeight : 1.3) * lineCount;
        return Float32Array.of(w, height, lineCount, baseline);
      }

      // Wrapping logic: split runs into lines based on maxWidth.
      // Strategy mirrors the existing code around line 1375 — measure each run,
      // break on newlines, and when a run exceeds the budget, split it.
      const lines = [];
      let currentLine = [];
      let currentLineWidth = 0;

      for (const run of runs) {
        // Split on newlines first.
        const chunks = run.text.split('\n');
        for (let chunkIdx = 0; chunkIdx < chunks.length; chunkIdx++) {
          const chunk = chunks[chunkIdx];
          const isLastChunk = chunkIdx === chunks.length - 1;

          if (chunk) {
            // Measure this chunk with wrapping if needed.
            const chunkWidth = measureRunWidth(chunk, run.sz, run.weight, run.italic, run.family);
            const totalChunkWidth = chunkWidth + chunk.length * run.letterSpacing;

            if (currentLineWidth + totalChunkWidth <= maxWidth) {
              // Fits on current line.
              currentLine.push({ text: chunk, width: chunkWidth, spacing: run.letterSpacing });
              currentLineWidth += totalChunkWidth;
            } else if (currentLineWidth > 0) {
              // Doesn't fit; start a new line if there's anything on the current line.
              lines.push(currentLine);
              currentLine = [{ text: chunk, width: chunkWidth, spacing: run.letterSpacing }];
              currentLineWidth = totalChunkWidth;
            } else {
              // Current line is empty but chunk still doesn't fit: break chunk.
              let remaining = chunk;
              while (remaining) {
                let fit = '';
                let fitWidth = 0;
                for (const ch of remaining) {
                  const candidate = fit + ch;
                  const candidateWidth = measureRunWidth(candidate, run.sz, run.weight, run.italic, run.family);
                  const totalWidth = candidateWidth + candidate.length * run.letterSpacing;
                  if (totalWidth <= maxWidth) {
                    fit = candidate;
                    fitWidth = candidateWidth;
                  } else {
                    break;
                  }
                }
                if (!fit && remaining) {
                  // Degenerate: even one char doesn't fit; output it anyway.
                  fit = remaining[0];
                  fitWidth = measureRunWidth(fit, run.sz, run.weight, run.italic, run.family);
                }
                if (fit) {
                  lines.push([{ text: fit, width: fitWidth, spacing: run.letterSpacing }]);
                  remaining = remaining.slice(fit.length);
                } else {
                  break;
                }
              }
              currentLine = [];
              currentLineWidth = 0;
            }
          }

          if (!isLastChunk) {
            // Newline: close the current line.
            if (currentLine.length > 0) {
              lines.push(currentLine);
            }
            currentLine = [];
            currentLineWidth = 0;
          }
        }
      }

      if (currentLine.length > 0) {
        lines.push(currentLine);
      }

      // Compute final metrics.
      const lineCount = Math.max(1, lines.length);
      const width = maxWidth;
      const height = finalLineHeight * lineCount;

      return Float32Array.of(width, height, lineCount, baseline);
    },
    registerSystemFont(family, bytes) {
      const key = normalizedFamily(family);
      if (!key || systemTypefaceKeys.has(key)) return Boolean(key);
      const tf = CK.Typeface.MakeFreeTypeFaceFromData(copyBytes(bytes));
      if (!tf) return false;
      const entry = {
        key,
        family: String(family || ''),
        tf,
        cjk: isCjkFamily(family),
        emoji: isEmojiFamily(family),
        textFallback: isTextFallbackFamily(family),
      };
      systemTypefaceKeys.add(key);
      systemTypefaces.push(entry);
      systemTypefacesByFamily.set(key, entry);
      coverageCache.clear();
      clearRegisteredFontCaches();
      return true;
    },
    registerImportedFont(family, bytes) {
      const key = primaryFamilyKey(family);
      if (!key) return false;
      const tf = CK.Typeface.MakeFreeTypeFaceFromData(copyBytes(bytes));
      if (!tf) return false;
      // Replace any prior face under the same key, freeing its wasm-heap tf.
      const prev = importedTypefaces.get(key);
      if (prev && prev.tf && prev.tf.delete) prev.tf.delete();
      importedTypefaces.set(key, { tf, family: String(family || '') });
      coverageCache.clear();
      clearRegisteredFontCaches();
      return true;
    },
    // Register an app-bundled design face. Same replace-on-same-key discipline
    // as `registerImportedFont`; only the registry (and therefore the lookup
    // rank) differs.
    registerBundledFont(family, bytes) {
      const key = primaryFamilyKey(family);
      if (!key) return false;
      const tf = CK.Typeface.MakeFreeTypeFaceFromData(copyBytes(bytes));
      if (!tf) return false;
      const prev = bundledTypefaces.get(key);
      if (prev && prev.tf && prev.tf.delete) prev.tf.delete();
      bundledTypefaces.set(key, { tf, family: String(family || '') });
      coverageCache.clear();
      clearRegisteredFontCaches();
      return true;
    },
    // (Fresh browser imports parse the family name in Rust via `ttf-parser` —
    // the vendored CanvasKit build exposes no family-name API — then register
    // through `registerImportedFont` above with the known family.)
    // The display names of every registered imported family, so the Rust snapshot
    // (which doesn't own the registry on web) can mirror the picker's Imported
    // group after every add / remove and at mount.
    importedFamilyList() {
      const out = [];
      for (const entry of importedTypefaces.values()) {
        if (entry && entry.family) out.push(entry.family);
      }
      return out;
    },
    removeImportedFont(family) {
      const key = primaryFamilyKey(family);
      if (!key) return;
      const entry = importedTypefaces.get(key);
      if (!entry) return;
      if (entry.tf && entry.tf.delete) entry.tf.delete();
      importedTypefaces.delete(key);
      clearRegisteredFontCaches();
    },

    clipRect(x, y, w, h) { canvas.clipRect(CK.LTRBRect(x, y, x + w, y + h), CK.ClipOp.Intersect, true); },
    clipRoundRect(x, y, w, h, rad) { canvas.clipRRect(CK.RRectXY(CK.LTRBRect(x, y, x + w, y + h), rad, rad), CK.ClipOp.Intersect, true); },
    clipRoundRectPerCorner(x, y, w, h, tl, tr, br, bl) {
      const rr = Float32Array.of(x, y, x + w, y + h, tl, tl, tr, tr, br, br, bl, bl);
      canvas.clipRRect(rr, CK.ClipOp.Intersect, true);
    },
    clipOval(x, y, w, h) {
      const path = new CK.Path();
      path.addOval(CK.LTRBRect(x, y, x + w, y + h));
      canvas.clipPath(path, CK.ClipOp.Intersect, true);
      path.delete();
    },
    clipPolygon(pts) {
      if (pts.length < 6) {
        canvas.clipRect(CK.LTRBRect(0, 0, 0, 0), CK.ClipOp.Intersect, true);
        return;
      }
      const path = new CK.Path(); path.moveTo(pts[0], pts[1]);
      for (let i = 2; i < pts.length; i += 2) path.lineTo(pts[i], pts[i + 1]);
      path.close();
      canvas.clipPath(path, CK.ClipOp.Intersect, true);
      path.delete();
    },
    clipSvgPathInRect(d, x, y, w, h, evenOdd) {
      const path = cachedSvgPath(d);
      if (!path) {
        canvas.clipRect(CK.LTRBRect(0, 0, 0, 0), CK.ClipOp.Intersect, true);
        return;
      }
      if (evenOdd) path.setFillType(CK.FillType.EvenOdd);
      fitPathToRect(path, x, y, w, h);
      canvas.clipPath(path, CK.ClipOp.Intersect, true);
      path.delete();
    },
    save() { canvas.save(); },
    pushCompositeLayer(x, y, w, h, opacity, blendMode) {
      const paint = new CK.Paint();
      paint.setAlphaf(Math.max(0, Math.min(1, opacity)));
      applyBlend(paint, blendMode);
      canvas.saveLayer(paint, CK.LTRBRect(x, y, x + w, y + h));
      paint.delete();
    },
    pushMaskSourceLayer(luminance) {
      const paint = new CK.Paint();
      paint.setBlendMode(CK.BlendMode.DstIn);
      const luma = luminance && CK.ColorFilter && CK.ColorFilter.MakeLuma
        ? CK.ColorFilter.MakeLuma()
        : null;
      if (luma) paint.setColorFilter(luma);
      canvas.saveLayer(paint);
      paint.delete();
      if (luma && luma.delete) luma.delete();
    },
    pushBlendLayer(blendMode) {
      if (!blendMode) {
        canvas.save();
        return;
      }
      const paint = new CK.Paint();
      applyBlend(paint, blendMode);
      canvas.saveLayer(paint);
      paint.delete();
    },
    pushBackdropBlurLayer(sigma) {
      if (!(sigma > 0) || !CK.ImageFilter || !CK.ImageFilter.MakeBlur) {
        canvas.save();
        return;
      }
      const filter = CK.ImageFilter.MakeBlur(sigma, sigma, CK.TileMode.Clamp, null);
      if (!filter) {
        canvas.save();
        return;
      }
      canvas.saveLayer(null, null, filter, 0, CK.TileMode.Clamp);
      if (filter.delete) filter.delete();
    },
    // Shader effects (#6). CanvasKit exposes no runtime-shader image
    // filter, so an object pass redirects the node's draws into an
    // offscreen surface until the balancing restore() and then draws the
    // program over the node box with `object` bound to that capture. A
    // backdrop pass snapshots the current surface under the node box and
    // draws the program with `background` bound to it before the node
    // paints; content still inside an open saveLayer is not in that
    // snapshot (known gap). Anything CanvasKit refuses is a plain save.
    pushShaderEffectLayer(x, y, w, h, sksl, uniformNames, uniforms, uniformArities, backdrop) {
      const entry = runtimeEffects.get(sksl);
      const childName = backdrop ? OP_CK_EFFECT_BACKGROUND_CHILD : OP_CK_EFFECT_OBJECT_CHILD;
      const box = entry && entry.children.includes(childName)
        ? opCkDeviceBounds(
          canvas.getTotalMatrix(), x, y, w, h, drawSurface.width(), drawSurface.height(),
        )
        : null;
      if (!box) {
        canvas.save();
        return;
      }
      const [dl, dt, dr, db] = box;
      // The uniform slices are views into wasm memory that only live for
      // this call; an object pass reads them at its restore, so copy.
      const pass = {
        entry, x, y, w, h, childName,
        names: uniformNames,
        values: Float32Array.from(uniforms),
        arities: Uint32Array.from(uniformArities),
      };
      if (backdrop) {
        const image = drawSurface.makeImageSnapshot(Int32Array.of(dl, dt, dr, db));
        canvas.save();
        if (image) {
          drawShaderEffectPass(pass, image, childName, dl, dt);
          if (image.delete) image.delete();
        }
        return;
      }
      const offscreen = scratchSurface(effectRedirects.length, dr - dl, db - dt);
      if (!offscreen) {
        canvas.save();
        return;
      }
      const offscreenCanvas = offscreen.getCanvas();
      offscreenCanvas.restoreToCount(1);
      offscreenCanvas.resetMatrix();
      offscreenCanvas.clear(CK.TRANSPARENT);
      offscreenCanvas.save();
      offscreenCanvas.translate(-dl, -dt);
      offscreenCanvas.concat(canvas.getTotalMatrix());
      effectRedirects.push({
        ...pass,
        dl,
        dt,
        cw: dr - dl,
        ch: db - dt,
        surface: offscreen,
        canvas: offscreenCanvas,
        depth: offscreenCanvas.getSaveCount(),
        parentCanvas: canvas,
        parentSurface: drawSurface,
      });
      canvas = offscreenCanvas;
      drawSurface = offscreen;
    },
    // Shader textures (#165): a backdrop snapshot, or the draws between
    // begin and end redirected into an offscreen surface (an SDF of their
    // alpha when `sdf`). Children bind them by id like images.
    supportsShaderTextures() { return true; },
    captureShaderBackdrop(lo, hi, x, y, w, h) {
      // From the window surface: an open redirect holds only the node
      // being captured. `(ox, oy)`: the current surface's offset in it.
      const root = effectRedirects.length > 0 ? effectRedirects[0].parentSurface : drawSurface;
      let ox = 0;
      let oy = 0;
      for (const held of effectRedirects) {
        if (held.surface) { ox += held.dl; oy += held.dt; }
      }
      const shifted = opCkMat3Multiply([1, 0, ox, 0, 1, oy, 0, 0, 1], canvas.getTotalMatrix());
      const box = opCkDeviceBounds(shifted, x, y, w, h, root.width(), root.height());
      if (!box) return;
      installFrameTexture(
        lo, hi, root.makeImageSnapshot(Int32Array.of(...box)), box[0] - ox, box[1] - oy, 1,
      );
    },
    beginShaderTexture(lo, hi, x, y, w, h) {
      // The whole box, off-surface parts included, unless it is huge.
      const whole = opCkDeviceBox(canvas.getTotalMatrix(), x, y, w, h);
      const box = whole && whole[2] - whole[0] <= 4096 && whole[3] - whole[1] <= 4096
        ? whole
        : opCkDeviceBounds(canvas.getTotalMatrix(), x, y, w, h, drawSurface.width(), drawSurface.height());
      const offscreen = box && scratchSurface(effectRedirects.length, box[2] - box[0], box[3] - box[1]);
      if (!offscreen) {
        effectRedirects.push({ texture: { lo, hi }, skipped: true, canvas: null });
        canvas.save();
        return;
      }
      const [dl, dt] = box;
      const total = canvas.getTotalMatrix();
      const offscreenCanvas = offscreen.getCanvas();
      offscreenCanvas.restoreToCount(1);
      offscreenCanvas.resetMatrix();
      offscreenCanvas.clear(CK.TRANSPARENT);
      offscreenCanvas.save();
      offscreenCanvas.translate(-dl, -dt);
      offscreenCanvas.concat(total);
      effectRedirects.push({
        texture: { lo, hi },
        // Device texels per paint unit, for the SDF's distance unit.
        deviceScale: Math.sqrt(Math.abs(total[0] * total[4] - total[1] * total[3])),
        dl,
        dt,
        w: box[2] - dl,
        h: box[3] - dt,
        surface: offscreen,
        canvas: offscreenCanvas,
        depth: offscreenCanvas.getSaveCount(),
        parentCanvas: canvas,
        parentSurface: drawSurface,
      });
      canvas = offscreenCanvas;
      drawSurface = offscreen;
    },
    endShaderTexture(sdf, sdfScale) {
      const top = effectRedirects[effectRedirects.length - 1];
      if (!top || !top.texture) return;
      effectRedirects.pop();
      if (top.skipped) {
        canvas.restore();
        return;
      }
      const image = top.surface.makeImageSnapshot(Int32Array.of(0, 0, top.w, top.h));
      top.canvas.restoreToCount(1);
      canvas = top.parentCanvas;
      drawSurface = top.parentSurface;
      if (!image) return;
      if (sdf) {
        const field = opCkMakeSdfImage(CK, image, top.deviceScale * sdfScale);
        image.delete();
        if (field) {
          installFrameTexture(top.texture.lo, top.texture.hi, field.image, top.dl, top.dt, field.factor);
        }
      } else {
        installFrameTexture(top.texture.lo, top.texture.hi, image, top.dl, top.dt, 1);
      }
    },
    releaseShaderTexture(lo, hi) { releaseFrameTexture(`${lo}:${hi}`); },
    hasShaderTexture(lo, hi) { return frameTextures.has(`${lo}:${hi}`); },
    restore() {
      const top = effectRedirects[effectRedirects.length - 1];
      if (top && !top.texture && canvas === top.canvas && canvas.getSaveCount() === top.depth) {
        finishObjectShaderEffect();
        return;
      }
      canvas.restore();
    },
    translate(x, y) { canvas.translate(x, y); },
    scale(sx, sy) { canvas.scale(sx, sy); },
    rotate(deg, px, py) { canvas.rotate(deg, px, py); },

    resize(w, h) {
      el.width = w; el.height = h;
      try { surface.delete(); } catch (e) {}
      // A resize mid-pass drops the open shader-effect redirects.
      effectRedirects.length = 0;
      for (const held of effectScratch.splice(0)) if (held) held.surface.delete();
      surface = CK.MakeWebGLCanvasSurface(canvasId);
      canvas = surface.getCanvas();
      drawSurface = surface;
    },
    // Set the device-pixel-ratio used to supersample the offscreen text raster.
    // Called from Rust on mount + every display resize so glyph bitmaps stay
    // crisp on HiDPI screens.
    setDpr(v) {
      const next = Number.isFinite(v) && v > 0 ? Math.max(1, v) : 1;
      if (next === textDpr) return;
      textDpr = next;
      clearBrowserTextCache();
    },
  };
}
