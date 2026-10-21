/**
 * The account arm: talking to the OpenPencil Hub about who is signed in.
 *
 * This file is the browser half of `crates/op-chrome-extension-core`'s
 * `account` module. Every rule — which origin a region resolves to, whether a
 * stored override is allowed to be honoured, what a session reply means, what
 * is safe to render — is decided there and tested natively. What is left here
 * is `fetch`, `chrome.tabs`, `chrome.storage` and `chrome.permissions`.
 *
 * # The flow, and why it holds no secrets
 *
 * The extension is a **public client**. It never sees an SSO client secret,
 * an authorization code, or a session token:
 *
 * 1. {@link openSignIn} opens the Hub's own login URL in a normal TAB. The
 *    whole OAuth/PKCE handshake happens between the browser and the Hub, on
 *    the Hub's origin, exactly as it does when a user visits the portal.
 * 2. The Hub sets `op_hub_session` — `HttpOnly`, `Secure`, `SameSite=Lax`,
 *    `Path=/`. The extension cannot read that cookie and does not ask to:
 *    there is no `cookies` permission in the manifest.
 * 3. {@link fetchSession} does `GET /api/v1/session` with
 *    `credentials: 'include'` and renders the JSON.
 *
 * The one value that crosses into extension memory is the session's
 * `csrf_token`, which the Hub requires on its mutating endpoints — sign-out,
 * and every snapshot filed in the account inbox. It is held for the lifetime
 * of one popup (or one service-worker wake) and is deliberately **never
 * written to `chrome.storage`** — see {@link cacheAccount}, which stores only
 * what the header row paints.
 *
 * # Two browser facts this depends on
 *
 * * **No CORS.** op-hub sends no `Access-Control-Allow-Origin` header at all
 *   (its Go router has no CORS middleware). That is fine, and the same
 *   reason `client.js` works against the local editor: a `fetch` from an
 *   extension page to a host in `host_permissions` is a privileged request,
 *   so no preflight is sent and no ACAO header is required on the way back.
 *   {@link ensureHostPermission} is what turns a manifest mistake into a
 *   sentence instead of an unexplained `TypeError`.
 * * **The `SameSite=Lax` cookie still travels.** Chrome computes
 *   `site_for_cookies` from the request URL for extension-initiated requests
 *   to a host the extension has permission for, so the Hub's Lax cookie is
 *   attached. This is the one part of the design that cannot be proven from
 *   the source of either repository; if a future Chrome tightened it, every
 *   probe would come back `401` while the user is plainly signed in on the
 *   Hub in another tab. That is the first thing to check if the account row
 *   insists you are signed out.
 *
 * # Used from both entry points
 *
 * The popup owns the account row; the service worker calls {@link fetchSession}
 * for itself when an element pick has to be delivered to the account, because
 * the CSRF token lives in popup memory and there is nothing stored for the
 * worker to read. That is why this module imports `core-registry.js` and
 * nothing else: it stays free of a dynamic `import()`, which
 * `ServiceWorkerGlobalScope` forbids, so pulling it into the worker's static
 * graph costs nothing (`scripts/check-sw-imports.mjs` proves it).
 */

import { getCore } from './core-registry.js';

/** Which region's hub the account lives in: `cn` or `global`. */
export const REGION_KEY = 'hubRegion';
/** A loopback hub origin for development. Ignored unless it is loopback. */
export const HUB_DEV_ORIGIN_KEY = 'hubDevOrigin';
/** Which delivery the user chose: `local` or `account`. */
export const DELIVERY_KEY = 'deliveryTarget';
/** The last painted account row, so the popup opens without a blank header. */
export const ACCOUNT_CACHE_KEY = 'account';

/**
 * Build an error carrying a `code` the popup switches on, matching the
 * convention `client.js` established.
 */
function accountError(code, detail) {
  const error = new Error(detail || code);
  error.code = code;
  error.detail = detail;
  return error;
}

/** The stored region, normalized by the core. */
export async function storedRegion() {
  const stored = await chrome.storage.local.get(REGION_KEY);
  return getCore().accountRegion(String(stored[REGION_KEY] ?? ''));
}

/** Persist a region choice. Returns the normalized value actually stored. */
export async function persistRegion(region) {
  const normalized = getCore().accountRegion(String(region ?? ''));
  await chrome.storage.local.set({ [REGION_KEY]: normalized });
  return normalized;
}

/**
 * The hub origin to talk to for `region`.
 *
 * The stored override is passed to the core rather than applied here: the
 * core accepts it only when it normalizes to a loopback `host:port`, which
 * is the one shape the manifest already grants and the one shape that cannot
 * aim a cookie-bearing request at somebody else's host.
 */
export async function hubOrigin(region) {
  const stored = await chrome.storage.local.get(HUB_DEV_ORIGIN_KEY);
  return getCore().hubOrigin(region, String(stored[HUB_DEV_ORIGIN_KEY] ?? ''));
}

/**
 * Throw unless the extension may talk to `origin`.
 *
 * `chrome.permissions.contains` needs no permission of its own. Without this
 * check a manifest missing the origin fails as a bare network `TypeError`,
 * which reads to the user exactly like "the hub is down".
 */
async function ensureHostPermission(origin) {
  const pattern = getCore().hubHostPermission(origin);
  let granted = true;
  try {
    granted = await chrome.permissions.contains({ origins: [pattern] });
  } catch {
    // An older Chrome, or a pattern it will not parse. Let the request
    // itself be the judge rather than blocking on a check that failed.
    return;
  }
  if (!granted) throw accountError('hubNotPermitted', pattern);
}

