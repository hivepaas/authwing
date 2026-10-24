// The messages between the extension and the authwing app, as in
// bridge/PROTOCOL.md (the Rust side's `protocol` module has the same
// shapes). Fields are snake_case, as the app has them.
//
// The extension and the app update on their own: each says its version in
// its first message and both keep to the lower one. A message of a type this
// side does not know is skipped, as are fields it does not know.

/** This side's version of the protocol. */
export const PROTOCOL = 1;

/** The native messaging host, as the host manifests name it. */
export const HOST_NAME = "dev.authwing.bridge";

export type TabId = number;

/** Why a fill did not happen, in `filled.reason`. */
export type FillRefusal =
  | "no_tab" // the tab is gone
  | "url_changed" // the tab no longer shows the address the app checked
  | "no_field" // no code field in the page
  | "other_site" // a code field only in a frame of another site, never filled
  | "length_mismatch" // the code does not fit the field's boxes
  | "no_answer"; // the page did not answer (no content script there)

export type FromExtension =
  | { type: "hello"; protocol: number; extension_version: string; browser: string }
  | {
      type: "tab";
      tab_id: TabId;
      url: string;
      title: string;
      frame_urls?: string[];
      has_code_field: boolean;
    }
  | { type: "tab_closed"; tab_id: TabId }
  | { type: "filled"; tab_id: TabId; ok: boolean; reason?: FillRefusal };

export type ToExtension =
  | { type: "welcome"; protocol: number; app_version: string }
  | { type: "fill"; tab_id: TabId; url: string; code: string }
  | { type: "status"; linked: boolean; message?: string };

/** `message` as a ToExtension this side knows, or null to skip it. */
export function decode(message: unknown): ToExtension | null {
  if (typeof message !== "object" || message === null) return null;
  const m = message as Record<string, unknown>;
  switch (m.type) {
    case "welcome":
      return typeof m.protocol === "number" && typeof m.app_version === "string"
        ? { type: "welcome", protocol: m.protocol, app_version: m.app_version }
        : null;
    case "fill":
      return typeof m.tab_id === "number" && typeof m.url === "string" && typeof m.code === "string"
        ? { type: "fill", tab_id: m.tab_id, url: m.url, code: m.code }
        : null;
    case "status":
      if (typeof m.linked !== "boolean") return null;
      return typeof m.message === "string"
        ? { type: "status", linked: m.linked, message: m.message }
        : { type: "status", linked: m.linked };
    default:
      return null;
  }
}

// Between the extension's own parts, never to the app.

/** Content script to background: what this frame has. */
export interface FieldsReport {
  kind: "fields";
  has_code_field: boolean;
}

/** Background to content script: fill `code`, if the frame is at `url`. */
export interface FillRequest {
  kind: "fill";
  url: string;
  code: string;
}

/** Background to content scripts: say again what the frame has. */
export interface Probe {
  kind: "probe";
}

/** A content script's answer to a FillRequest. */
export interface FillResult {
  ok: boolean;
  reason?: FillRefusal;
}

/** Popup to background. */
export type PopupRequest = { kind: "status" } | { kind: "retry" };

/** What the popup shows. */
export interface LinkState {
  linked: boolean;
  app_version?: string;
  /** The version both sides speak. */
  protocol?: number;
  /** Why it is not linked. */
  message?: string;
}

/** Background to the popup, when LinkState changes. */
export interface LinkChanged {
  kind: "link_changed";
  state: LinkState;
}
