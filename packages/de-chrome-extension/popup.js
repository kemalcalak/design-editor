/**
 * Popup controller: capture the active tab, then import or download it.
 *
 * This file owns the browser-facing half of the extension — `chrome.tabs`,
 * `chrome.downloads`, `chrome.storage` and the popup DOM. The rules it applies
 * (which endpoints are addressable, what a download is named) come from the
 * Rust core; nothing here re-derives them. UI text comes from `i18n.js`, not
 * `chrome.i18n`, because the language is the user's choice here rather than
 * the browser's — see that file's header.
 *
 * The one flow that does NOT run here is the element pick: Chrome closes the
 * popup as soon as the user clicks into the page, so arming the picker,
 * capturing, delivering and reporting all belong to the service worker
 * (`background.js`). This file starts that flow and, on its next open, shows
 * whatever the worker recorded.
 *
 * The status line and every sentence that can appear in it live next door in
 * `popup-status.js`: the mapping from a failure code to a message grows with
 * each destination, while this controller does not, and keeping all of it in
 * one file is what makes it checkable.
 */

import { capturePage } from './capture.js';
import { defaultEndpoint, importSnapshot, normalizeEndpoint } from './client.js';
import { coreError, getCore, invalidateCore, isCoreTrap } from './core-registry.js';
import {
  currentLocale,
  initLocale,
  isSupportedLocale,
  LOCALE_KEY,
  persistLocale,
  setLocale,
  SUPPORTED_LOCALES,
} from './i18n.js';
import {
  captureDetail,
  localizeDocument,
  msg,
  renderStatus,
  reportCaptureError,
  reportDeliveryError,
  setStatus,
} from './popup-status.js';
import { showPendingWorkerResult, startWorkerAction } from './popup-worker-actions.js';
// The popup's own loader — dynamic, so an unbuilt checkout still renders an
// actionable message instead of a blank window. The service worker MUST NOT
// import this file; see `core-registry.js`.
import { loadCore } from './wasm-core.js';

const ENDPOINT_KEY = 'endpoint';
/** Which delivery the user last chose; the element pick reuses it. */
const LAST_ACTION_KEY = 'lastAction';
/** Schemes Chrome lets `activeTab` + `scripting` reach. */
const INJECTABLE = /^(https?|file):/i;

const captureButton = document.getElementById('capture');
const designButton = document.getElementById('design-md');
const pickButton = document.getElementById('pick');
const downloadButton = document.getElementById('download');
const endpointInput = document.getElementById('endpoint');
const endpointSummary = document.getElementById('endpoint-summary');
const localeSelect = document.getElementById('locale');

/** Last successful capture in this popup session: `{ text, meta }`. */
let captured = null;
let busy = false;

function setBusy(next) {
  busy = next;
  for (const button of [captureButton, designButton, pickButton, downloadButton]) {
    button.disabled = next;
  }
}

async function activeTab() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab || typeof tab.id !== 'number')
    throw Object.assign(new Error('no tab'), { code: 'noTab' });
  if (!INJECTABLE.test(tab.url || '')) {
    throw Object.assign(new Error('restricted tab'), { code: 'restricted' });
  }
  return tab;
}

/** Capture the page unless this popup session already holds a snapshot. */
async function ensureCapture() {
  if (captured) return captured;
  const tab = await activeTab();
  setStatus(msg('statusCapturing'), 'working', { progress: 0 });
  captured = await capturePage(tab.id, (transferred, total) => {
    const fraction = total === 0 ? 1 : transferred / total;
    setStatus(msg('statusTransferring', [`${Math.round(fraction * 100)}%`]), 'working', {
      progress: fraction,
    });
  });
  return captured;
}

/** Read + validate the endpoint box, storing the normalized form. */
async function commitEndpoint() {
  const endpoint = normalizeEndpoint(endpointInput.value);
  if (!endpoint) {
    setStatus(msg('errorEndpoint'), 'error');
    return null;
  }
  endpointInput.value = endpoint;
  endpointSummary.textContent = endpoint;
  await chrome.storage.local.set({ [ENDPOINT_KEY]: endpoint });
  return endpoint;
}

