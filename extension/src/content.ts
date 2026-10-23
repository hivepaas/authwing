// The page's side, in each frame: says whether the frame has a field for a
// one-time code, and nothing else of it, and fills a code the background
// passes on, if the frame is still at the address the app checked.

import { api } from "./api";
import { fillCode, findCodeField } from "./fields";
import type { FieldsReport, FillRequest, FillResult, Probe } from "./protocol";

/** Pages that show the field late, as most sign-in pages made of scripts
 * do, change in bursts. */
const SETTLE = 300;

/** What the background was last told. Frames without a field say nothing
 * until they have one: most frames (ads, embeds) never do. */
let told = false;
let timer: ReturnType<typeof setTimeout> | undefined;

function report(always: boolean): void {
  const has = findCodeField(document) !== null;
  if (has === told && !always) return;
  told = has;
  const message: FieldsReport = { kind: "fields", has_code_field: has };
  api.runtime.sendMessage(message).catch(() => {});
}

function fill(request: FillRequest): FillResult {
  if (location.href !== request.url) return { ok: false, reason: "url_changed" };
  const field = findCodeField(document);
  return field ? fillCode(field, request.code) : { ok: false, reason: "no_field" };
}

api.runtime.onMessage.addListener((message: unknown, _sender, sendResponse) => {
  const kind = (message as FillRequest | Probe | undefined)?.kind;
  if (kind === "probe") report(true);
  else if (kind === "fill") sendResponse(fill(message as FillRequest));
});

new MutationObserver(() => {
  clearTimeout(timer);
  timer = setTimeout(() => report(false), SETTLE);
}).observe(document, {
  childList: true,
  subtree: true,
  attributes: true,
  attributeFilter: ["autocomplete", "inputmode", "maxlength", "type", "name", "id", "hidden", "style", "class", "disabled"],
});

report(false);
