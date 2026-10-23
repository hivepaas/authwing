// The extension's one link to the authwing app, through the native
// messaging host. It tells the app which tab is in front and at exactly
// what address, and passes a code from the app to the page, keeping none.
//
// Chrome runs this as a service worker and Firefox as an event page. Either
// may be stopped when idle and started by the next event, which runs this
// file from the top again: listeners are added at once, and it links again.
// An open native port keeps Chrome's worker running (Chrome 105 and later).
// https://developer.chrome.com/docs/extensions/develop/concepts/service-workers/lifecycle

import { api, BUILD } from "./api";
import {
  decode,
  HOST_NAME,
  PROTOCOL,
  type FieldsReport,
  type FillRefusal,
  type FillRequest,
  type FillResult,
  type FromExtension,
  type LinkChanged,
  type LinkState,
  type PopupRequest,
  type Probe,
  type TabId,
  type ToExtension,
} from "./protocol";

/** Waits before linking again after the host went away: doubled each time. */
const RETRY_FIRST = 1_000;
const RETRY_MOST = 60_000;

/** Tab changes come in bursts (activated, then loading, then complete). */
const DEBOUNCE = 150;

/** The most frame addresses a tab message carries. */
const MOST_FRAMES = 16;

let port: chrome.runtime.Port | null = null;
let state: LinkState = { linked: false, message: "Linking to authwing…" };
let retryIn = RETRY_FIRST;
let retryTimer: ReturnType<typeof setTimeout> | undefined;
let tabTimer: ReturnType<typeof setTimeout> | undefined;
/** The last tab message sent, not to send the same again. */
let lastTab: string | null = null;
/** How many tab reports were started, so only the last one is sent. */
let tabQueries = 0;

/** Each tab's frames that reported, by frame id: the frame's address and
 * whether it has a code field. Frame 0 is the page itself. */
const frames = new Map<TabId, Map<number, { url: string; hasField: boolean }>>();

function connect(): void {
  if (port) return;
  clearTimeout(retryTimer);
  let opened: chrome.runtime.Port;
  try {
    opened = api.runtime.connectNative(HOST_NAME);
  } catch (e) {
    lost(String(e));
    return;
  }
  port = opened;
  opened.onMessage.addListener((message: unknown) => received(decode(message)));
  opened.onDisconnect.addListener((gone: chrome.runtime.Port) => {
    // Chrome says why in runtime.lastError, Firefox in the port's error.
    const error =
      (gone as { error?: { message?: string } }).error?.message ?? api.runtime.lastError?.message;
    if (port === opened) port = null;
    lost(error);
  });
  send({
    type: "hello",
    protocol: PROTOCOL,
    extension_version: api.runtime.getManifest().version,
    browser: browserName(),
  });
}

/** The host went away, or never came: says why, and tries again later. */
function lost(error: string | undefined): void {
  lastTab = null;
  setState({ linked: false, message: explain(error) });
  clearTimeout(retryTimer);
  retryTimer = setTimeout(connect, retryIn);
  retryIn = Math.min(retryIn * 2, RETRY_MOST);
}

/** What the user can do about `error`, from the browser's own words. */
function explain(error: string | undefined): string {
  // Chrome: "Specified native messaging host not found."
  // Firefox: "No such native application dev.authwing.bridge"
  if (error && /not found|no such native application/i.test(error)) {
    return "authwing is not set up for this browser: turn on the browser extension in authwing's settings";
  }
  // Chrome: "Access to the specified native messaging host is forbidden."
  if (error && /forbidden/i.test(error)) {
    return "authwing does not allow this copy of the extension";
  }
  return "Lost the link to authwing";
}

function send(message: FromExtension): void {
  try {
    port?.postMessage(message);
  } catch {
    // Disconnected: onDisconnect says so.
  }
}

function received(message: ToExtension | null): void {
  switch (message?.type) {
    case "welcome":
      retryIn = RETRY_FIRST;
      setState({
        linked: true,
        app_version: message.app_version,
        protocol: Math.min(PROTOCOL, message.protocol),
      });
      lastTab = null;
      void reportTab();
      break;
    case "status":
      if (message.linked) {
        setState({ ...state, linked: true });
      } else {
        lastTab = null;
        setState({ linked: false, message: message.message ?? "authwing is not running" });
      }
      break;
    case "fill":
      void fill(message.tab_id, message.url, message.code);
      break;
    default:
    // A type this version does not know, or not a message: skipped.
  }
}

function setState(next: LinkState): void {
  state = next;
  const changed: LinkChanged = { kind: "link_changed", state };
  // Only the popup listens, when it is open.
  api.runtime.sendMessage(changed).catch(() => {});
}

function tabChanged(): void {
  clearTimeout(tabTimer);
  tabTimer = setTimeout(() => void reportTab(), DEBOUNCE);
}

