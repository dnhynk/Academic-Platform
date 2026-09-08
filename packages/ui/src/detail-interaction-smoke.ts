/** Browser DOM regressions with synthetic DTOs; never native persistence/playback evidence. */
import { buildDetailFixture } from "./detail-fixture.js";
import { detailClient, detailDecisionDigest, type DetailDecisionRequest, type DetailState } from "./detail-client.js";
import { DetailViews } from "./detail-view.js";
import { detailDestination } from "./destinations.js";
import { ROUTES_BY_ID } from "./routes.js";

function check(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>((done) => { resolve = done; }); return { promise, resolve }; }
async function until(condition: () => boolean): Promise<void> {
  const deadline = performance.now() + 10000;
  while (!condition()) { if (performance.now() > deadline) throw new Error("Interaction smoke timed out"); await new Promise((resolve) => { setTimeout(resolve, 10); }); }
}
function button(host: HTMLElement, label: string): HTMLButtonElement {
  const found = [...host.querySelectorAll("button")].find((item) => item.textContent === label); check(found, `Missing ${label}`); return found;
}
function input(host: HTMLElement, id: string): HTMLInputElement { const found = host.querySelector<HTMLInputElement>(`#${id}`); check(found, `Missing ${id}`); return found; }

export async function runDetailInteractionSmoke(): Promise<readonly string[]> {
  const passed: string[] = [];
  for (const outcome of ["accepted", "unavailable"] as const) {
    const initial: DetailState = { corpus: buildDetailFixture(), decisions: [], revision: 1, profile_id: "interaction-synthetic-profile", known_at_accept_seq: 100, valid_at_ms: 1000, projector_version: "academic.details.v2", source_digest: "1".repeat(64) };
    const response = deferred<unknown>(); const sent = deferred<DetailDecisionRequest>(); let reads = 0; let failRead = false; let state = initial;
    const client = detailClient((_, args) => {
      const op = (args as { request: { operation: DetailDecisionRequest & { command: string } } }).request.operation;
      if (op.command === "details_read") {
        reads++; if (failRead) return Promise.reject(new Error("Synthetic read unavailable"));
        const selected = op.selector.known_at_accept_seq === 90 ? { ...initial, known_at_accept_seq: 90, valid_at_ms: 10 } : state;
        return Promise.resolve({ version: 1, state: "ready", details: selected });
      }
      sent.resolve(op); return response.promise;
    });
    const host = document.createElement("main"); host.style.cssText = "height:300px;overflow:auto"; document.body.append(host);
    const views = new DetailViews(client, () => {});
    const open = (routeId: string, id: string) => { const route = ROUTES_BY_ID.get(routeId); check(route, "Missing route"); host.replaceChildren(); views.mount(host, detailDestination(route, id)); };
    try {
      const concept = initial.corpus.concepts[0]; const project = initial.corpus.projects[0]; check(concept && project, "Missing synthetic fixtures");
      open("learn.concepts", concept.id); await until(() => !!host.querySelector("[data-evidence]"));
      const decision = host.querySelector<HTMLButtonElement>("button[id^='decision-']"); check(decision, "Missing rejection"); decision.click(); const op = await sent.promise;
      open("build.projects", project.id);
      const end = input(host, "preview-end"); end.value = "17";
      const known = input(host, "detail-known"); known.value = "90"; input(host, "detail-valid").value = "10";
      const history = known.closest("details"); check(history, "Missing history disclosure"); history.open = true;
      const source = [...host.querySelectorAll("details")].find((item) => item.querySelector("summary")?.textContent === "Source details"); check(source, "Missing source disclosure"); source.open = true;
      end.focus(); host.scrollTop = 137;
      const scroll = host.scrollTop; const nodes = [...host.querySelectorAll("input, select, details, [data-evidence]")];
      const actor = "00000000-0000-7000-8000-000000000001";
      state = { ...initial, revision: 2, known_at_accept_seq: 101, decisions: [{ sequence: 101, relationId: op.relation_id, action: "REJECT", undoes: null, actor }] };
      response.resolve(outcome === "unavailable" ? { version: 1, state: "unavailable", reason: "LOCAL_SERVICE_UNAVAILABLE" }
        : { version: 1, state: "accepted", details: state, receipt_id: Array<number>(16).fill(7), decision_sequence: 101,
          receipt_decision: { sequence: 101, relation_id: op.relation_id, relation_claim_id: "00000000-0000-7000-8000-000000000002", action: "reject", undoes: null, actor },
          request_id: op.request_id, client_instance_id: op.client_instance_id, idempotency_key: op.idempotency_key, request_digest: await detailDecisionDigest(op) });
      await until(() => host.textContent.includes("Reload detail evidence when ready"));
      check(reads === 1 && nodes.every((item) => item.isConnected), "Late completion reloaded or replaced destination controls");
      check(document.activeElement === end && end.value === "17", "Late completion lost the actual focused draft");
      check(known.value === "90" && input(host, "detail-valid").value === "10" && host.querySelectorAll("details[open]").length === 2 && host.scrollTop === scroll, "Late completion reset history draft, disclosure or scroll");
      check(views.acceptedState()?.revision === 1 && host.querySelector("[data-section='project-goals']"), "Late completion changed the selected destination or asserted a refreshed snapshot");
      check([...host.querySelectorAll<HTMLButtonElement>("button[id^='decision-']")].every((item) => item.disabled), "Stale destination enabled decisions");
      check(client.pendingDecisions().length === (outcome === "accepted" ? 0 : 1), "Receipt recovery lost the pending identity");
      passed.push(`late_${outcome}_preserves_actual_destination_draft_focus_disclosures_scroll`);
      // Explicit user refresh still validates reads, and a failure retains the retry.
      failRead = true; button(host, "Reload detail evidence").click(); await until(() => host.textContent.includes("Detail evidence unavailable"));
      check(views.acceptedState() === null && client.pendingDecisions().length === (outcome === "accepted" ? 0 : 1), "Unavailable read asserted acceptance or lost an original request");
      failRead = false; button(host, "Reload detail evidence").click(); await until(() => !!host.querySelector("[data-evidence]"));
      check(views.acceptedState()?.revision === 2, "Explicit current read did not reconcile the accepted source");
      input(host, "detail-known").value = "90"; input(host, "detail-valid").value = "10"; button(host, "Load selected time").click();
      await until(() => views.acceptedState()?.known_at_accept_seq === 90);
      check(host.textContent.includes("Historical evidence is read-only"), "Historical selector was replaced by a current receipt");
      button(host, "Return to current evidence").click(); await until(() => views.acceptedState()?.known_at_accept_seq === 101);
      passed.push(`late_${outcome}_explicit_refresh_failure_retry_and_history`);
    } finally { views.dispose(); host.remove(); }
  }
  // Isolated audio API double: this checks view/player ownership, not playback.
  const nativeAudio = window.AudioContext; let starts = 0; let closes = 0;
  const audioCounts = () => ({ starts, closes });
  class Context {
    currentTime = 0; destination = {};
    createGain() { return { gain: { value: 1 }, connect() {}, disconnect() {} }; }
    decodeAudioData() { return Promise.resolve({ duration: 12, getChannelData: () => new Float32Array(160) }); }
    resume() { return Promise.resolve(); }
    close() { closes++; return Promise.resolve(); }
    createBufferSource() { return { connect() {}, disconnect() {}, start() { starts++; }, stop() {}, onended: null }; }
  }
  window.AudioContext = Context as unknown as typeof AudioContext;
  const host = document.createElement("main"); document.body.append(host);
  const response = deferred<unknown>(); const sent = deferred<DetailDecisionRequest>();
  const initial: DetailState = { corpus: buildDetailFixture(), decisions: [], revision: 1, profile_id: "audio-ownership-synthetic", known_at_accept_seq: 100, valid_at_ms: 1000, projector_version: "academic.details.v2", source_digest: "1".repeat(64) };
  const bytes = [1, 2, 3]; const digest = [...new Uint8Array(await crypto.subtle.digest("SHA-256", Uint8Array.from(bytes)))].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  const client = detailClient((_, args) => {
    const op = (args as { request: { operation: DetailDecisionRequest & { command: string; lecture_id: string } } }).request.operation;
    if (op.command === "details_read") return Promise.resolve({ version: 1, state: "ready", details: initial });
    if (op.command === "details_audio") return Promise.resolve({ version: 1, state: "ready", audio: { lecture_id: op.lecture_id, media_type: "audio/wav", content_digest: digest, total_bytes: 3, offset: 0, bytes } });
    sent.resolve(op); return response.promise;
  });
  const views = new DetailViews(client, () => {});
  try {
    const concept = initial.corpus.concepts[0]; const lecture = initial.corpus.lectures[0]; const concepts = ROUTES_BY_ID.get("learn.concepts"); const lectures = ROUTES_BY_ID.get("learn.lectures"); check(concept && lecture && concepts && lectures, "Missing audio fixtures");
    views.mount(host, detailDestination(concepts, concept.id)); await until(() => !!host.querySelector("[data-evidence]"));
    const reject = host.querySelector<HTMLButtonElement>("button[id^='decision-']"); check(reject, "Missing audio ownership rejection"); reject.click(); const op = await sent.promise;
    host.replaceChildren(); views.mount(host, detailDestination(lectures, lecture.id));
    await until(() => !button(host, "Play original audio").disabled); button(host, "Play original audio").click(); await until(() => starts === 1);
    const position = host.querySelector<HTMLInputElement>("input[aria-label='Original audio position']"); check(position, "Missing audio position"); position.focus();
    const actor = "00000000-0000-7000-8000-000000000001";
    response.resolve({ version: 1, state: "accepted", details: { ...initial, revision: 2, known_at_accept_seq: 101, decisions: [{ sequence: 101, relationId: op.relation_id, action: "REJECT", undoes: null, actor }] }, receipt_id: Array<number>(16).fill(7), decision_sequence: 101,
      receipt_decision: { sequence: 101, relation_id: op.relation_id, relation_claim_id: "00000000-0000-7000-8000-000000000002", action: "reject", undoes: null, actor },
      request_id: op.request_id, client_instance_id: op.client_instance_id, idempotency_key: op.idempotency_key, request_digest: await detailDecisionDigest(op) });
    await until(() => host.textContent.includes("Reload detail evidence when ready"));
    check(position.isConnected && document.activeElement === position && closes === 0 && starts === 1 && button(host, "Pause original audio"), "Late receipt replaced or disposed destination audio");
    passed.push("late_receipt_preserves_destination_live_audio_ownership_isolated_double");
  } finally { views.dispose(); host.remove(); window.AudioContext = nativeAudio; }
  check(audioCounts().closes === 1, "Destination audio was not disposed on departure");
  return passed;
}
