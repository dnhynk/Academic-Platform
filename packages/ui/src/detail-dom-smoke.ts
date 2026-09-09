/** Runs actual DOM interactions against the supplied client; native smoke supplies the real bridge. */
import { DetailViews } from "./detail-view.js";
import { type DetailClient, relationRejected } from "./detail-client.js";
import { allRelations, previewSourceBytes } from "./details.js";
import { detailDestination, indexDestination, type Destination } from "./destinations.js";
import { ROUTES_BY_ID } from "./routes.js";

function requireCondition(value: unknown, message: string): asserts value { if (!value) throw new Error(message); }
async function until(check: () => boolean): Promise<void> {
  const deadline = performance.now() + 10000;
  while (!check()) { if (performance.now() > deadline) throw new Error("Detail smoke did not reach the expected rendered state"); await new Promise((resolve) => { setTimeout(resolve, 20); }); }
}
function click(root: ParentNode, label: string): void {
  const button = [...root.querySelectorAll("button")].find((item) => item.textContent === label);
  requireCondition(button, `Missing detail button ${label}`); requireCondition(!button.disabled, `Disabled detail button ${label}`); button.click();
}
function element(root: ParentNode, selector: string): HTMLElement { const item = root.querySelector<HTMLElement>(selector); requireCondition(item, `Missing ${selector}`); return item; }