/** Remember which delivery to reuse when the user picks an element. */
function rememberAction(action) {
  return chrome.storage.local.set({ [LAST_ACTION_KEY]: action });
}

/** Send the capture to the Design Editor at the configured loopback endpoint. */
async function onCapture() {
  if (busy) return;
  const endpoint = await commitEndpoint();
  if (!endpoint) return;

  setBusy(true);
  try {
    const { text, meta } = await ensureCapture();
    setStatus(msg('statusSending'), 'working');
    const outcome = await importSnapshot(endpoint, text);
    await rememberAction('send');
    setStatus(msg('statusImported', [String(outcome.nodeCount)]), 'ok', {
      detail: captureDetail(meta, outcome.warnings),
    });
  } catch (error) {
    if (isCoreTrap(error)) invalidateCore();
    reportDeliveryError(error, endpoint);
  } finally {
    setBusy(false);
  }
}

/**
 * Revoke `objectUrl` once download `id` reaches a terminal state.
 *
 * `chrome.downloads.download` resolves as soon as the download STARTS, so
 * revoking right after it would cut a multi-MB snapshot short. A timer backs
 * the listener up in case the event never arrives.
 */
function revokeWhenDownloadSettles(id, objectUrl) {
  let done = false;
  const finish = () => {
    if (done) return;
    done = true;
    chrome.downloads.onChanged.removeListener(onChanged);
    URL.revokeObjectURL(objectUrl);
  };
  function onChanged(delta) {
    if (delta.id === id && delta.state && delta.state.current !== 'in_progress') finish();
  }
  chrome.downloads.onChanged.addListener(onChanged);
  setTimeout(finish, 120_000);
}

async function onDownload() {
  if (busy) return;
  setBusy(true);
  try {
    const { text, meta } = await ensureCapture();
    // Convert the raw snapshot into a ready-to-open `.de` document (the same
    // conversion `de import:snapshot` runs), so the download opens directly by
    // double-click with no CLI step.
    const converted = JSON.parse(getCore().snapshotToOpDocument(text, String(meta.title || '')));
    if (!converted.ok) {
      const error = new Error(String(converted.error || 'empty capture'));
      error.code = 'empty';
      throw error;
    }
    // The page title is attacker-controlled, so the name is sanitised by the
    // core (`filename.rs`) — no title can produce a `..` component or a
    // dot-file.
    const filename = getCore().opFilename(String(meta.title || ''));
    // `application/octet-stream`, NOT json: Chrome's download pipeline
    // second-guesses an unknown suffix against the blob's MIME type and
    // rewrote `<title>.de` to `<title>.json`. A generic byte stream keeps
    // the `.de` name the core built.
    const objectUrl = URL.createObjectURL(
      new Blob([converted.op], { type: 'application/octet-stream' }),
    );
    const id = await chrome.downloads.download({ url: objectUrl, filename, saveAs: false });
    revokeWhenDownloadSettles(id, objectUrl);
    await rememberAction('download');
    setStatus(msg('statusDownloaded', [filename]), 'ok', {
      detail: captureDetail(meta, converted.warnings),
    });
  } catch (error) {
    if (isCoreTrap(error)) invalidateCore();
    if (error && (error.code === 'offline' || error.code === 'import')) {
      setStatus(msg('errorDownload', [String(error.message)]), 'error');
    } else if (error && error.code) {
      reportCaptureError(error);
    } else {
      setStatus(
        msg('errorDownload', [String(error && error.message ? error.message : error)]),
        'error',
      );
    }
  } finally {
    setBusy(false);
  }
}

/**
 * Hand the element pick to the service worker, then get out of the way.
 *
 * The popup cannot stay open across the click that selects an element, so it
 * closes itself deliberately rather than being closed mid-sentence. The
 * worker reports back through the toolbar badge and through `pickResult`,
 * which {@link showPendingPickResult} renders on the next open.
 */
