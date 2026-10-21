/**
 * Fail if the MV3 service worker's static import graph contains a dynamic
 * `import(` call.
 *
 * Why this needs a guard rather than a code review: `import()` is disallowed
 * on `ServiceWorkerGlobalScope` by the HTML specification
 * (https://github.com/w3c/ServiceWorker/issues/1356). Chrome rejects the call
 * itself, whenever it happens — deferring it, or making it during the
 * worker's initial evaluation, changes nothing. And nothing catches it early:
 * the extension loads, the popup works, the manifest validates, and the
 * failure only appears when a user triggers the pick flow, as a rejection
 * inside the worker. That is a long way from the edit that caused it.
 *
 * The rule is therefore structural. `background.js` is the worker entry; this
 * walks its relative static-import graph and refuses any dynamic `import(` in
 * it. The popup keeps its dynamic loader (`wasm-core.js`), which is correct
 * for a document — proving the worker never *reaches* that file is this
 * guard's real job.
 *
 * Comments and string literals are stripped before matching, so the many
 * prose mentions of `import()` in this codebase's own documentation do not
 * trip it. Regular-expression literals are not tracked; nothing here contains
 * one, and a false positive from that direction would be a guard that is too
 * strict rather than one that lets the bug through.
 */

import { readFileSync } from 'node:fs';
import { dirname, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const extensionDirectory = resolve(scriptDirectory, '..');
const ENTRY = 'background.js';

/**
 * Blank out comments — and, when `keepStrings` is false, string literals too —
 * preserving offsets and newlines so reported line numbers stay true.
 *
 * The dynamic-import scan wants strings gone as well (a quoted `import(` in
 * prose is not code). The specifier scan obviously needs them kept, since the
 * specifier IS a string; blanking comments alone is enough there, because a
 * commented-out import must not pull a file into the graph.
 */
function stripComments(source, keepStrings) {
  let out = '';
  let index = 0;
  const blank = (text) => text.replaceAll(/[^\n]/g, ' ');

  while (index < source.length) {
    const two = source.slice(index, index + 2);
    if (two === '//') {
      const end = source.indexOf('\n', index);
      const stop = end === -1 ? source.length : end;
      out += blank(source.slice(index, stop));
      index = stop;
      continue;
    }
    if (two === '/*') {
      const end = source.indexOf('*/', index + 2);
      const stop = end === -1 ? source.length : end + 2;
      out += blank(source.slice(index, stop));
      index = stop;
      continue;
    }
    const quote = source[index];
    if (quote === '"' || quote === "'" || quote === '`') {
      let cursor = index + 1;
      while (cursor < source.length) {
        if (source[cursor] === '\\') {
          cursor += 2;
          continue;
        }
        if (source[cursor] === quote) {
          cursor += 1;
          break;
        }
        cursor += 1;
      }
      const literal = source.slice(index, cursor);
      out += keepStrings ? literal : blank(literal);
      index = cursor;
      continue;
    }
    out += quote;
    index += 1;
  }
  return out;
}

/** `import` immediately followed by `(`, not preceded by an identifier char or a dot. */
const DYNAMIC_IMPORT = /(^|[^\w$.])import\s*\(/g;
/** Relative specifiers of static imports: `from './x.js'` and `import './x.js'`. */
const STATIC_SPECIFIER = /(?:from|import)\s*['"](\.[^'"]*)['"]/g;

function lineOf(source, offset) {
  return source.slice(0, offset).split('\n').length;
}

const queue = [ENTRY];
const visited = [];
const violations = [];
const missing = [];

while (queue.length > 0) {
  const current = queue.shift();
  if (visited.includes(current)) continue;
  visited.push(current);

  const absolute = resolve(extensionDirectory, current);
  let source;
  try {
    source = readFileSync(absolute, 'utf8');
  } catch {
    // `wasm/` is a build product; the guard must run in a checkout that has
    // not been built yet, so an unresolvable file is recorded, not fatal.
    missing.push(current);
    continue;
  }

  const code = stripComments(source, false);
  for (const match of code.matchAll(DYNAMIC_IMPORT)) {
    violations.push({ file: current, line: lineOf(code, match.index) });
  }
  for (const match of stripComments(source, true).matchAll(STATIC_SPECIFIER)) {
    const target = resolve(dirname(absolute), match[1]);
    if (target.startsWith(extensionDirectory)) {
      queue.push(relative(extensionDirectory, target));
    }
  }
}

if (violations.length > 0) {
  for (const { file, line } of violations) {
    console.error(`sw-imports: ${file}:${line}: dynamic import() is reachable from ${ENTRY}`);
  }
  console.error(
    'sw-imports: import() is disallowed on ServiceWorkerGlobalScope (w3c/ServiceWorker#1356).',
  );
  console.error(
    `sw-imports: import the module statically in ${ENTRY}, or keep the file that needs`,
  );
  console.error('sw-imports: a dynamic import out of the worker graph.');
  process.exit(1);
}

console.log(`sw-imports: ok (${visited.length} files reachable from ${ENTRY}, no dynamic import)`);
console.log(`sw-imports: graph = ${visited.join(' ')}`);
if (missing.length > 0) {
  console.log(`sw-imports: not built yet, skipped = ${missing.join(' ')}`);
}
