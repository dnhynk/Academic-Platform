/** Native HTML detail surfaces, supplied only by the core read-model client. */
import { detailDestination, type Destination } from "./destinations.js";
import { type DetailClient, type DetailSelector, type DetailState, relationRejected } from "./detail-client.js";
import { ROUTES_BY_ID } from "./routes.js";
import { coverage, previewSourceBytes, questionGroups, selectCapture, selectParagraph, selectSegment, staleSnapshot, timecode, type AudioSelection, type Concept, type Lecture, type Project, type Question, type Relation } from "./details.js";
import { lecturePlayer, type LecturePlayer } from "./detail-audio.js";

function node<K extends keyof HTMLElementTagNameMap>(tag: K, value = ""): HTMLElementTagNameMap[K] { const element = document.createElement(tag); element.textContent = value; return element; }
function button(label: string, action: () => void, id?: string): HTMLButtonElement {
  const element = node("button", label); element.type = "button"; if (id) element.id = id; element.addEventListener("click", action); return element;
}
function section(parent: HTMLElement, heading: string, id: string): HTMLElement {
  const element = node("section"); element.dataset.section = id; element.append(node("h2", heading)); parent.append(element); return element;
}
function list(parent: HTMLElement, values: readonly string[]): void { const ul = node("ul"); for (const value of values) ul.append(node("li", value)); parent.append(ul); }
function fields(parent: HTMLElement, values: Readonly<Record<string, string>>): void { const dl = node("dl"); for (const [key, value] of Object.entries(values)) dl.append(node("dt", key), node("dd", value)); parent.append(dl); }
function explanation(parent: HTMLElement, value: string): void {
  const details = node("details"); details.dataset.aiExplanation = "true";
  details.append(node("summary", "AI explanation · optional"), node("p", value)); parent.append(details);
}
const DETAIL_ROUTES = new Set(["learn.lectures", "learn.questions", "build.projects"]);
const CONCEPT_GROUPS = ["Evidence timeline", "Contradictions", "Prerequisites", "Used in", "Open questions", "SNU courses, lectures and assessments", "Projects", "Competencies", "Roles"];
const PROJECT_GROUPS = ["Architecture map", "ADR / spec / code drift", "OBSERVED", "REQUIRED", "WOULD_BENEFIT_FROM", "Open questions and issues / incidents", "Active Critical Path", "Build → Learn branch", "SNU Course / Offering and external options", "Competency evidence and authorship", "Snapshot semantic diff", "Stack inventory (supporting information)"];
export function isDetailSurface(destination: Destination): boolean {
  return DETAIL_ROUTES.has(destination.routeId) || (destination.routeId === "learn.concepts" && destination.entityId !== null);
}

