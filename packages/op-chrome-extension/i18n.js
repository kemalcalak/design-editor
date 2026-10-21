/**
 * The extension's own message layer — the reason `chrome.i18n.getMessage` is
 * not called anywhere outside the manifest.
 *
 * `chrome.i18n` resolves against the *browser's* UI language and offers no way
 * to ask for a different one: there is no `getMessage(key, subs, locale)`, and
 * `chrome.i18n.getAcceptLanguages` only reports, it does not select. A user
 * whose Chrome runs in English cannot read this popup in 日本語 through that
 * API, however the extension is packaged. So the popup reads
 * `_locales/<locale>/messages.json` itself — the same files Chrome consumes for
 * `__MSG_*` in the manifest, no second catalog to keep in sync — and does the
 * substitution in JS.
 *
 * What that buys and what it costs:
 *
 * * The popup and the picker overlay follow the language chosen in the popup,
 *   persisted in `chrome.storage.local` under `uiLocale`.
 * * The manifest's `__MSG_extName__` / `__MSG_extDescription__` still follow
 *   the browser, because Chrome renders those (extension name in the toolbar,
 *   the card in `chrome://extensions`, the store listing) long before any of
 *   this code runs. That limitation is documented in the README rather than
 *   worked around: nothing an extension can do reaches those strings.
 *
 * Substitution mirrors `chrome.i18n.getMessage`: `$1`–`$9` are positional,
 * `$name$` resolves through the message's `placeholders` block (whose
 * `content` may itself hold a positional reference), and `$$` is a literal
 * `$`. It is a single left-to-right pass, so text that a substitution inserts
 * is never rescanned — a captured page title containing `$1` cannot make the
 * next argument appear twice.
 *
 * This module is reachable from the service worker's static import graph, so
 * it must never use a dynamic `import()` (see `scripts/check-sw-imports.mjs`)
 * and must not depend on the wasm core — the popup renders the
 * "extension has not been built" error through it.
 */

/**
 * The 15 shipped locales, in the product's own picker order
 * (`crates/op-i18n/src/locale.rs::Locale::ALL`), each labelled with its native
 * display name. `code` is the `_locales/` directory name, which is Chrome's
 * spelling (`zh_CN`, not `zh-CN`).
 *
 * `scripts/check-locales.mjs` asserts this list and `_locales/` agree exactly,
 * so a directory added without a switcher row — or the reverse — fails lint
 * rather than shipping a language nobody can select.
 */
export const SUPPORTED_LOCALES = [
  { code: 'en', name: 'English' },
  { code: 'zh_CN', name: '简体中文' },
  { code: 'zh_TW', name: '繁體中文' },
  { code: 'ja', name: '日本語' },
  { code: 'ko', name: '한국어' },
  { code: 'fr', name: 'Français' },
  { code: 'es', name: 'Español' },
  { code: 'de', name: 'Deutsch' },
  { code: 'pt', name: 'Português' },
  { code: 'ru', name: 'Русский' },
  { code: 'hi', name: 'हिन्दी' },
  { code: 'tr', name: 'Türkçe' },
  { code: 'th', name: 'ไทย' },
  { code: 'vi', name: 'Tiếng Việt' },
  { code: 'id', name: 'Bahasa Indonesia' },
];

/** The fallback catalog, and the manifest's `default_locale`. */
export const FALLBACK_LOCALE = 'en';

/** Where the chosen locale is persisted, and the key other views watch. */
export const LOCALE_KEY = 'uiLocale';

const CODES = new Set(SUPPORTED_LOCALES.map((entry) => entry.code));

/** Catalogs already fetched, keyed by locale code. */
const catalogs = new Map();

let active = FALLBACK_LOCALE;

export function isSupportedLocale(code) {
  return CODES.has(code);
}

/**
 * Map a BCP-47 tag onto one of the 15, mirroring
 * `op_i18n::Locale::from_tag` — including its one non-obvious rule: an
 * explicit `Hans` / `Hant` script beats the region, so `zh-Hans-TW` is
 * Simplified, while a script-less `zh-HK` / `zh-MO` / `zh-TW` is Traditional.
 * Everything else matches on the language subtag alone, which is what folds
 * `pt-BR` and `pt-PT` onto `pt`, and `en-GB` onto `en`.
 *
 * Returns `null` rather than a default so callers can tell "unsupported" from
 * "deliberately English".
 */
export function localeFromTag(tag) {
  if (typeof tag !== 'string') return null;
  const parts = tag
    .trim()
    .split(/[.@]/u)[0]
    .replaceAll('_', '-')
    .toLowerCase()
    .split('-')
    .filter((part) => part.length > 0);
  const language = parts[0];
  if (!language) return null;
  if (language === 'zh') {
    const script = parts.slice(1).find((part) => part === 'hans' || part === 'hant');
    if (script) return script === 'hant' ? 'zh_TW' : 'zh_CN';
    return parts.slice(1).some((part) => part === 'tw' || part === 'hk' || part === 'mo')
      ? 'zh_TW'
      : 'zh_CN';
  }
  // `in` is the pre-1989 ISO code for Indonesian; some platforms still emit it.
  const normalized = language === 'in' ? 'id' : language;
  return isSupportedLocale(normalized) ? normalized : null;
}

/** The locale to use before the user has chosen one. */
export function defaultLocale() {
  const ui = typeof chrome?.i18n?.getUILanguage === 'function' ? chrome.i18n.getUILanguage() : '';
  return localeFromTag(ui) || FALLBACK_LOCALE;
}

