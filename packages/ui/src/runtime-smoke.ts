/** Explicit synthetic native smoke, launched only with the host --smoke flag. */
import { allDestinations } from "./destinations.js";
import { ENTITIES, entityFor } from "./entities.js";
import { ROUTE_MANIFEST } from "./routes.js";

function requireCondition(value: unknown, message: string): asserts value {
  if (!value) throw new Error(message);
}
function focusedId(): string | undefined { return document.activeElement?.id; }
function click(label: string, root: ParentNode = document): void {
  const buttons = [...root.querySelectorAll("button")].filter((node) => node.textContent === label);
  requireCondition(buttons.length > 0, `Missing rendered button: ${label}`);
  buttons[0]?.click();
}
function paletteSearch(query: string): HTMLElement {
  click("Find anything · Ctrl K");
  const input = document.querySelector<HTMLInputElement>("#palette-query");
  requireCondition(input, "Missing palette input");
  input.value = query;
  input.dispatchEvent(new Event("input"));
  const results = document.querySelector<HTMLElement>("#palette-results");
  requireCondition(results, "Missing rendered palette results");
  return results;
}
export async function runSmoke(): Promise<void> {
  const output = document.createElement("pre");
  output.id = "native-smoke-report";
  output.setAttribute("role", "status");
  document.body.append(output);
  try {
    let routes = 0;
    let palettes = 0;
    let backlinks = 0;
    for (const destination of allDestinations()) {
      const results = paletteSearch(destination.path);
      const expected = [...results.querySelectorAll("button")].find((node) => node.dataset.destination === destination.path);
      requireCondition(expected, `No palette navigation to ${destination.path}`);
      expected.click();
      requireCondition(document.querySelector<HTMLElement>("#view")?.dataset.route === destination.path, `Wrong rendered destination: ${destination.path}`);
      requireCondition(focusedId() === "view", "Palette navigation lost destination focus");
      routes++;
      for (const entity of ENTITIES) {
        const originResults = paletteSearch(destination.path);
        const origin = [...originResults.querySelectorAll("button")].find((node) => node.dataset.destination === destination.path);
        requireCondition(origin, "Missing origin destination");
        origin.click();
        const results = paletteSearch(entity.title);
        click(`${entity.ref.kind}: ${entity.title}`, results);
        click("Pin evidence");
        requireCondition(focusedId() === "pin-evidence", "Pin evidence lost keyboard focus");
        requireCondition(document.querySelector<HTMLElement>("#drawer")?.dataset.entity === entity.ref.id, "Evidence was not pinned");
        const links = document.querySelector(".backlinks");
        requireCondition(links, "Missing backlink region");
        for (const label of [...links.querySelectorAll("button")].map((link) => link.textContent)) {
          click(`${entity.ref.kind}: ${entity.title}`, paletteSearch(entity.title));
          const target = ENTITIES.find((candidate) => label === `${candidate.ref.kind}: ${entityFor(candidate.ref)?.title ?? candidate.ref.id}`);
          requireCondition(target, "Backlink does not resolve to a corpus entity");
          requireCondition(label, "Backlink label is empty");
          click(label, document.querySelector(".backlinks") ?? document);
          requireCondition(document.querySelector("#view h1")?.textContent.includes(target.title), "Backlink opened the wrong entity");
          backlinks++;
        }
        click(ROUTE_MANIFEST[0]?.iaLabel ?? "Missing root", document.querySelector("#navigation") ?? document);
        requireCondition(document.querySelector<HTMLElement>("#drawer")?.dataset.entity === entity.ref.id, "Navigation discarded evidence");
        palettes++;
      }
    }
    const bridge = window.__TAURI__;
    requireCondition(bridge, "Native bridge absent");
    let denied = 0;
    for (const [command, args] of [
      ["not_allowlisted", {}],
      ["plugin:fs|read_file", { path: "synthetic.db" }],
      ["plugin:http|fetch", { url: "https://example.invalid" }],
      ["plugin:shell|execute", { program: "synthetic" }],
      ["desktop_request_v1", { request: { version: 2, operation: { command: "diagnostics" } } }],
      ["desktop_request_v1", { request: { version: 1, operation: { command: "diagnostics", path: "synthetic.db" } } }],
    ] as const) {
      let rejected = false;
      try { await bridge.core.invoke(command, args); } catch { rejected = true; }
      requireCondition(rejected, `Unexpectedly allowed ${command}`);
      denied++;
    }
    const reply = await bridge.core.invoke("desktop_request_v1", { request: { version: 1, operation: { command: "diagnostics" } } });
    requireCondition(typeof reply === "object" && reply !== null && "version" in reply && reply.version === 1, "Allowlisted native command did not respond");
    const ipcResources = performance.getEntriesByType("resource").map((entry) => entry.name).filter((name) => name.startsWith("http://ipc.localhost/")).slice(-10);
    output.textContent = JSON.stringify({ result: "PASS", routes, palettes, backlinks, denied, ipcResources, reply }, null, 2);
  } catch (error) {
    output.textContent = `FAIL: ${String(error)}`;
  }
}
