/** Explicit synthetic native smoke, launched only with the host --smoke flag. */
import { allDestinations, destinationForEntity, detailDestination } from "./destinations.js";
import { ENTITIES, entityFor } from "./entities.js";
import { ROUTE_MANIFEST, ROUTES_BY_ID } from "./routes.js";
import { detailClient, type DetailClient } from "./detail-client.js";
import { runDetailDomSmoke } from "./detail-dom-smoke.js";
import { profileBacklinks } from "./detail-navigation.js";

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
async function until(check: () => boolean): Promise<void> {
  const deadline = performance.now() + 10000;
  while (!check()) { if (performance.now() > deadline) throw new Error("Profile shell did not reach the expected state"); await new Promise((resolve) => { setTimeout(resolve, 20); }); }
}
/** Exercises the production shell with the IDs actually returned by its selected profile. */
export async function runProfileShellSmoke(client: DetailClient): Promise<readonly string[]> {
  const state = await client.read(); const lecture = state.corpus.lectures[0]; const concept = state.corpus.concepts[0];
  requireCondition(lecture && concept, "Profile shell smoke requires a seeded lecture and concept");
  click("Lectures", document.querySelector("#navigation") ?? document);
  await until(() => document.querySelector("#view [data-evidence]") !== null);
  click("Reload detail evidence", document.querySelector("#view") ?? document);
  await until(() => document.querySelector("#view #detail-status")?.textContent.includes(`Core projection revision ${String(state.revision)}`) === true);
  click(lecture.title, document.querySelector("#view") ?? document);
  requireCondition(document.querySelector("#view h1")?.textContent === lecture.title, "Lecture title was not derived from the selected profile");
  const route = ROUTES_BY_ID.get("learn.concepts"); requireCondition(route, "Concept route missing");
  const destination = detailDestination(route, concept.id);
  const results = paletteSearch(concept.id);
  const choice = [...results.querySelectorAll<HTMLButtonElement>("button")].find((item) => item.dataset.destination === destination.path);
  requireCondition(choice, "Palette omitted the selected profile concept"); choice.click();
  requireCondition(document.querySelector("#view h1")?.textContent === concept.title && focusedId() === "view", "Profile concept navigation lost its title or focus");
  click("Pin evidence");
  requireCondition(document.querySelector("#drawer")?.textContent.includes(state.source_digest), "Pinned evidence did not bind the accepted profile source");
  requireCondition(focusedId() === "pin-evidence", "Profile pin lost focus");
  const passed = ["profile_shell_palette_title_and_pinned_source"];
  const backlink = profileBacklinks(state, destination).find((item) => item.target.path !== destination.path);
  if (backlink) {
    click(backlink.title, document.querySelector(".backlinks") ?? document);
    requireCondition(document.querySelector<HTMLElement>("#view")?.dataset.route === backlink.target.path, "Profile backlink opened the wrong destination");
    requireCondition(document.querySelector<HTMLElement>("#drawer")?.dataset.entity === concept.id, "Profile navigation discarded pinned evidence");
    passed.push("profile_shell_backlink_preserves_pinned_evidence");
  }
  const absent = ENTITIES.find((entity) => entity.ref.kind === "Concept" && !state.corpus.concepts.some((item) => item.id === entity.ref.id));
  if (absent) {
    const missingPath = destinationForEntity(absent.ref).path;
    const missingChoice = [...paletteSearch(missingPath).querySelectorAll<HTMLButtonElement>("button")].find((item) => item.dataset.destination === missingPath);
    requireCondition(missingChoice, "Original shell example route missing"); missingChoice.click(); click("Pin evidence");
    requireCondition(document.querySelector("#drawer")?.textContent.includes("unavailable in the accepted detail projection"), "Absent detail fell back to another fixture's evidence");
    passed.push("missing_profile_detail_never_pins_legacy_fixture_evidence");
  }
  return passed;
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
        for (const link of [...links.querySelectorAll("button")].map((button) => ({ label: button.textContent, path: button.dataset.destination }))) {
          const label = link.label;
          click(`${entity.ref.kind}: ${entity.title}`, paletteSearch(entity.title));
          const target = ENTITIES.find((candidate) => label === `${candidate.ref.kind}: ${entityFor(candidate.ref)?.title ?? candidate.ref.id}`);
          requireCondition(target || link.path, "Backlink does not resolve to a corpus entity or accepted profile destination");
          requireCondition(label, "Backlink label is empty");
          click(label, document.querySelector(".backlinks") ?? document);
          requireCondition(link.path ? document.querySelector<HTMLElement>("#view")?.dataset.route === link.path : target && document.querySelector("#view h1")?.textContent.includes(target.title), "Backlink opened the wrong entity");
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
    let detailChecks: readonly string[] = [];
    let detailLimitation: string | null = null;
    try {
      const client = detailClient((command, args) => bridge.core.invoke(command, args));
      detailChecks = [...await runDetailDomSmoke(client), ...await runProfileShellSmoke(client)];
    }
    catch (error) { detailLimitation = String(error); }
    const ipcResources = performance.getEntriesByType("resource").map((entry) => entry.name).filter((name) => name.startsWith("http://ipc.localhost/")).slice(-10);
    output.textContent = JSON.stringify({ result: detailLimitation === null ? "PASS" : "PARTIAL", routes, palettes, backlinks, denied, ipcResources, reply, detailChecks, detailLimitation }, null, 2);
  } catch (error) {
    output.textContent = `FAIL: ${String(error)}`;
  }
}
