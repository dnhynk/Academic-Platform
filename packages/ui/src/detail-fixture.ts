/** Deterministic, invented corpus. No personal recording, repository or school record. */
import type { Concept, DetailCorpus, Lecture, Project, Question, Relation } from "./details.js";

function relation(id: string, label: string, content: string, target?: Relation["target"]): Relation {
  return {
    id, label, status: id.includes("candidate") ? "PROPOSED" : id.includes("contradiction") ? "CONTESTED" : "CONFIRMED",
    confidence: "Synthetic calibrated example · 0.81 (fixture, not a personal assessment)",
    source: { id: `source-${id}`, title: `Synthetic evidence · ${label}`, locator: id.includes("code") ? "snapshot 1111111 · src/union.ts:2–5" : `fixture-v1 / ${id}`, content },
    ...(target ? { target } : {}),
  };
}
const conceptTarget = { routeId: "learn.concepts", id: "amortized-analysis" };
const questionTarget = { routeId: "learn.questions", id: "q-why-inverse-ackermann" };
const lectureTarget = { routeId: "learn.lectures", id: "synthetic-learn-lectures-1" };
const projectTarget = { routeId: "build.projects", id: "synthetic-graph-lab" };
const courseTarget = { routeId: "academic.courses", id: "4190.310" };