/** `fetch` with the core's account timeout, returning `{status, text}`. */
async function request(url, init) {
  const timeoutMs = getCore().accountRequestTimeoutMs();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, {
      // The cookie is the whole point: without it the Hub answers 401 for a
      // user who is signed in.
      credentials: 'include',
      // A hub reply is never worth reading from the HTTP cache — a stale
      // 200 would show a session that has since been revoked.
      cache: 'no-store',
      redirect: 'error',
      signal: controller.signal,
      ...init,
    });
    return { status: response.status, text: await response.text() };
  } catch (cause) {
    if (controller.signal.aborted) {
      throw accountError('hubTimeout', String(timeoutMs / 1000));
    }
    throw accountError('hubOffline', String(cause?.message ?? cause));
  } finally {
    clearTimeout(timer);
  }
}

/**
 * Ask the hub who is signed in.
 *
 * @returns {Promise<{state: 'signedIn'|'signedOut'|'error', …}>} — the core's
 *   classification, never a raw reply. `signedOut` is the ordinary answer,
 *   not a failure; `error` means the hub said something we cannot act on and
 *   the previous state should be left alone rather than replaced.
 */
export async function fetchSession(origin) {
  await ensureHostPermission(origin);
  const reply = await request(getCore().hubSessionUrl(origin), { method: 'GET' });
  return JSON.parse(getCore().parseSession(reply.status, reply.text));
}

/**
 * Open the hub's sign-in page in a new tab.
 *
 * A TAB, not a fetch: the SSO handshake sets cookies, follows redirects
 * across origins and may ask for a second factor. None of that belongs in an
 * extension popup, and routing it through the browser is what keeps the
 * extension a public client with nothing to steal.
 */
export async function openSignIn(origin) {
  await chrome.tabs.create({ url: getCore().hubLoginUrl(origin), active: true });
}

/** Open the hub's account page — the sign-out fallback below, and a link. */
export async function openAccountPage(origin) {
  await chrome.tabs.create({ url: getCore().hubAccountUrl(origin), active: true });
}

/**
 * Sign out, and say whether the hub agreed.
 *
 * The Hub used to require an `Origin` header exactly equal to its own public
 * origin, which a request from an extension page can never carry — `fetch`
 * refuses to set that header, so this call was always answered 403. op-hub now
 * admits a well-formed extension origin on exactly the two routes the
 * extension needs, sign-out and the snapshot inbox
 * (`auth.ExtensionCapableMutation`), so it succeeds.
 *
 * The `false` path is still live and still matters: an older Hub, or one that
 * pins a different extension id, refuses again. The caller then opens the
 * account page. Either way the local cache is cleared by the caller, so the
 * popup never claims a session it has stopped believing in.
 *
 * @returns {Promise<boolean>} whether the hub actually ended the session.
 */
export async function signOut(origin, csrfToken) {
  if (typeof csrfToken !== 'string' || csrfToken === '') return false;
  try {
    await ensureHostPermission(origin);
    const reply = await request(getCore().hubLogoutUrl(origin), {
      method: 'POST',
      headers: { 'X-CSRF-Token': csrfToken },
    });
    // 204 is the documented success; a 401 means the session was already
    // gone, which is the same outcome from the user's point of view.
    return reply.status === 204 || reply.status === 401;
  } catch {
    return false;
  }
}

/* --------------------------------------------------------------- the cache */

/**
 * Remember enough to paint the header row on the next open.
 *
 * Deliberately partial: the display name, the avatar URL, the region and the
 * user id. **No CSRF token** — it is a session credential, it would be stale
 * by the next open anyway, and storing it would put a usable token in an
 * extension-local store for no benefit. A cached row is a picture of the last
 * session, not proof of a current one; every open re-probes.
 */
export async function cacheAccount(view, region) {
  if (!view || view.state !== 'signedIn') return;
  await chrome.storage.local.set({
    [ACCOUNT_CACHE_KEY]: {
      userId: view.userId,
      displayName: view.displayName,
      avatarUrl: view.avatarUrl ?? null,
      region,
    },
  });
}

/** The cached row, or null. Anything malformed is treated as absent. */
export async function cachedAccount() {
  const stored = await chrome.storage.local.get(ACCOUNT_CACHE_KEY);
  const cached = stored[ACCOUNT_CACHE_KEY];
  if (!cached || typeof cached.displayName !== 'string') return null;
  return {
    userId: String(cached.userId ?? ''),
    displayName: cached.displayName,
    // Re-validated by the core on the way out: the store is writable by any
    // other view of this extension, and this value ends up in an `<img src>`.
    avatarUrl:
      typeof cached.avatarUrl === 'string'
        ? (getCore().sanitizeAvatarUrl(cached.avatarUrl) ?? null)
        : null,
    region: getCore().accountRegion(String(cached.region ?? '')),
  };
}

export async function clearCachedAccount() {
  await chrome.storage.local.remove(ACCOUNT_CACHE_KEY);
}

/* ------------------------------------------------------------- delivery */

/** The stored delivery target, resolved against whether a session exists. */
export async function deliveryTarget(signedIn) {
  const stored = await chrome.storage.local.get(DELIVERY_KEY);
  return getCore().deliveryTarget(String(stored[DELIVERY_KEY] ?? ''), signedIn);
}

/** Persist a delivery choice. Returns the value that will actually be used. */
export async function persistDeliveryTarget(target, signedIn) {
  await chrome.storage.local.set({ [DELIVERY_KEY]: String(target ?? '') });
  return deliveryTarget(signedIn);
}
