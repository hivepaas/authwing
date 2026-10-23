// Whether the extension is linked to the authwing app, and, where the
// browser lets the user withhold it (Firefox), whether it may read sites.

import { api } from "./api";
import type { LinkChanged, LinkState, PopupRequest } from "./protocol";

const ALL_SITES: chrome.permissions.Permissions = { origins: ["<all_urls>"] };

function element(id: string): HTMLElement {
  const found = document.getElementById(id);
  if (!found) throw new Error(`popup.html has no #${id}`);
  return found;
}

function show(state: LinkState): void {
  element("link").classList.toggle("linked", state.linked);
  element("text").textContent = state.linked
    ? `Linked to authwing ${state.app_version ?? ""}`.trim()
    : "Not linked to authwing";
  const detail = element("detail");
  detail.textContent = state.linked ? "" : (state.message ?? "");
  detail.hidden = state.linked || !state.message;
  element("retry").hidden = state.linked;
}

async function ask(request: PopupRequest): Promise<void> {
  const state = (await api.runtime.sendMessage(request)) as LinkState | undefined;
  if (state) show(state);
}

async function checkSites(): Promise<void> {
  element("sites").hidden = await api.permissions.contains(ALL_SITES);
}

api.runtime.onMessage.addListener((message: unknown) => {
  const changed = message as LinkChanged | undefined;
  if (changed?.kind === "link_changed") show(changed.state);
});

element("retry").addEventListener("click", () => void ask({ kind: "retry" }));
element("allow").addEventListener("click", () => {
  // Asked straight from the click: the browser asks only in answer to one.
  void api.permissions.request(ALL_SITES).then(checkSites);
});

void ask({ kind: "status" });
void checkSites();