export function buildDetailFixture(): DetailCorpus {
  const lecture: Lecture = {
    id: lectureTarget.id, title: "Synthetic Lecture 05 · Amortized analysis",
    segments: [
      { id: "s1", startMs: 0, endMs: 2000, raw: "A sequence of operations shares the total cost.", corrected: "A sequence of operations shares the total cost.", disposition: null, reason: null },
      { id: "s2", startMs: 2000, endMs: 4000, raw: "Path compression shortens parent chains. The bound uses alpha n.", corrected: "Path compression shortens parent chains. The bound uses α(n).", disposition: null, reason: null },
      { id: "s3", startMs: 4000, endMs: 6000, raw: "An unresolved synthetic aside remains outside the document.", corrected: "An unresolved synthetic aside remains outside the document.", disposition: "UNMAPPED", reason: null },
      { id: "s4", startMs: 6000, endMs: 8000, raw: "", corrected: "", disposition: "REDACTED_WITH_POLICY", reason: "Synthetic policy redaction-1 · transcript retention exclusion" },
      { id: "s5", startMs: 8000, endMs: 10000, raw: "", corrected: "", disposition: "UNTRANSCRIBED_FAILURE", reason: "Synthetic capture journal frame 5 · missing audio frame" },
      { id: "s6", startMs: 10000, endMs: 12000, raw: "", corrected: "", disposition: "EXCLUDED_NON_SPEECH", reason: "Synthetic user decision non-speech-1 · silence interval" },
    ],
    paragraphs: [
      { id: "p1", text: "A sequence of operations shares the total cost.", segmentIds: ["s1"] },
      { id: "p2", text: "Path compression shortens parent chains. The bound uses α(n).", segmentIds: ["s2"] },
    ],
    captures: [{ id: "capture-1", title: "Board capture · parent chains", text: "0 ← 1 ← 2 becomes 0 ← {1, 2}", atMs: 2400, segmentId: "s2" }],
    review: [
      { id: "mark-1", kind: "Mark Moment", segmentId: "s2", note: "Revisit the α(n) argument." },
      { id: "confidence-1", kind: "Low confidence", segmentId: "s3", note: "Unmapped aside needs review; never silently omit it." },
      { id: "equation-1", kind: "Equation", segmentId: "s2", note: "Compare raw alpha n with corrected α(n)." },
      { id: "code-1", kind: "Code", segmentId: "s2", note: "Check parent-pointer update against capture-1." },
    ],
    links: {
      "Concept candidates and approval": [relation("lecture-concept-candidate", "Amortized analysis · awaiting approval", "Raw segment s1 at 00:00.000 supports the candidate.", conceptTarget)],
      "Questions at this moment": [relation("lecture-question", "Why α(n)? · 00:02.000", "Question originated at raw segment s2.", questionTarget)],
      "Prerequisite gaps": [relation("lecture-gap", "Tree invariants · insufficient evidence", "Gap fixture: no confirmed fresh explanation evidence for tree invariants.")],
      "Next lecture preparation": [relation("lecture-preparation", "Trace three parent updates before Lecture 06", "Prerequisite closure: tree invariant → path compression; next context Lecture 06.")],
      "Assessments": [relation("lecture-assessment", "Synthetic Assignment 2 · parent trace", "Assessment prompt asks for a trace and explanation; no grade implies mastery.")],
    },
    explanation: "Synthetic AI explanation: compare the cost of the whole sequence with the cost of one operation. This explanation is not resolution evidence.",
  };
  const concepts: Concept[] = ["amortized-analysis", "union-find"].map((id) => ({
    id, title: id === "union-find" ? "Disjoint set union" : "Amortized analysis", state: "Practiced", confidence: "0.81 · synthetic calibrated fixture",
    freshness: "Moderate", lastStrongEvidence: "2026-09-01T09:00:00Z",
    relations: {
      "Evidence timeline": [relation(`${id}-lecture`, "Lecture 05 → Assignment 2 → Project experiment", "2026-08-25: lecture exposure; 2026-08-28: completed synthetic trace; 2026-09-01: independently authored experiment.", lectureTarget)],
      "Contradictions": [relation(`${id}-contradiction`, "Trace disagrees with claimed constant worst-case cost", "Experiment step 3 walks two parent links; claim remains contested beside its evidence.", projectTarget)],
      "Prerequisites": [relation(`${id}-prerequisite`, "Tree invariants", "Synthetic prerequisite relation: preserve the root and parent reachability.")],
      "Used in": [relation(`${id}-used-in`, "Disjoint set union", "Reachable find() operation uses path compression.", { routeId: "learn.concepts", id: "union-find" })],
      "Open questions": [relation(`${id}-question`, "Why does the bound involve α(n)?", "Question is OPEN; an AI explanation alone cannot resolve it.", questionTarget)],
      "SNU courses, lectures and assessments": [relation(`${id}-course`, "Synthetic Algorithms · Course / Lecture 05 / Assignment 2", "Synthetic course 4190.310; actual fixture lecture at s2 and assessment trace, not a predicted offering.", courseTarget)],
      "Projects": [relation(`${id}-code`, "Graph lab · exact code and experiment", "src/union.ts:2–5 at snapshot 1111111; experiment trace: find(2) follows parent[2] then parent[1].", projectTarget)],
      "Competencies": [relation(`${id}-competency-candidate`, "Complexity diagnosis · evidence candidate", "Candidate based on the independent trace; no repository-wide personal skill claim.", projectTarget)],
      "Roles": [relation(`${id}-role`, "Backend · Infrastructure", "Role relevance links to complexity diagnosis; following a role does not declare a career choice.")],
    },
    explanation: "Synthetic AI explanation: the evidence suggests practice, with a remaining worst-case misconception. Inspect the source before deciding.",
  }));
  const question: Question = {
    id: questionTarget.id, origin: "Lecture", isNew: false, status: "OPEN", goalRelevance: "Active graph-lab goal: explain find() cost", goalRank: 3,
    nextContext: "Lecture 06 · amortized proof, then graph-lab review", ageDays: 12,
    revisions: [
      { at: "2026-08-27T09:00:00Z", text: "Why is find nearly constant?", concepts: [relation("question-concept-v1", "Disjoint set union", "Initial linked concept at creation.", { routeId: "learn.concepts", id: "union-find" })], resolutionEvidence: [] },
      { at: "2026-09-01T09:00:00Z", text: "Why does the bound involve α(n)?", concepts: [relation("question-concept-v2", "Amortized analysis", "Append-only question revision adds a more precise concept.", conceptTarget)], resolutionEvidence: [relation("question-resolution-candidate", "Trace experiment · partial evidence, unresolved", "A trace demonstrates behavior but does not prove the asymptotic bound.", projectTarget)] },
    ],
    evidence: [relation("question-origin", "Lecture origin · s2 at 00:02.000", "Raw: The bound uses alpha n. Corrected: The bound uses α(n).", lectureTarget)],
    explanation: "Synthetic AI explanation: α is the inverse Ackermann function. This answer cannot mark the question resolved.",
  };
  const latest = question.revisions.at(-1);
  if (!latest) throw new Error("Synthetic question requires a revision");
  const questions: Question[] = [question, ...(["Lecture", "Repository", "Concept detail"] as const).map((origin, i) => ({ ...question, id: `inbox-${String(i)}`, origin, isNew: true, ageDays: i + 1, goalRank: i, goalRelevance: i === 0 ? "No active goal linkage yet" : "Upcoming graph-lab review" })), ...(["PARTIAL", "RESOLVED", "REFRAMED"] as const).map((status, i) => ({ ...question, id: `question-${status.toLowerCase()}`, status, ageDays: i + 2, goalRank: i, revisions: [{ ...latest, text: `${status}: synthetic parent-trace question`, resolutionEvidence: [relation(`question-${status}-evidence`, `${status} lifecycle evidence`, status === "RESOLVED" ? "Synthetic user confirmation resolves this question using the completed parent trace." : status === "REFRAMED" ? "Original question retained; reframed as q-why-inverse-ackermann." : "Trace answers one sub-question; proof remains open.", questionTarget)] }] }))];
  const project: Project = {
    id: projectTarget.id, title: "Synthetic graph lab", goal: "Explain and validate parent-chain compression", successCriteria: ["Preserve connectivity after each union", "Provide an independent trace and complexity explanation"],
    repository: "synthetic/graph-lab", branch: "exercise", snapshot: "1111111", currentSnapshot: "2222222", capturedAt: "2026-09-01T09:00:00Z", currentAt: "2026-09-08T09:00:00Z", dirty: true,
    relations: {
      "Architecture map": [relation("project-architecture", "CLI → union module → parent array", "Directed component dependencies: CLI calls union module; union module owns parent array.")],
      "ADR / spec / code drift": [relation("project-drift", "INTENDED_NOT_IMPLEMENTED · rollback", "Approved synthetic ADR asks for rollback; snapshot 1111111 contains no rollback operation. Both evidence lanes remain visible.")],
      "OBSERVED": [relation("project-observed-code", "Path compression · src/union.ts:2–5", "Observed reachable find call with parent-pointer writes in this component only.", conceptTarget)],
      "REQUIRED": [relation("project-required", "Tree invariant reasoning", "Current goal → preserve connectivity → parent update invariant → tree reasoning → insufficient explanation evidence.")],
      "WOULD_BENEFIT_FROM": [relation("project-benefit", "Rollback log if interactive history is added", "Trigger: interactive history requested (not active). Benefit: reversible union. Trade-off: additional memory and complexity.")],
      "Open questions and issues / incidents": [relation("project-question", "Why α(n)? · issue SYN-4", "SYN-4 asks for a proof; no incident is inferred from the open question.", questionTarget)],
      "Active Critical Path": [relation("project-path", "Tree invariants → trace → complexity explanation", "Active goal scope: synthetic graph-lab goal version 1.")],
      "Build → Learn branch": [relation("project-build-learn", "Declare trace result, then run the experiment", "Pre-declared validation expects root identity and connectivity to survive compression.")],
      "SNU Course / Offering and external options": [relation("project-course", "Synthetic Algorithms · confirmed fixture offering", "Course 4190.310; synthetic autumn offering; compare external self-study parent trace as an alternative.", courseTarget)],
      "Competency evidence and authorship": [relation("project-authorship-candidate", "Independent trace · authorship confirmed in fixture", "Trace authored by synthetic user; generated scaffold is excluded from personal competency promotion.")],
      "Snapshot semantic diff": [relation("project-diff", "1111111 → 2222222 · CODE_CHANGED", "Same analyzer build synthetic-analyzer-v1: newer snapshot adds a rollback log. Dependency inventory unchanged; semantic channel changed.")],
      "Stack inventory (supporting information)": [relation("project-stack", "TypeScript manifest · PRESENT_ONLY", "Manifest presence does not establish observed use or personal competence.")],
    },
    files: [{ path: "src/union.ts", text: "// 합성 fixture\nexport function find(parent: number[], x: number): number {\n  if (parent[x] !== x) parent[x] = find(parent, parent[x]);\n  return parent[x];\n}\n" }],
    explanation: "Synthetic AI explanation: compare the required invariant with the exact code and trace. Recommendations remain scoped to this goal and snapshot.",
  };
  return { lectures: [lecture], concepts, questions, projects: [project] };
}
