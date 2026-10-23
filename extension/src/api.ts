// The WebExtension API: Firefox's `browser`, else Chrome's `chrome`. Both
// return promises in Manifest V3 and are alike in what the extension uses.

declare const browser: typeof chrome | undefined;

/** "chromium" or "firefox": which build this is (build.mjs sets it). */
declare const __BROWSER__: "chromium" | "firefox";

export const api: typeof chrome = typeof browser !== "undefined" ? browser : chrome;

export const BUILD: "chromium" | "firefox" = __BROWSER__;