export class DetailViews {
  readonly #client: DetailClient;
  readonly #navigate: (destination: Destination) => void;
  readonly #onState: (() => void) | undefined;
  #state: DetailState | null = null;
  #generation = 0;
  #pending = false;
  #root: HTMLElement | null = null;
  #destination: Destination | null = null;
  #message = "";
  #relationOrdinal = 0;
  #player: LecturePlayer | null = null;
  #selector: DetailSelector = { view: "detail_workspace", known_at_accept_seq: null, valid_at_ms: null };
  constructor(client: DetailClient, navigate: (destination: Destination) => void, onState?: () => void) { this.#client = client; this.#navigate = navigate; this.#onState = onState; }
  acceptedState(): DetailState | null { return this.#state; }
  mount(parent: HTMLElement, destination: Destination): boolean {
    this.#clearPlayer();
    this.#generation++;
    this.#root = null;
    this.#destination = destination;
    if (!isDetailSurface(destination)) return false;
    const root = node("div"); root.className = "detail-surface"; parent.append(root); this.#root = root;
    if (this.#state) this.#render();
    else void this.reload();
    return true;
  }
  async reload(selector = this.#selector): Promise<void> {
    if (this.#pending) return;
    const generation = ++this.#generation;
    const root = this.#root;
    if (!root) return;
    this.#state = null; this.#onState?.();
    this.#clearPlayer(); root.replaceChildren(node("p", "Loading detail evidence from the local service…")); root.setAttribute("aria-busy", "true");
    try {
      const state = await this.#client.read(selector);
      if (generation !== this.#generation) return;
      this.#selector = { ...selector }; this.#state = state; this.#message = ""; this.#render(); this.#onState?.();
    } catch (error) {
      if (generation !== this.#generation) return;
      const status = node("p", String(error)); status.setAttribute("role", "status");
      root.replaceChildren(node("h2", "Detail evidence unavailable"), status, button("Reload detail evidence", () => { void this.reload(); }));
    } finally { root.removeAttribute("aria-busy"); }
  }
  #open(routeId: string, id: string): void { const route = ROUTES_BY_ID.get(routeId); if (!route) throw new Error("Missing relation destination"); this.#navigate(detailDestination(route, id)); }
  #clearPlayer(): void { this.#player?.dispose(); this.#player = null; }
  dispose(): void { this.#generation++; this.#clearPlayer(); this.#root = null; }
  #render(): void {
    const root = this.#root; const state = this.#state; const destination = this.#destination;
    if (!root || !state || !destination) return;
    const activeId = document.activeElement?.id;
    const main = root.closest("main"); const scrollTop = main?.scrollTop ?? 0;
    this.#clearPlayer(); root.replaceChildren();
    this.#relationOrdinal = 0;
    const status = node("p", this.#message || `Core projection revision ${String(state.revision)} · synthetic data only`); status.id = "detail-status"; status.setAttribute("role", "status"); root.append(status, button("Reload detail evidence", () => { void this.reload(); }));
    const history = node("details"); history.append(node("summary", "View current or historical evidence"));
    const form = node("form");
    const known = node("input"); known.type = "number"; known.min = "0"; known.step = "1"; known.id = "detail-known"; known.value = this.#selector.known_at_accept_seq?.toString() ?? "";
    const valid = node("input"); valid.type = "number"; valid.min = "0"; valid.step = "1"; valid.id = "detail-valid"; valid.value = this.#selector.valid_at_ms?.toString() ?? "";
    const knownLabel = node("label", "Known acceptance sequence (blank for current)"); knownLabel.htmlFor = known.id;
    const validLabel = node("label", "Valid time in milliseconds (blank for current)"); validLabel.htmlFor = valid.id;
    const apply = node("button", "Load selected time"); apply.type = "submit"; apply.disabled = this.#pending;
    form.append(knownLabel, known, validLabel, valid, apply);
    form.addEventListener("submit", (event) => { event.preventDefault(); void this.reload({ view: "detail_workspace", known_at_accept_seq: known.value === "" ? null : Number(known.value), valid_at_ms: valid.value === "" ? null : Number(valid.value) }); });
    history.append(form, button("Return to current evidence", () => { void this.reload({ view: "detail_workspace", known_at_accept_seq: null, valid_at_ms: null }); })); root.append(history);
    const historical = this.#selector.known_at_accept_seq !== null || this.#selector.valid_at_ms !== null;
    if (historical) root.append(node("p", "Historical evidence is read-only. Return to current evidence to make or confirm a decision."));
    for (const pending of this.#client.pendingDecisions()) {
      const retry = button(`Confirm original ${pending.action} request · ${pending.relation_id}`, () => { void this.#decide(pending.relation_id, pending.action, "", pending.expected_revision); });
      retry.dataset.pendingRequest = "true"; retry.disabled = this.#pending || historical;
      root.append(node("p", "A prior request is unconfirmed. This retry keeps its original profile, revision and request identity, even if the source has changed."), retry);
    }
    const provenance = node("p", "Imported snapshot · Reported by importer. This does not establish user confirmation.");
    provenance.dataset.importedMetadata = "true"; root.append(provenance);
    const sourceDetails = node("details"); sourceDetails.append(node("summary", "Source details"));
    fields(sourceDetails, { "Source projection": state.projector_version, Profile: state.profile_id, "Known acceptance sequence": String(state.known_at_accept_seq), "Valid time (milliseconds)": String(state.valid_at_ms), "Source digest": state.source_digest, "Source authority": "Importer observation · DIRECT_OBSERVATION / CODE_OBSERVED" });
    sourceDetails.append(node("p", "States, confidence and classifications are reported metadata. Accepted rejection and undo receipts are separate from this import.")); root.append(sourceDetails);
    const evidence = node("div"); evidence.dataset.evidence = "true"; root.append(evidence);
    const id = destination.entityId;
    if (destination.routeId === "learn.lectures") {
      const lecture = state.corpus.lectures.find((item) => item.id === id);
      if (id === null) this.#index(evidence, "Lectures", state.corpus.lectures, "learn.lectures");
      else if (lecture) { this.#lecture(evidence, lecture); explanation(root, lecture.explanation); }
      else this.#missing(evidence);
    } else if (destination.routeId === "learn.concepts") {
      const concept = state.corpus.concepts.find((item) => item.id === id);
      if (concept) { this.#concept(evidence, concept); explanation(root, concept.explanation); } else this.#missing(evidence);
    } else if (destination.routeId === "learn.questions") {
      if (id === null) this.#questions(evidence, state.corpus.questions);
      else { const question = state.corpus.questions.find((item) => item.id === id); if (question) { this.#question(evidence, question); explanation(root, question.explanation); } else this.#missing(evidence); }
    } else if (destination.routeId === "build.projects") {
      const project = state.corpus.projects.find((item) => item.id === id);
      if (id === null) this.#index(evidence, "Projects", state.corpus.projects, "build.projects");
      else if (project) { this.#project(evidence, project); explanation(root, project.explanation); } else this.#missing(evidence);
    }
    if (activeId) [...root.querySelectorAll<HTMLElement>("[id]")].find((element) => element.id === activeId)?.focus({ preventScroll: true });
    if (main) main.scrollTop = scrollTop;
  }
  #missing(parent: HTMLElement): void { parent.append(node("p", "This detail is absent from the accepted projection."), button("Reload detail evidence", () => { void this.reload(); })); }
  #index(parent: HTMLElement, heading: string, items: readonly { readonly id: string; readonly title: string }[], routeId: string): void {
    const region = section(parent, heading, "index");
    if (!items.length) { region.append(node("p", "No accepted detail records in this profile."), button("Reload detail evidence", () => { void this.reload(); })); return; }
    const ul = node("ul"); for (const item of items) { const li = node("li"); li.append(button(item.title, () => { this.#open(routeId, item.id); })); ul.append(li); } region.append(ul);
  }
  #relation(parent: HTMLElement, relation: Relation): void {
    const state = this.#state; if (!state) return;
    const rejected = relationRejected(state, relation.id);
    const details = node("details"); details.dataset.relation = relation.id;
    details.append(node("summary", `${relation.label} · ${rejected ? "REJECTED" : `Reported ${relation.status}`}`));
    fields(details, { Source: relation.source.title, Locator: relation.source.locator, Status: rejected ? `REJECTED · original reported ${relation.status} preserved` : `Reported ${relation.status} · confirmation authority unavailable`, Confidence: `Reported ${relation.confidence}` });
    details.append(node("pre", relation.source.content));
    if (relation.target) {
      const target = relation.target;
      if (ROUTES_BY_ID.get(target.routeId)?.detailParam) details.append(button("Open linked detail", () => { this.#open(target.routeId, target.id); }));
      else details.append(node("p", "The linked destination is unavailable in this desktop version; its source remains visible above."));
    }
    this.#relationOrdinal++;
    const controlId = `decision-${String(this.#relationOrdinal)}`;
    const decision = button(rejected ? "Undo rejection" : "Reject relation", () => { void this.#decide(relation.id, rejected ? "undo" : "reject", controlId); }, controlId);
    decision.disabled = this.#pending || this.#client.pendingDecisions().length > 0 || this.#selector.known_at_accept_seq !== null || this.#selector.valid_at_ms !== null; details.append(decision);
    if (decision.disabled) details.append(node("p", "Decisions require current evidence and confirmation of any original pending request."));
    const history = state.decisions.filter((event) => event.relationId === relation.id);
    if (history.length) list(details, history.map((event) => `#${String(event.sequence)} ${event.action} · ${event.actor}${event.undoes === null ? "" : ` · undoes #${String(event.undoes)}`}`));
    parent.append(details);
  }
  async #decide(relationId: string, action: "reject" | "undo", controlId: string, expectedRevision?: number): Promise<void> {
    if (this.#pending || !this.#state) return;
    this.#pending = true;
    const root = this.#root; const revision = expectedRevision ?? this.#state.revision; const generation = this.#generation;
    for (const control of root?.querySelectorAll<HTMLButtonElement>("button[id^='decision-']") ?? []) control.disabled = true;
    const status = root?.querySelector("#detail-status"); if (status) status.textContent = "Waiting for the core to confirm this decision…";
    try {
      this.#state = await this.#client.decide(relationId, action, revision);
      this.#message = `${action === "reject" ? "Rejection" : "Undo"} recorded by the core for the requested source. Revision ${String(this.#state.revision)} shows the current snapshot; a replaced source keeps its own disposition.`;
    } catch (error) { this.#message = `${String(error)} No receipt confirmation is displayed. Use the original pending request to retry if one remains.`; }
    finally {
      this.#pending = false;
      if (generation !== this.#generation) { if (this.#root) void this.reload(); }
      else {
        this.#render();
        this.#onState?.();
        const originalControl = controlId ? this.#root?.querySelector<HTMLButtonElement>(`#${controlId}`) : null;
        const detail = originalControl?.closest("details");
        if (detail) { detail.open = true; detail.querySelector<HTMLButtonElement>("button[id^='decision-']")?.focus({ preventScroll: true }); }
      }
    }
  }
  #groups(parent: HTMLElement, groups: Readonly<Record<string, readonly Relation[]>>, order: readonly string[] = []): void {
    for (const heading of new Set([...order, ...Object.keys(groups)])) {
      const relations = groups[heading] ?? [];
      const region = section(parent, heading, heading);
      for (const relation of relations) this.#relation(region, relation);
      if (!relations.length) region.append(node("p", "No evidence in this projection."));
    }
  }
  #lecture(parent: HTMLElement, lecture: Lecture): void {
    const documentRegion = section(parent, "Full preserved document", "lecture-document");
    documentRegion.dataset.defaultView = "full-document";
    const rows = coverage(lecture); const unmapped = rows.filter((row) => row.status === "UNMAPPED").length;
    documentRegion.append(node("p", unmapped ? `INCOMPLETE · ${String(unmapped)} unmapped segment. Every segment is listed in the coverage report.` : "Every segment has a disposition; see coverage evidence."));
    const audioRegion = section(parent, "Original audio and timecodes", "lecture-audio");
    audioRegion.append(node("p", "Original audio is resolved from this lecture in the selected profile. Full-length streaming and codecs beyond bounded WAV remain unavailable."));
    try { this.#player = lecturePlayer(audioRegion, this.#client, lecture.id); }
    catch { audioRegion.append(node("p", "Audio playback is unavailable in this runtime. Raw segments and timecodes remain available.")); }
    const audioStatus = node("p", "Choose a paragraph, raw segment or capture to seek the original timestamp."); audioStatus.setAttribute("role", "status"); audioRegion.append(audioStatus);
    const rawSelection = node("div"); rawSelection.id = "raw-selection"; audioRegion.append(rawSelection);
    const select = (selection: AudioSelection): void => {
      const segment = selection.segment;
      this.#player?.seek(segment.startMs);
      audioStatus.textContent = `${timecode(segment.startMs)}–${timecode(segment.endMs)} · raw ${segment.id}`;
      rawSelection.dataset.rawSegment = segment.id; rawSelection.dataset.timestamp = String(segment.startMs);
      rawSelection.replaceChildren(node("h3", `Raw segment ${segment.id}`), node("p", segment.raw || segment.reason || "No raw text is available."));
      for (const id of selection.paragraphIds) rawSelection.append(button(`Return to paragraph ${id}`, () => { const paragraph = [...documentRegion.querySelectorAll<HTMLElement>("[data-paragraph]")].find((item) => item.dataset.paragraph === id); paragraph?.scrollIntoView({ block: "nearest" }); paragraph?.querySelector("button")?.focus(); }));
      for (const id of selection.captureIds) rawSelection.append(button(`Return to capture ${id}`, () => { [...parent.querySelectorAll<HTMLElement>("[data-capture]")].find((item) => item.dataset.capture === id)?.querySelector("button")?.focus(); }));
      rawSelection.scrollIntoView({ block: "nearest" });
    };
    for (const paragraph of lecture.paragraphs) {
      const article = node("article"); article.dataset.paragraph = paragraph.id;
      article.append(node("p", paragraph.text));
      for (const selection of selectParagraph(lecture, paragraph.id)) article.append(button(`Open original audio · ${timecode(selection.segment.startMs)} · ${selection.segment.id}`, () => { select(selection); }));
      documentRegion.append(article);
    }
    const transcript = section(parent, "Transcript versions", "lecture-transcript");
    const controls = node("div"); controls.className = "actions";
    const content = node("div"); let version: "raw" | "corrected" = "corrected";
    const renderTranscript = (): void => {
      content.dataset.version = version; content.replaceChildren();
      raw.setAttribute("aria-pressed", String(version === "raw")); corrected.setAttribute("aria-pressed", String(version === "corrected"));
      for (const segment of lecture.segments) { const row = node("p"); row.append(button(`${timecode(segment.startMs)} · ${segment.id}`, () => { select(selectSegment(lecture, segment.id)); }), node("span", ` ${segment[version] || segment.reason || "Unmapped"}`)); content.append(row); }
    };
    const raw = button("Raw transcript", () => { version = "raw"; renderTranscript(); });
    const corrected = button("Corrected transcript", () => { version = "corrected"; renderTranscript(); });
    controls.append(raw, corrected); transcript.append(controls, content); renderTranscript();
    const captures = section(parent, "Captures and segment alignment", "lecture-captures");
    for (const capture of lecture.captures) { const figure = node("figure"); figure.dataset.capture = capture.id; figure.append(node("figcaption", `${capture.title} · ${timecode(capture.atMs)}`), node("pre", capture.text), button(`Open aligned segment ${capture.segmentId}`, () => { select(selectCapture(lecture, capture.id)); })); captures.append(figure); }
    const review = section(parent, "Review queues", "lecture-review");
    for (const kind of ["Mark Moment", "Low confidence", "Equation", "Code"]) { review.append(node("h3", kind)); for (const item of lecture.review.filter((entry) => entry.kind === kind)) review.append(node("p", item.note), button(`Review ${item.id} · ${item.segmentId}`, () => { select(selectSegment(lecture, item.segmentId)); })); }
    this.#groups(parent, lecture.links);
    const report = section(parent, "Coverage report · exhaustive transcript partition", "lecture-coverage");
    const table = node("table"); const head = node("tr"); for (const heading of ["Raw segment", "Time", "Status", "Mapping or disposition evidence"]) { const th = node("th", heading); th.scope = "col"; head.append(th); } const thead = node("thead"); thead.append(head); table.append(thead);
    const body = node("tbody"); for (const row of rows) { const tr = node("tr"); tr.dataset.segment = row.segment.id; const cell = node("td"); cell.append(button(row.segment.id, () => { select(selectSegment(lecture, row.segment.id)); })); tr.append(cell, node("td", timecode(row.segment.startMs)), node("td", row.status), node("td", row.paragraphIds.join(", ") || row.segment.reason || "No mapping; document remains incomplete")); body.append(tr); } table.append(body); report.append(table, node("p", `${String(rows.length)} / ${String(lecture.segments.length)} segments accounted for; ${String(unmapped)} unmapped. Excluded non-speech segments remain in the denominator.`));
  }
  #concept(parent: HTMLElement, concept: Concept): void {
    const state = section(parent, "My state and freshness", "concept-state");
    fields(state, { "My state": `Reported ${concept.state}`, Confidence: `Reported ${concept.confidence}`, Freshness: `Reported ${concept.freshness}`, "Last strong evidence": `Reported ${concept.lastStrongEvidence}` });
    this.#groups(parent, concept.relations, CONCEPT_GROUPS);
  }
  #questions(parent: HTMLElement, questions: readonly Question[]): void {
    for (const [heading, group] of questionGroups(questions)) {
      const region = section(parent, heading, `question-group-${heading}`);
      if (!group.length) region.append(node("p", "No questions in this group."));
      for (const question of group) {
        const article = node("article"); article.dataset.question = question.id;
        fields(article, { "Active goal relevance": question.goalRelevance, "Upcoming context": question.nextContext, Age: `${String(question.ageDays)} days`, Origin: question.origin, Status: `Reported ${question.status}` });
        article.append(button(question.revisions.at(-1)?.text ?? question.id, () => { this.#open("learn.questions", question.id); })); region.append(article);
      }
    }
  }
  #question(parent: HTMLElement, question: Question): void {
    const context = section(parent, "Question context", "question-context");
    fields(context, { "Active goal relevance": question.goalRelevance, "Upcoming context": question.nextContext, Age: `${String(question.ageDays)} days`, Origin: question.origin, Status: `Reported ${question.status}` });
    const evidence = section(parent, "Origin and evidence", "question-evidence"); for (const relation of question.evidence) this.#relation(evidence, relation);
    const timeline = section(parent, "Question timeline · parallel revision comparison", "question-timeline");
    const table = node("table"); const head = node("tr"); for (const heading of ["Recorded at", "Question text", "Linked concepts", "Resolution evidence"]) { const th = node("th", heading); th.scope = "col"; head.append(th); } const thead = node("thead"); thead.append(head); table.append(thead);
    const body = node("tbody"); for (const revision of question.revisions) { const row = node("tr"); const concepts = node("td"); const resolution = node("td"); for (const relation of revision.concepts) this.#relation(concepts, relation); for (const relation of revision.resolutionEvidence) this.#relation(resolution, relation); if (!revision.resolutionEvidence.length) resolution.append(node("p", "No resolution evidence")); row.append(node("td", revision.at), node("td", revision.text), concepts, resolution); body.append(row); } table.append(body); timeline.append(table);
  }
  #project(parent: HTMLElement, project: Project): void {
    const banner = staleSnapshot(project);
    if (banner) { const sticky = node("p", banner); sticky.className = "stale-snapshot"; sticky.dataset.staleSnapshot = project.snapshot; parent.append(sticky); }
    const goals = section(parent, "Project goal and success criteria", "project-goals"); goals.append(node("p", project.goal)); list(goals, project.successCriteria);
    const snapshot = section(parent, "Repository snapshot", "project-snapshot"); fields(snapshot, { Repository: project.repository, Branch: project.branch, Snapshot: project.snapshot, "Captured at": project.capturedAt, "Dirty working tree": project.dirty ? "Yes · frozen snapshot includes uncommitted changes" : "No" });
    this.#groups(parent, project.relations, PROJECT_GROUPS);
    const analysis = section(parent, "Analyze · read-only", "project-analyze");
    analysis.append(node("p", "A new analysis run is unavailable. Inspect the imported findings for this frozen snapshot; this view cannot edit repository files or run a build."));
    const result = node("div");
    analysis.append(button("Inspect frozen analysis · read-only", () => {
      result.replaceChildren(node("p", `Read-only inspection of imported snapshot ${project.snapshot} · ${project.capturedAt}`));
      for (const classification of ["OBSERVED", "REQUIRED", "WOULD_BENEFIT_FROM"]) {
        result.append(node("p", `${classification}: ${String(project.relations[classification]?.length ?? 0)} reported findings in the imported snapshot`), button(`Inspect ${classification} findings`, () => {
          const region = [...parent.querySelectorAll<HTMLElement>("section[data-section]")].find((item) => item.dataset.section === classification);
          region?.scrollIntoView({ block: "nearest" }); region?.querySelector<HTMLElement>("summary")?.focus();
        }));
      }
    }), result);
    const preview = section(parent, "Source byte preview", "project-egress");
    preview.append(node("p", "Provider preview is unavailable. Inspect local UTF-8 source ranges below; this does not authorize transmission."));
    const form = node("form"); const file = node("select"); file.id = "preview-file"; const fileLabel = node("label", "Frozen file"); fileLabel.htmlFor = file.id;
    for (const item of project.files) { const option = node("option", item.path); option.value = item.path; file.append(option); }
    const start = node("input"); start.type = "number"; start.min = "0"; start.step = "1"; start.value = "0"; start.id = "preview-start";
    const end = node("input"); end.type = "number"; end.min = "1"; end.step = "1"; end.id = "preview-end";
    const initial = project.files[0]; end.value = String(initial ? new TextEncoder().encode(initial.text).length : 0);
    const startLabel = node("label", "Start byte (inclusive)"); startLabel.htmlFor = start.id; const endLabel = node("label", "End byte (exclusive)"); endLabel.htmlFor = end.id;
    file.addEventListener("change", () => { const selected = project.files.find((item) => item.path === file.value); start.value = "0"; end.value = String(selected ? new TextEncoder().encode(selected.text).length : 0); });
    const output = node("div"); const error = node("p"); error.id = "preview-error"; error.setAttribute("role", "status"); start.setAttribute("aria-describedby", error.id); end.setAttribute("aria-describedby", error.id);
    const submit = node("button", "Preview exact bytes"); submit.type = "submit";
    form.append(fileLabel, file, startLabel, start, endLabel, end, submit, error);
    form.addEventListener("submit", (event) => {
      event.preventDefault(); error.textContent = ""; output.replaceChildren(); start.removeAttribute("aria-invalid"); end.removeAttribute("aria-invalid");
      try {
        const frozen = previewSourceBytes(project, [{ path: file.value, start: Number(start.value), end: Number(end.value) }]);
        const bytes = Uint8Array.from(frozen.bytes);
        fields(output, { Snapshot: frozen.snapshot, Range: `${file.value} [${start.value}, ${end.value})`, "Exact source byte length": `${String(bytes.length)} bytes`, Encoding: frozen.encoding });
        const hex = node("pre", [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join(" ")); hex.dataset.previewHex = "true";
        output.append(node("h3", "Exact bytes · hexadecimal"), hex, node("h3", "UTF-8 reading aid"), node("pre", new TextDecoder().decode(bytes)), node("p", "No bytes transmitted. External provider is unavailable."));
      } catch (failure) { error.textContent = String(failure); start.setAttribute("aria-invalid", "true"); end.setAttribute("aria-invalid", "true"); }
    });
    preview.append(form, output);
  }
}
