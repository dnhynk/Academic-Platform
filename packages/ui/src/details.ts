/** Detail read models. Coordinates belong to frozen synthetic sources, never to AI prose. */
export type CoverageStatus = "MAPPED" | "UNMAPPED" | "EXCLUDED_NON_SPEECH" | "REDACTED_WITH_POLICY" | "UNTRANSCRIBED_FAILURE";
export interface Source {
  readonly id: string;
  readonly title: string;
  readonly locator: string;
  readonly content: string;
  readonly href?: string;
}
export interface Relation {
  readonly id: string;
  readonly label: string;
  readonly source: Source;
  readonly status: "PROPOSED" | "CONFIRMED" | "CONTESTED";
  readonly confidence: string;
  readonly target?: { readonly routeId: string; readonly id: string };
}
export interface Segment {
  readonly id: string;
  readonly startMs: number;
  readonly endMs: number;
  readonly raw: string;
  readonly corrected: string;
  readonly disposition: Exclude<CoverageStatus, "MAPPED"> | null;
  readonly reason: string | null;
}
export interface Paragraph { readonly id: string; readonly text: string; readonly segmentIds: readonly string[] }
export interface Capture { readonly id: string; readonly title: string; readonly text: string; readonly atMs: number; readonly segmentId: string }
export interface ReviewItem { readonly id: string; readonly kind: "Mark Moment" | "Low confidence" | "Equation" | "Code"; readonly segmentId: string; readonly note: string }
export interface Lecture {
  readonly id: string; readonly title: string; readonly segments: readonly Segment[]; readonly paragraphs: readonly Paragraph[];
  readonly captures: readonly Capture[]; readonly review: readonly ReviewItem[];
  readonly links: Readonly<Record<string, readonly Relation[]>>; readonly explanation: string;
}
export interface Concept {
  readonly id: string; readonly title: string; readonly state: string; readonly confidence: string;
  readonly freshness: string; readonly lastStrongEvidence: string;
  readonly relations: Readonly<Record<string, readonly Relation[]>>; readonly explanation: string;
}
export type QuestionStatus = "OPEN" | "PARTIAL" | "RESOLVED" | "REFRAMED";
export interface QuestionRevision { readonly at: string; readonly text: string; readonly concepts: readonly Relation[]; readonly resolutionEvidence: readonly Relation[] }
export interface Question {
  readonly id: string; readonly origin: string; readonly isNew: boolean; readonly status: QuestionStatus;
  readonly goalRelevance: string; readonly goalRank: number; readonly nextContext: string; readonly ageDays: number;
  readonly revisions: readonly QuestionRevision[]; readonly evidence: readonly Relation[]; readonly explanation: string;
}
export interface Project {
  readonly id: string; readonly title: string; readonly goal: string; readonly successCriteria: readonly string[];
  readonly repository: string; readonly branch: string; readonly snapshot: string; readonly currentSnapshot: string;
  readonly capturedAt: string; readonly currentAt: string; readonly dirty: boolean;
  readonly relations: Readonly<Record<string, readonly Relation[]>>;
  readonly files: readonly { readonly path: string; readonly text: string }[]; readonly explanation: string;
}
export interface DetailCorpus {
  readonly lectures: readonly Lecture[]; readonly concepts: readonly Concept[];
  readonly questions: readonly Question[]; readonly projects: readonly Project[];
}
/** The frame names the same detail regions rendered by the native surface. */
export function detailSections(routeId: string, detail: boolean): readonly { readonly id: string; readonly heading: string; readonly filledBy: string }[] {
  const headings = routeId === "learn.lectures" ? (detail ? ["Full preserved document", "Original audio and timecodes", "Transcript versions", "Captures and segment alignment", "Review queues", "Coverage report"] : ["Lectures"])
    : routeId === "learn.concepts" ? ["My state and freshness", "Evidence timeline", "Contradictions", "Prerequisites", "Used in", "Open questions", "SNU courses, lectures and assessments", "Projects", "Competencies", "Roles"]
      : routeId === "learn.questions" ? (detail ? ["Question context", "Origin and evidence", "Question timeline"] : ["Inbox by origin", "Open", "Partial", "Resolved", "Reframed"])
        : detail ? ["Project goal and success criteria", "Repository snapshot", "Architecture map", "ADR / spec / code drift", "OBSERVED", "REQUIRED", "WOULD_BENEFIT_FROM", "Analyze · read-only", "External provider byte preview"] : ["Projects"];
  return headings.map((heading, i) => ({ id: `${routeId}.${String(i)}`, heading, filledBy: "P2-X4" }));
}
export interface CoverageRow { readonly segment: Segment; readonly status: CoverageStatus; readonly paragraphIds: readonly string[] }