/** Tells the app about the tab in front, if anything changed. */
async function reportTab(): Promise<void> {
  if (!port || !state.linked) return;
  const asked = ++tabQueries;
  const [tab] = await api.tabs.query({ active: true, lastFocusedWindow: true });
  // A later report started meanwhile: its answer is the newer.
  if (asked !== tabQueries) return;
  if (tab?.id === undefined || tab.id === api.tabs.TAB_ID_NONE || !tab.url) return;
  // Nothing known of its frames, as after the background was stopped: they
  // are asked, and their answers report the tab again.
  if (!frames.has(tab.id)) probe(tab.id);
  const withField = [...(frames.get(tab.id)?.entries() ?? [])].filter(([, f]) => f.hasField);
  const message: FromExtension = {
    type: "tab",
    tab_id: tab.id,
    url: tab.url,
    title: tab.title ?? "",
    has_code_field: withField.length > 0,
  };
  const inFrames = withField
    .filter(([frameId]) => frameId !== 0)
    .map(([, f]) => f.url)
    .slice(0, MOST_FRAMES);
  if (inFrames.length > 0) message.frame_urls = inFrames;
  const json = JSON.stringify(message);
  if (json === lastTab) return;
  lastTab = json;
  send(message);
}

/** Fills `code` in the tab, if it still shows `url`, the address the app
 * checked: in the page itself, or else in a frame of the page's own origin.
 * Never in a frame of another site, which the app did not check. */
async function fill(tabId: TabId, url: string, code: string): Promise<void> {
  const done = (ok: boolean, reason?: FillRefusal) =>
    send(reason ? { type: "filled", tab_id: tabId, ok, reason } : { type: "filled", tab_id: tabId, ok });
  let tab: chrome.tabs.Tab;
  try {
    tab = await api.tabs.get(tabId);
  } catch {
    return done(false, "no_tab");
  }
  if (tab.url !== url) return done(false, "url_changed");
  const origin = originOf(url);
  const withField = [...(frames.get(tabId)?.entries() ?? [])].filter(([, f]) => f.hasField);
  const candidates = withField
    .filter(([frameId, f]) => frameId === 0 || (origin !== null && originOf(f.url) === origin))
    .sort(([a], [b]) => a - b);
  let last: FillResult = { ok: false, reason: withField.length > 0 ? "other_site" : "no_field" };
  for (const [frameId, f] of candidates) {
    // The frame fills only if it is still at this address when it fills.
    const request: FillRequest = { kind: "fill", url: frameId === 0 ? url : f.url, code };
    try {
      const result = (await api.tabs.sendMessage(tabId, request, { frameId })) as FillResult | undefined;
      if (result?.ok) return done(true);
      last = result ?? { ok: false, reason: "no_answer" };
    } catch {
      last = { ok: false, reason: "no_answer" };
    }
  }
  done(false, last.reason);
}

/** Asks the tab's frames to say again whether they have a code field. */
function probe(tabId: TabId): void {
  const message: Probe = { kind: "probe" };
  // No content script there (a browser page, a page still loading): no answer.
  api.tabs.sendMessage(tabId, message).catch(() => {});
}

function originOf(url: string): string | null {
  try {
    const origin = new URL(url).origin;
    return origin === "null" ? null : origin;
  } catch {
    return null;
  }
}

/** The browser's name, as the user knows it; only to be shown. */
function browserName(): string {
  if (BUILD === "firefox") return "firefox";
  if ("brave" in navigator) return "brave";
  const brands =
    (navigator as { userAgentData?: { brands?: { brand: string }[] } }).userAgentData?.brands?.map(
      (b) => b.brand,
    ) ?? [];
  const known: [string, string][] = [
    ["Microsoft Edge", "edge"],
    ["Opera", "opera"],
    ["Vivaldi", "vivaldi"],
    ["Google Chrome", "chrome"],
  ];
  return known.find(([brand]) => brands.includes(brand))?.[1] ?? "chromium";
}

function isFieldsReport(message: unknown): message is FieldsReport {
  return (message as FieldsReport | undefined)?.kind === "fields";
}

function isPopupRequest(message: unknown): message is PopupRequest {
  const kind = (message as PopupRequest | undefined)?.kind;
  return kind === "status" || kind === "retry";
}

api.tabs.onActivated.addListener(tabChanged);

api.tabs.onUpdated.addListener((tabId, change, tab) => {
  if (change.url !== undefined) {
    // Another document, or the same one at another address: what its
    // frames had no longer holds, so they are asked again.
    frames.delete(tabId);
    probe(tabId);
  }
  if (tab.active && (change.url !== undefined || change.title !== undefined || change.status === "complete")) {
    tabChanged();
  }
});

api.tabs.onRemoved.addListener((tabId) => {
  frames.delete(tabId);
  send({ type: "tab_closed", tab_id: tabId });
});

api.windows.onFocusChanged.addListener((windowId) => {
  // No window: the user went to another app, authwing say. The tab in
  // front stays the one it was.
  if (windowId !== api.windows.WINDOW_ID_NONE) tabChanged();
});

api.runtime.onMessage.addListener((message: unknown, sender, sendResponse) => {
  if (isFieldsReport(message)) {
    const tabId = sender.tab?.id;
    if (tabId === undefined || sender.frameId === undefined || !sender.url) return;
    const known = frames.get(tabId) ?? new Map();
    known.set(sender.frameId, { url: sender.url, hasField: message.has_code_field });
    frames.set(tabId, known);
    if (sender.tab?.active) tabChanged();
  } else if (isPopupRequest(message)) {
    if (message.kind === "retry") {
      retryIn = RETRY_FIRST;
      connect();
    }
    sendResponse(state);
  }
});

connect();