async function onPick() {
  if (busy) return;
  const endpoint = await commitEndpoint();
  if (!endpoint) return;

  setBusy(true);
  try {
    const tab = await activeTab();
    try {
      await startWorkerAction('pick', tab.id);
    } catch (cause) {
      if (cause && cause.code === 'actionBusy') throw cause;
      // No worker answered. The usual reason is that its static import of the
      // core did not resolve, so it never registered — which is a different
      // problem from "this tab cannot be captured", and the only one of the
      // capture buttons this can affect.
      throw coreError('pickUnavailable', cause);
    }
    window.close();
  } catch (error) {
    setBusy(false);
    if (error && error.code === 'actionBusy') setStatus(msg('designBusy'), 'error');
    else reportCaptureError(error);
  }
}

/**
 * Design-system extraction is worker-owned because the local AI request may
 * outlive this popup by two minutes. It is independent of lastAction and of
 * the element pick's pending result.
 */
async function onDesignMd() {
  if (busy) return;
  const endpoint = await commitEndpoint();
  if (!endpoint) return;
  setBusy(true);
  try {
    const tab = await activeTab();
    setStatus(msg('statusDesignWorking'), 'working');
    await startWorkerAction('designMd', tab.id);
    window.close();
  } catch (error) {
    setBusy(false);
    if (error && (error.code === 'noTab' || error.code === 'restricted')) {
      reportCaptureError(error);
    } else if (error && error.code === 'actionBusy') {
      setStatus(msg('designBusy'), 'error');
    } else {
      setStatus(
        msg('errorDesign', [String(error && error.message ? error.message : error)]),
        'error',
      );
    }
  }
}

/**
 * Fill the language selector.
 *
 * Option labels are each language's own name, so they are NOT translated —
 * a user looking for their language reads it in that language, which is the
 * whole point of a switcher they can find while the UI is in a script they
 * cannot read.
 */
function buildLocaleSelect() {
  localeSelect.replaceChildren();
  for (const { code, name } of SUPPORTED_LOCALES) {
    const option = document.createElement('option');
    option.value = code;
    option.textContent = name;
    localeSelect.append(option);
  }
  localeSelect.value = currentLocale();
}

/** Re-render every string in place. No popup restart, no flash of English. */
async function applyLocale(code) {
  await setLocale(code);
  localizeDocument();
  localeSelect.value = currentLocale();
  renderStatus();
}

async function init() {
  await initLocale();
  localizeDocument();
  buildLocaleSelect();
  // The switcher is wired before the core is loaded on purpose: the
  // "extension has not been built" path returns early, and a user who lands
  // there must still be able to read the sentence in their own language.
  localeSelect.addEventListener('change', async () => {
    const code = localeSelect.value;
    if (!isSupportedLocale(code) || code === currentLocale()) return;
    await persistLocale(code);
    await applyLocale(code);
  });
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area !== 'local' || !changes[LOCALE_KEY]) return;
    const code = changes[LOCALE_KEY].newValue;
    if (!isSupportedLocale(code) || code === currentLocale()) return;
    applyLocale(code);
  });

  // Nothing else in the popup may run before the core is up: every handler
  // below reaches into it. A checkout whose `wasm/` has not been built lands
  // here, so the failure has to be an explicit, actionable message rather
  // than dead buttons.
  try {
    await loadCore();
  } catch (error) {
    setBusy(true);
    setStatus(
      msg('errorCoreMissing', [String(error && error.message ? error.message : error)]),
      'error',
    );
    return;
  }

  const stored = await chrome.storage.local.get(ENDPOINT_KEY);
  const endpoint = stored[ENDPOINT_KEY] || defaultEndpoint();
  endpointInput.value = endpoint;
  endpointSummary.textContent = endpoint;

  if (!(await showPendingWorkerResult())) setStatus(msg('statusIdle'), 'idle');

  captureButton.addEventListener('click', onCapture);
  designButton.addEventListener('click', onDesignMd);
  pickButton.addEventListener('click', onPick);
  downloadButton.addEventListener('click', onDownload);
  endpointInput.addEventListener('change', async () => {
    if (await commitEndpoint()) setStatus(msg('statusIdle'), 'idle');
  });
}

init();