export async function runDetailDomSmoke(client: DetailClient): Promise<readonly string[]> {
  const passed: string[] = [];
  const initial = await client.read();
  const lecture = initial.corpus.lectures[0]; const concept = initial.corpus.concepts[0]; const project = initial.corpus.projects[0]; const question = initial.corpus.questions[0];
  requireCondition(lecture && concept && project && question, "Seed the disposable detail profile before native detail smoke");
  const host = document.createElement("main"); host.className = "detail-smoke-host"; host.setAttribute("aria-label", "Detail acceptance smoke"); document.body.append(host);
  let destination: Destination;
  const views = new DetailViews(client, (next) => { destination = next; host.replaceChildren(); views.mount(host, next); });
  async function open(routeId: string, id: string | null): Promise<void> {
    const route = ROUTES_BY_ID.get(routeId); requireCondition(route, "Unknown smoke route");
    destination = id === null ? indexDestination(route) : detailDestination(route, id);
    host.replaceChildren(); views.mount(host, destination); await until(() => host.querySelector("[data-evidence]") !== null);
  }
  function evidenceFirst(): void {
    const evidence = element(host, "[data-evidence]"); const explanation = element(host, "[data-ai-explanation]");
    requireCondition(Boolean(evidence.compareDocumentPosition(explanation) & Node.DOCUMENT_POSITION_FOLLOWING), "AI explanation preceded evidence in the DOM");
    requireCondition(explanation instanceof HTMLDetailsElement && !explanation.open, "AI explanation is not opt-in");
  }
  try {
    await open("learn.lectures", lecture.id);
    const firstSection = element(host, "[data-evidence] section"); requireCondition(firstSection.dataset.defaultView === "full-document", "Lecture default is not the full document");
    requireCondition(firstSection.querySelectorAll("[data-paragraph]").length === lecture.paragraphs.length, "Preserved document omitted paragraphs");
    passed.push("lecture_full_document_is_default");
    await until(() => host.querySelector("[data-audio-digest]") !== null);
    const audio = element(host, "[data-audio-digest]");
    requireCondition(audio.dataset.audioLecture === lecture.id && Number(audio.dataset.audioDuration) > 0 && audio.querySelector(".audio-waveform path") !== null, "Original audio did not decode into the selected lecture waveform");
    passed.push("selected_profile_original_audio_digest_and_waveform");
    const paragraph = lecture.paragraphs.at(-1); requireCondition(paragraph, "No mapped paragraph");
    element(host, `[data-paragraph='${paragraph.id}'] button`).click();
    const selected = element(host, "#raw-selection"); const segment = lecture.segments.find((item) => item.id === paragraph.segmentIds[0]); requireCondition(segment, "Missing paragraph source");
    requireCondition(selected.dataset.rawSegment === segment.id && selected.dataset.timestamp === String(segment.startMs) && selected.textContent.includes(segment.raw), "Paragraph did not open its original timestamp and raw text");
    const position = audio.querySelector<HTMLInputElement>("input[aria-label='Original audio position']"); requireCondition(position?.value === String(segment.startMs / 1000), "Player did not seek to the original audio timestamp");
    click(host, `Return to paragraph ${paragraph.id}`);
    requireCondition(document.activeElement?.closest("[data-paragraph]")?.getAttribute("data-paragraph") === paragraph.id, "Audio roundtrip lost paragraph focus");
    passed.push("paragraph_opens_audio_timestamp_and_raw_segment");
    const capture = lecture.captures[0]; requireCondition(capture, "No capture in smoke corpus");
    click(host, `Open aligned segment ${capture.segmentId}`); click(host, `Return to capture ${capture.id}`);
    requireCondition(document.activeElement.closest("[data-capture]")?.getAttribute("data-capture") === capture.id, "Capture roundtrip lost alignment");
    passed.push("capture_alignment_round_trip");
    click(host, "Raw transcript"); requireCondition(element(host, "[data-version]").dataset.version === "raw", "Raw toggle did not change transcript");
    click(host, "Corrected transcript"); requireCondition(element(host, "[data-version]").dataset.version === "corrected", "Corrected toggle did not change transcript");
    const rows = [...host.querySelectorAll<HTMLElement>("[data-section='lecture-coverage'] tbody tr")];
    requireCondition(rows.length === lecture.segments.length && new Set(rows.map((row) => row.dataset.segment)).size === lecture.segments.length, "Coverage table did not partition every source segment");
    passed.push("coverage_report_partitions_every_segment"); evidenceFirst();
    await open("learn.concepts", concept.id); evidenceFirst();
    for (const heading of ["My state and freshness", ...Object.keys(concept.relations)]) requireCondition([...host.querySelectorAll("h2")].some((item) => item.textContent === heading), `Missing concept field ${heading}`);
    passed.push("concept_detail_exposes_every_named_field");
    for (const details of host.querySelectorAll<HTMLDetailsElement>("details[data-relation]")) {
      element(details, "summary").click();
      requireCondition(details.open && ["Source", "Status", "Confidence"].every((label) => [...details.querySelectorAll("dt")].some((item) => item.textContent === label)), "Relation source/status/confidence did not open");
    }
    passed.push("relation_opens_source_status_confidence");
    requireCondition(element(host, "[data-imported-metadata]").textContent.includes("does not establish user confirmation"), "Imported metadata was presented as confirmation authority");
    requireCondition(element(host, "[data-section='concept-state']").textContent.includes(`Reported ${concept.state}`), "Imported mastery was presented as an established user state");
    passed.push("imported_metadata_does_not_establish_confirmation");
    const relation = Object.values(concept.relations).flat()[0]; requireCondition(relation, "No concept relation to reject");
    const wasRejected = relationRejected(initial, relation.id);
    const action = wasRejected ? "Undo rejection" : "Reject relation";
    click(element(host, `[data-relation='${relation.id}']`), action);
    await until(() => element(host, "#detail-status").textContent.includes("recorded by the core"));
    const changed = await client.read(); requireCondition(relationRejected(changed, relation.id) !== wasRejected, "Core read did not reflect decision");
    requireCondition(JSON.stringify(changed.decisions.slice(0, initial.decisions.length)) === JSON.stringify(initial.decisions), "A decision rewrote previous history");
    click(element(host, `[data-relation='${relation.id}']`), wasRejected ? "Reject relation" : "Undo rejection");
    await until(() => element(host, "#detail-status").textContent.includes("recorded by the core"));
    const restored = await client.read(); requireCondition(relationRejected(restored, relation.id) === wasRejected && restored.decisions.length === initial.decisions.length + 2, "Undo did not append and restore prior relation disposition");
    requireCondition(JSON.stringify(allRelations(restored.corpus)) === JSON.stringify(allRelations(initial.corpus)), "Decisions rewrote original relations");
    passed.push("relation_reject_is_append_only_with_undo");
    await open("learn.questions", null);
    for (const article of host.querySelectorAll("[data-question]")) {
      const fields = [...article.querySelectorAll("dt")].map((item) => item.textContent);
      requireCondition(fields.indexOf("Active goal relevance") < fields.indexOf("Age") && fields.indexOf("Upcoming context") < fields.indexOf("Age"), "Question age preceded active context");
    }
    await open("learn.questions", question.id); evidenceFirst();
    requireCondition(host.querySelectorAll("[data-section='question-timeline'] tbody tr").length === question.revisions.length, "Question timeline omitted revisions");
    passed.push("question_workspace_context_and_parallel_timeline");
    await open("build.projects", project.id); evidenceFirst();
    const banner = element(host, "[data-stale-snapshot]"); requireCondition(getComputedStyle(banner).position === "sticky" && banner.textContent.includes(project.snapshot) && banner.textContent.includes(project.capturedAt), "Stale snapshot has no persistent commit/time banner");
    host.scrollTop = host.scrollHeight;
    requireCondition(banner.getBoundingClientRect().top >= host.getBoundingClientRect().top && banner.getBoundingClientRect().top < host.getBoundingClientRect().top + 100, "Stale banner disappeared while scrolling");
    passed.push("stale_snapshot_banner_is_persistent");
    click(host, "Inspect frozen analysis · read-only"); requireCondition(element(host, "[data-section='project-analyze']").textContent.includes(project.snapshot) && element(host, "[data-section='project-analyze']").textContent.includes("A new analysis run is unavailable"), "Read-only inspection is disconnected from the snapshot or implies a new analysis run");
    passed.push("project_analyze_is_labelled_read_only");
    const file = project.files[0]; requireCondition(file, "Missing frozen project file");
    const form = element(host, "[data-section='project-egress'] form"); requireCondition(form instanceof HTMLFormElement, "Expected native form"); form.requestSubmit();
    const preview = previewSourceBytes(project, [{ path: file.path, start: 0, end: new TextEncoder().encode(file.text).length }]);
    requireCondition(element(host, "[data-preview-hex]").textContent === preview.bytes.map((byte) => byte.toString(16).padStart(2, "0")).join(" "), "Rendered byte preview changed exact payload bytes");
    requireCondition(element(host, "[data-section='project-egress']").textContent.includes("No bytes transmitted"), "Preview pretended to transmit");
    passed.push("source_preview_equals_rendered_bytes", "ai_explanation_never_precedes_evidence");
    return passed;
  } finally { views.dispose(); host.remove(); }
}
