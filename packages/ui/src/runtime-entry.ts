/** DOM binding for the existing shell state and versioned native command. */
import { destinationForEntity, indexDestination, type Destination } from "./destinations.js";
import { entityFor } from "./entities.js";
import { paletteFor } from "./palette.js";
import { ROUTE_MANIFEST, ROUTES_BY_ID } from "./routes.js";
import { initialState, navigate, render, select } from "./shell.js";
import { runtimeRequests } from "./runtime-request.js";
import { detailClient } from "./detail-client.js";
import { DetailViews, isDetailSurface } from "./detail-view.js";
import { profileBacklinks, profileEvidence, profilePalette, profileTitle } from "./detail-navigation.js";

interface NativeBridge {
  readonly core: { readonly invoke: (command: string, args: unknown) => Promise<unknown> };
}
declare global { interface Window { readonly __TAURI__?: NativeBridge } }

function element<T extends HTMLElement>(id: string, kind: { new(): T }): T {
  const node = document.getElementById(id);
  if (!(node instanceof kind)) throw new Error(`Missing shell element ${id}`);
  return node;
}
function text(tag: string, value: string): HTMLElement {
  const node = document.createElement(tag);
  node.textContent = value;
  return node;
}
function button(label: string, action: () => void): HTMLButtonElement {
  const node = document.createElement("button");
  node.textContent = label;
  node.addEventListener("click", action);
  return node;
}
let state = initialState();
const view = element("view", HTMLElement);
const drawer = element("drawer", HTMLElement);
const navigation = element("navigation", HTMLElement);
const dialog = element("palette", HTMLDialogElement);
const query = element("palette-query", HTMLInputElement);
const details = new DetailViews(detailClient(async (command, args) => {
  if (!window.__TAURI__) throw new Error("Native runtime absent. Start the desktop with a local service session.");
  return window.__TAURI__.core.invoke(command, args);
}, {
  keys: () => Array.from({ length: window.localStorage.length }, (_, index) => window.localStorage.key(index) ?? ""),
  getItem: (key) => window.localStorage.getItem(key),
  setItem: (key, value) => { window.localStorage.setItem(key, value); },
  removeItem: (key) => { window.localStorage.removeItem(key); },
}), go, profileChrome);
function profileChrome(): void {
  const accepted = details.acceptedState();
  const title = profileTitle(accepted, state.destination);
  const heading = view.querySelector("h1"); if (heading) heading.textContent = title ?? render(state).title;
  const links = view.querySelector(".backlinks");
  if (links) {
    links.replaceChildren(text("h2", "Backlinks"));
    const backlinks = profileBacklinks(accepted, state.destination);
    for (const backlink of backlinks) { const link = button(backlink.title, () => { go(backlink.target); }); link.dataset.destination = backlink.target.path; links.append(link); }
    if (!backlinks.length) links.append(text("p", accepted ? "No backlinks in the selected profile projection." : "Selected profile evidence is unavailable or loading."));
  }
  if (dialog.open) palette();
}
function go(destination: Destination): void {
  state = navigate(state, destination);
  refresh();
  view.scrollTop = 0;
  view.focus();
}
function refresh(): void {
  const frame = render(state);
  const trail = text("p", frame.breadcrumb.map((id) => ROUTES_BY_ID.get(id)?.iaLabel ?? id).join(" / "));
  trail.className = "breadcrumb";
  view.replaceChildren(trail, text("h1", frame.title));
  if (!isDetailSurface(frame.destination)) view.append(text("p", "Explore the synthetic course, concept, project and question examples. Live records are not loaded."), button("Browse examples", openPalette));
  view.dataset.route = frame.destination.path;
  const route = ROUTES_BY_ID.get(state.destination.routeId);
  if (route?.entityKind && state.destination.entityId) {
    const reference = { kind: route.entityKind, id: state.destination.entityId };
    const pin = button("Pin evidence", () => {
      const accepted = profileEvidence(details.acceptedState(), reference);
      if (accepted) state = { ...state, drawer: { selected: reference, pinned: accepted } };
      else if (isDetailSurface(state.destination)) state = { ...state, drawer: { selected: reference, pinned: { title: `Evidence — ${reference.id}`, evidence: [{ source: "Selected detail profile", statement: "Evidence for this entity is unavailable in the accepted detail projection. Reload detail evidence before pinning its sources." }] } } };
      else if (entityFor(reference)) state = select(state, reference);
      else { drawer.replaceChildren(text("h2", "Evidence unavailable"), text("p", "Reload detail evidence before pinning this entity.")); return; }
      refresh(); view.querySelector<HTMLButtonElement>("#pin-evidence")?.focus();
    });
    pin.id = "pin-evidence";
    view.append(pin);
  }
  if (!details.mount(view, frame.destination)) for (const section of frame.sections) {
    const region = text("section", "");
    region.append(text("h2", section.heading), text("p", "No live records loaded."));
    view.append(region);
  }
  const backlinks = text("section", "");
  backlinks.className = "backlinks";
  backlinks.append(text("h2", "Backlinks"));
  for (const reference of frame.backlinks) backlinks.append(button(`${reference.kind}: ${entityFor(reference)?.title ?? reference.id}`, () => { go(destinationForEntity(reference)); }));
  if (frame.backlinks.length === 0) backlinks.append(text("p", "No entity backlinks in this view."));
  view.append(backlinks);
  profileChrome();
  drawer.replaceChildren(text("h2", frame.drawer.title));
  drawer.dataset.entity = frame.drawer.selected?.id ?? "";
  for (const line of frame.drawer.evidence) drawer.append(text("p", line.statement), text("small", line.source));
  if (frame.drawer.selected === null) drawer.append(text("p", "Open an entity and pin its evidence. It stays here as you move between views."), button("Find an entity", openPalette));
  navigation.replaceChildren(...ROUTE_MANIFEST.map((item) => {
    const node = button(item.iaLabel, () => { go(indexDestination(item)); });
    node.dataset.depth = item.parentId === null ? "0" : ROUTES_BY_ID.get(item.parentId)?.parentId === null ? "1" : "2";
    if (item.id === state.destination.routeId) node.setAttribute("aria-current", "page");
    return node;
  }));
}
function palette(): void {
  const profile = profilePalette(details.acceptedState(), state.destination, query.value);
  const destinations = new Set(profile.map((entry) => entry.target.path));
  const entries = [...profile, ...paletteFor(state.destination, query.value).filter((entry) => !destinations.has(entry.target.path))];
  element("palette-results", HTMLElement).replaceChildren(...entries.map((entry) => { const node = button(entry.label, () => { dialog.close(); go(entry.target); }); node.dataset.destination = entry.target.path; return node; }));
  if (entries.length === 0) element("palette-results", HTMLElement).append(text("p", "No matches. Try an entity type or a shorter search."), button("Clear search", () => { query.value = ""; palette(); query.focus(); }));
}
function openPalette(): void { dialog.showModal(); palette(); query.focus(); }
element("palette-open", HTMLButtonElement).addEventListener("click", openPalette);
element("palette-close", HTMLButtonElement).addEventListener("click", () => { dialog.close(); });
query.addEventListener("input", palette);
document.addEventListener("keydown", (event) => { if ((event.ctrlKey || event.metaKey) && event.key === "k") { event.preventDefault(); if (!dialog.open) openPalette(); } });
const request = runtimeRequests(async (command, args) => {
  if (!window.__TAURI__) throw new Error("Native runtime absent");
  return window.__TAURI__.core.invoke(command, args);
}, (status) => {
  element("ingest", HTMLButtonElement).disabled = status.pending !== null;
  element("diagnostics", HTMLButtonElement).disabled = status.pending !== null;
  element("daemon-status", HTMLElement).textContent = status.serviceMessage;
  element("daemon-status", HTMLElement).setAttribute("aria-busy", String(status.pending === "diagnostics"));
  element("command-status", HTMLElement).textContent = status.saveMessage;
  element("command-status", HTMLElement).setAttribute("aria-busy", String(status.pending === "synthetic_ingest"));
  const details = element("receipt-details", HTMLDetailsElement);
  details.hidden = status.receiptId === null;
  if (status.receiptId === null) details.open = false;
  element("receipt-id", HTMLElement).textContent = status.receiptId ?? "";
});
element("diagnostics", HTMLButtonElement).addEventListener("click", () => { void request("diagnostics"); });
element("ingest", HTMLButtonElement).addEventListener("click", () => { void request("synthetic_ingest"); });
refresh();
void request("diagnostics");
