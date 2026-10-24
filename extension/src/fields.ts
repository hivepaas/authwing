// Finding a field for a one-time code in a page, and filling it.
//
// Only inputs are looked at, and of them only what says what they are for
// (autocomplete, inputmode, maxlength, type, name, id, aria-label,
// placeholder): no text of the page. What is found stays here; the
// extension tells the app only whether there is a field.

import type { FillResult } from "./protocol";

/** One box for each character, as many as the code has. */
type Split = { kind: "split"; inputs: HTMLInputElement[] };

export type CodeField = { kind: "single"; input: HTMLInputElement } | Split;

/** Codes from an authenticator have 6 to 8 digits. */
const SHORTEST = 6;
const LONGEST = 8;

/** Words that name a code field, in a few languages. */
const CODE_WORDS = new RegExp(
  [
    "otp",
    "totp",
    "2fa",
    "mfa",
    "one[-_ ]?time",
    "passcode",
    "verification",
    "verify",
    "authenticat",
    "security[-_ ]?code",
    "code",
    "c[oó]digo", // es, pt
    "verificaci[oó]n", // es
    "verifica[cç][aã]o", // pt
    "codice", // it
    "best[aä]tigung", // de
    "kod", // pl, sv, tr
    "weryfikac", // pl
    "do[gğ]rulama", // tr
    "код", // ru, uk
    "验证码|驗證碼", // zh
    "認証|確認コード", // ja
    "인증", // ko
  ].join("|"),
  "i",
);

/** Words that name the code field firmly enough even for a password input. */
const FIRM_WORDS = /otp|totp|2fa|mfa|one[-_ ]?time|passcode|authenticator/i;

/** Codes that are not one-time codes. */
const OTHER_CODES =
  /zip|postal|post[-_ ]?code|pin[-_ ]?code|promo|coupon|voucher|discount|gift|country|area|phone|mobile|captcha|referr|invit|currency|language|search|recovery|backup|qr/i;

const TEXT_TYPES = new Set(["", "text", "tel", "number", "password"]);

/** The page's code field, if it has one. The one with focus wins; then one
 * the page names as for one-time codes; then split boxes; then guesses. */
export function findCodeField(root: Document): CodeField | null {
  const inputs = allInputs(root).filter(usable);
  const split = splitField(inputs);
  const singles = inputs.filter((i) => i.maxLength !== 1);
  const focused = root.activeElement;
  if (focused instanceof HTMLInputElement) {
    if (split?.inputs.includes(focused)) return split;
    if (singles.includes(focused) && rank(focused) > 0) return { kind: "single", input: focused };
  }
  let best: HTMLInputElement | null = null;
  let bestRank = 0;
  for (const input of singles) {
    const r = rank(input);
    if (r > bestRank) [best, bestRank] = [input, r];
  }
  if (best && bestRank === 3) return { kind: "single", input: best };
  if (split) return split;
  return best ? { kind: "single", input: best } : null;
}

/** How surely `input` is a code field: 3 named so, 2 by its shape, 1 by
 * its words, 0 not. */
function rank(input: HTMLInputElement): number {
  const words = [input.name, input.id, input.getAttribute("aria-label"), input.placeholder]
    .filter(Boolean)
    .join(" ");
  if (OTHER_CODES.test(words)) return 0;
  if (input.autocomplete.split(/\s+/).includes("one-time-code")) return 3;
  if (input.maxLength >= 0 && input.maxLength < SHORTEST) return 0;
  if (input.type === "password") return FIRM_WORDS.test(words) ? 1 : 0;
  const numeric = input.inputMode === "numeric" || input.type === "tel" || input.type === "number";
  if (numeric && input.maxLength >= SHORTEST && input.maxLength <= LONGEST) return 2;
  return CODE_WORDS.test(words) ? 1 : 0;
}

/** Boxes of one character each, SHORTEST to LONGEST of them side by side. */
function splitField(inputs: HTMLInputElement[]): Split | null {
  const groups = new Map<Element, HTMLInputElement[]>();
  for (const box of inputs) {
    if (box.maxLength !== 1 || box.type === "password") continue;
    let group = box.parentElement;
    // Boxes each in a wrapper of their own: the wrappers' parent holds them.
    if (group?.parentElement && group.querySelectorAll("input").length === 1) group = group.parentElement;
    if (!group) continue;
    groups.set(group, [...(groups.get(group) ?? []), box]);
  }
  for (const boxes of groups.values()) {
    if (boxes.length >= SHORTEST && boxes.length <= LONGEST) return { kind: "split", inputs: boxes };
  }
  return null;
}

/** The inputs of the page, in open shadow roots too. */
function allInputs(root: Document | ShadowRoot): HTMLInputElement[] {
  const inputs = [...root.querySelectorAll("input")];
  for (const element of root.querySelectorAll("*")) {
    if (element.shadowRoot) inputs.push(...allInputs(element.shadowRoot));
  }
  return inputs;
}

function usable(input: HTMLInputElement): boolean {
  return (
    TEXT_TYPES.has(input.type) &&
    !input.disabled &&
    !input.readOnly &&
    input.getClientRects().length > 0 &&
    getComputedStyle(input).visibility !== "hidden"
  );
}

/** The value setter of inputs themselves, not one a framework put on an
 * input (React does, to see what changes): set through it, the framework
 * sees the new value as typed, on the `input` event. */
const setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;

function put(input: HTMLInputElement, value: string): void {
  input.focus();
  if (setValue) setValue.call(input, value);
  else input.value = value;
  input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText", data: value }));
  input.dispatchEvent(new Event("change", { bubbles: true }));
}

/** Fills `code` in; never submits the form. */
export function fillCode(field: CodeField, code: string): FillResult {
  if (field.kind === "single") {
    const most = field.input.maxLength;
    if (most > 0 && code.length > most) return { ok: false, reason: "length_mismatch" };
    put(field.input, code);
    return { ok: true };
  }
  if (code.length !== field.inputs.length) return { ok: false, reason: "length_mismatch" };
  field.inputs.forEach((box, i) => put(box, code.charAt(i)));
  return { ok: true };
}