async function loadCatalog(locale) {
  const cached = catalogs.get(locale);
  if (cached) return cached;
  try {
    const response = await fetch(chrome.runtime.getURL(`_locales/${locale}/messages.json`));
    const parsed = await response.json();
    catalogs.set(locale, parsed);
    return parsed;
  } catch {
    // A missing or corrupt catalog must not take the popup down with it: the
    // per-key fallback below still reaches English, and English is packaged
    // beside this file.
    catalogs.set(locale, {});
    return {};
  }
}

/**
 * Load `locale` (plus English, for per-key fallback) and make it current.
 *
 * Safe to call repeatedly; catalogs are fetched once each. Returns the locale
 * actually made active, which is `FALLBACK_LOCALE` for an unsupported code.
 */
export async function setLocale(locale) {
  const next = isSupportedLocale(locale) ? locale : FALLBACK_LOCALE;
  await Promise.all(
    next === FALLBACK_LOCALE
      ? [loadCatalog(next)]
      : [loadCatalog(next), loadCatalog(FALLBACK_LOCALE)],
  );
  active = next;
  return active;
}

/** Read the stored choice, falling back to the browser's UI language. */
export async function storedLocale() {
  try {
    const stored = await chrome.storage.local.get(LOCALE_KEY);
    const saved = stored[LOCALE_KEY];
    if (isSupportedLocale(saved)) return saved;
  } catch {
    // Storage can be unavailable in a torn-down view; the UI language is a
    // perfectly good answer.
  }
  return defaultLocale();
}

/** Load whichever locale applies and make it current. */
export async function initLocale() {
  return setLocale(await storedLocale());
}

/** Persist a choice. `initLocale` in every other view picks it up. */
export async function persistLocale(locale) {
  const next = isSupportedLocale(locale) ? locale : FALLBACK_LOCALE;
  await chrome.storage.local.set({ [LOCALE_KEY]: next });
  return next;
}

export function currentLocale() {
  return active;
}

function entryFor(key) {
  const primary = catalogs.get(active);
  const entry = primary && primary[key];
  if (entry && typeof entry.message === 'string') return entry;
  // Per-key fallback, not per-catalog: a locale that is missing one string
  // still renders the other thirty-three in its own language.
  const english = catalogs.get(FALLBACK_LOCALE);
  const fallback = english && english[key];
  return fallback && typeof fallback.message === 'string' ? fallback : null;
}

/** Look up a named placeholder case-insensitively, as Chrome does. */
function placeholderContent(placeholders, name) {
  if (!placeholders) return null;
  const wanted = name.toLowerCase();
  for (const field of Object.keys(placeholders)) {
    if (field.toLowerCase() !== wanted) continue;
    const content = placeholders[field] && placeholders[field].content;
    return typeof content === 'string' ? content : null;
  }
  return null;
}

/** `$1`–`$9` against `args`; anything out of range renders as empty, per Chrome. */
function positional(source, args) {
  let out = '';
  for (let index = 0; index < source.length; index += 1) {
    const char = source[index];
    if (char !== '$') {
      out += char;
      continue;
    }
    const next = source[index + 1];
    if (next === '$') {
      out += '$';
      index += 1;
      continue;
    }
    if (next >= '1' && next <= '9') {
      const value = args[Number(next) - 1];
      out += value === undefined || value === null ? '' : String(value);
      index += 1;
      continue;
    }
    out += char;
  }
  return out;
}

/**
 * Render `key` in the active locale.
 *
 * `substitutions` accepts a string or an array, matching
 * `chrome.i18n.getMessage`. An unknown key renders as the key itself — the
 * same debugging affordance the old wrapper gave by mapping `''` back to the
 * key.
 */
export function t(key, substitutions) {
  const entry = entryFor(key);
  if (!entry) return key;
  const args =
    substitutions === undefined || substitutions === null
      ? []
      : Array.isArray(substitutions)
        ? substitutions
        : [substitutions];

  const message = entry.message;
  if (!message.includes('$')) return message;

  let out = '';
  for (let index = 0; index < message.length; index += 1) {
    const char = message[index];
    if (char !== '$') {
      out += char;
      continue;
    }
    const next = message[index + 1];
    if (next === '$') {
      out += '$';
      index += 1;
      continue;
    }
    if (next >= '1' && next <= '9') {
      const value = args[Number(next) - 1];
      out += value === undefined || value === null ? '' : String(value);
      index += 1;
      continue;
    }
    // `$name$` — the named form. The closing `$` must exist and the body must
    // look like a placeholder name, or this is just a stray dollar sign.
    const close = message.indexOf('$', index + 1);
    const name = close === -1 ? '' : message.slice(index + 1, close);
    if (name.length > 0 && /^[A-Za-z0-9_@]+$/u.test(name)) {
      const content = placeholderContent(entry.placeholders, name);
      if (content !== null) {
        out += positional(content, args);
        index = close;
        continue;
      }
    }
    out += char;
  }
  return out;
}

/**
 * Render a stored message reference.
 *
 * The service worker records the outcome of an element pick as a key plus
 * substitutions rather than as rendered text, so the popup renders it in
 * whatever language is selected when it is finally read — possibly a different
 * one from when the pick ran. A substitution may itself be a `{ key, args }`
 * reference (the pick result nests the delivery message inside
 * `statusPicked`), which is why this recurses.
 */
export function tRef(reference) {
  if (typeof reference === 'string') return reference;
  if (!reference || typeof reference.key !== 'string') return '';
  const args = Array.isArray(reference.args) ? reference.args.map(tRef) : [];
  return t(reference.key, args);
}