/** Enumerate the whole transcript; neither salience nor a supplied denominator is accepted. */
export function coverage(lecture: Lecture): readonly CoverageRow[] {
  const known = new Set(lecture.segments.map((segment) => segment.id));
  if (known.size !== lecture.segments.length) throw new Error("Duplicate transcript segment");
  for (const paragraph of lecture.paragraphs) {
    if (!paragraph.segmentIds.length || paragraph.segmentIds.some((id) => !known.has(id))) throw new Error("Paragraph has no resolvable raw segment");
  }
  return lecture.segments.map((segment) => {
    const paragraphIds = lecture.paragraphs.filter((paragraph) => paragraph.segmentIds.includes(segment.id)).map((paragraph) => paragraph.id);
    if (paragraphIds.length && segment.disposition !== null) throw new Error("Segment has both mapping and disposition");
    if (segment.disposition && segment.disposition !== "UNMAPPED" && !segment.reason) throw new Error("Disposition lacks evidence");
    if (paragraphIds.length) {
      const rendered = lecture.paragraphs.filter((paragraph) => paragraphIds.includes(paragraph.id)).map((paragraph) => paragraph.text).join(" ").split(/\s+/u);
      let at = 0;
      for (const token of segment.corrected.split(/\s+/u).filter(Boolean)) {
        const found = rendered.indexOf(token, at);
        if (found < 0) throw new Error("Mapped document omits corrected transcript text");
        at = found + 1;
      }
    }
    return { segment, status: paragraphIds.length ? "MAPPED" : segment.disposition ?? "UNMAPPED", paragraphIds };
  });
}
export interface AudioSelection { readonly segment: Segment; readonly paragraphIds: readonly string[]; readonly captureIds: readonly string[] }
export function selectSegment(lecture: Lecture, segmentId: string): AudioSelection {
  const segment = lecture.segments.find((item) => item.id === segmentId);
  if (!segment) throw new Error("Raw segment unavailable");
  return { segment, paragraphIds: lecture.paragraphs.filter((p) => p.segmentIds.includes(segmentId)).map((p) => p.id), captureIds: lecture.captures.filter((c) => c.segmentId === segmentId).map((c) => c.id) };
}
export function selectParagraph(lecture: Lecture, id: string): readonly AudioSelection[] {
  const paragraph = lecture.paragraphs.find((item) => item.id === id);
  if (!paragraph) throw new Error("Paragraph unavailable");
  return paragraph.segmentIds.map((segmentId) => selectSegment(lecture, segmentId));
}
export function selectCapture(lecture: Lecture, id: string): AudioSelection {
  const capture = lecture.captures.find((item) => item.id === id);
  if (!capture) throw new Error("Capture unavailable");
  const selection = selectSegment(lecture, capture.segmentId);
  if (capture.atMs < selection.segment.startMs || capture.atMs >= selection.segment.endMs) throw new Error("Capture lies outside aligned raw segment");
  return selection;
}

export interface RelationDecision {
  readonly sequence: number; readonly relationId: string; readonly action: "REJECT" | "UNDO";
  readonly undoes: number | null; readonly actor: string;
}
export function allRelations(corpus: DetailCorpus): readonly Relation[] {
  const relations = [
    ...corpus.lectures.flatMap((lecture) => Object.values(lecture.links).flat()),
    ...corpus.concepts.flatMap((concept) => Object.values(concept.relations).flat()),
    ...corpus.projects.flatMap((project) => Object.values(project.relations).flat()),
    ...corpus.questions.flatMap((question) => [...question.evidence, ...question.revisions.flatMap((revision) => [...revision.concepts, ...revision.resolutionEvidence])]),
  ];
  const unique = new Map<string, Relation>();
  const signature = (relation: Relation): string => JSON.stringify([relation.label, relation.status, relation.confidence, relation.source.id, relation.source.title, relation.source.locator, relation.source.content, relation.source.href ?? null, relation.target?.routeId ?? null, relation.target?.id ?? null]);
  for (const relation of relations) {
    const prior = unique.get(relation.id);
    if (prior && signature(prior) !== signature(relation)) throw new Error(`Conflicting definitions for relation ${relation.id}`);
    unique.set(relation.id, relation);
  }
  return [...unique.values()];
}
export function questionGroups(questions: readonly Question[]): ReadonlyMap<string, readonly Question[]> {
  const groups = new Map<string, Question[]>();
  for (const origin of [...new Set(questions.filter((q) => q.isNew).map((q) => q.origin))].sort()) groups.set(`Inbox · ${origin}`, []);
  for (const status of ["OPEN", "PARTIAL", "RESOLVED", "REFRAMED"]) groups.set(status, []);
  for (const question of questions) groups.get(question.isNew ? `Inbox · ${question.origin}` : question.status)?.push(question);
  for (const group of groups.values()) group.sort((a, b) => b.goalRank - a.goalRank || a.ageDays - b.ageDays || a.id.localeCompare(b.id));
  return groups;
}
export function staleSnapshot(project: Project): string | null {
  return project.snapshot === project.currentSnapshot ? null : `Stale analysis · commit ${project.snapshot} · captured ${project.capturedAt} · current ${project.currentSnapshot} at ${project.currentAt}`;
}
export interface ByteRange { readonly path: string; readonly start: number; readonly end: number }
export interface SourceBytePreview {
  readonly snapshot: string; readonly ranges: readonly ByteRange[]; readonly bytes: readonly number[];
  readonly encoding: "UTF-8";
}
/** Exact source bytes for local inspection; policy staging and transmission are separate. */
export function previewSourceBytes(project: Project, ranges: readonly ByteRange[]): SourceBytePreview {
  if (!ranges.length) throw new Error("Select at least one byte range");
  const bytes: number[] = [];
  for (const range of ranges) {
    const file = project.files.find((item) => item.path === range.path);
    if (!file) throw new Error("File is outside the frozen snapshot");
    const encoded = new TextEncoder().encode(file.text);
    if (!Number.isSafeInteger(range.start) || !Number.isSafeInteger(range.end) || range.start < 0 || range.end <= range.start || range.end > encoded.length) throw new Error("Byte range is outside the frozen file");
    bytes.push(...encoded.slice(range.start, range.end));
  }
  return Object.freeze({ snapshot: project.snapshot, ranges: Object.freeze(ranges.map((range) => Object.freeze({ ...range }))), bytes: Object.freeze(bytes), encoding: "UTF-8" });
}
export function timecode(ms: number): string {
  return `${Math.floor(ms / 60000).toString().padStart(2, "0")}:${Math.floor(ms / 1000 % 60).toString().padStart(2, "0")}.${(ms % 1000).toString().padStart(3, "0")}`;
}
