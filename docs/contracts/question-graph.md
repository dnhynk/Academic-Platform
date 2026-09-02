# Question graph

## Scope and source-of-truth correction

This contract implements design specification section 14. The P2-N4 execution
plan says “seven-state lifecycle,” but section 14.2 names exactly these six
states: `OPEN`, `PARTIALLY_RESOLVED`, `RESOLVED`, `REFRAMED`, `OBSOLETE`, and
`REOPENED`. `new Question` is the object produced by reframing, while
`REFRAMED_AS` is its graph edge; neither is another status. The design
specification is authoritative, so the implementation does not invent a
seventh state. The source-backed lifecycle test compares the Rust enum against
the six names and fails if a variant is added or removed.

## Time-bearing question and exact origin

`Question` has a stable entity identity, resolution scope, canonical text,
`createdAt`, origin, related concept claims, importance provenance, append-only
wording revisions, and append-only lifecycle events. Each revision retains its
previous and replacement text with an instant. Each lifecycle event retains
the directed status edge, instant, and any evidence identities.

The origin vocabulary is Lecture, CourseMaterial, Assignment, PersonalStudy,
Repository, CodeReview, ProjectSpec, and ConceptDetail. Seven variants carry a
non-empty typed context locator. The Repository variant instead requires all
of `snapshot`, normalized repository-relative `path`, and a one-based `line`
as fields of the type. Deserialization repeats these validations; omission of
any repository coordinate, an absolute path, and line zero are exercised as
rejection injections.

## Lifecycle and authority

The exact directed lifecycle edges drawn in section 14.2 are:

| From | To |
|---|---|
| `OPEN` | `PARTIALLY_RESOLVED` |
| `OPEN` | `REFRAMED` |
| `OPEN` | `OBSOLETE` |
| `PARTIALLY_RESOLVED` | `RESOLVED` |
| `PARTIALLY_RESOLVED` | `REFRAMED` |
| `RESOLVED` | `REOPENED` |
| `RESOLVED` | `REFRAMED` |

The transition test enumerates every ordered pair of distinct statuses. It
compares each pair with the table above, injects every non-edge into the
definition and observes rejection, then removes each allowed edge in turn and
observes rejection. It also injects missing and duplicate status entries.

`Question::resolve` accepts a `VerifiedQuestionResolution`, rather than an
`Actor` or raw `Claim`. Both constructors for that authority token reuse
ADR-003's `Claim::validate_for_actor` matrix and then require the exact
question, scope, predicate, object, cited evidence, `USER_EXPLICIT` authority,
`USER_CONFIRMED` status, and `Actor::User`. The two admitted paths are a direct
user decision and a pre-declared validation that completed with its fixed
result followed by user approval. Tests inject forged user authority for every
automatic actor and each automatic actor's native matrix row before observing
the user path pass.

`OBSOLETE` carries a closed reason-code enum and at least one evidence identity.
The reason codes describe invalidated questions: false premise, technology
change, superseded context, or retracted source. Avoidance and deferral use the
separate `QuestionDeferral` type, leave status unchanged, and are rejected by
the compiler when supplied to `mark_obsolete`. JSON that changes only the
status to `OBSOLETE` is also rejected.

Reframing creates a fresh open `Question`, transitions the preserved original
to `REFRAMED`, and returns a typed `REFRAMED_AS` relation. The old identity and
text remain present in the result.

## Workspace and generated material

The default workspace order is fixed as:

1. origin text and capture context;
2. linked concepts and prerequisites;
3. relevant existing evidence;
4. recurrence locations in lectures, courses, and projects;
5. possible resolution sources;
6. AI explanation.

Region six accepts a `GeneratedExplanation` only with the explicit `Requested`
preference. The generated value names its artifact, question, model run, and
creation instant. A matching `EvidenceItem` can cite that artifact in a later
user resolution decision. Creating the generated artifact or an AI resolution
candidate leaves the question status unchanged; the acceptance test separately
shows that the same artifact participates in resolution only after a verified
user decision.

## Categorical growth and conservative reuse

Growth remains a set of separate categorical descriptors: target scope,
prerequisite depth, comparison quality, condition specificity, evidence use,
and reuse breadth. The schema contains no combined scalar. The acceptance test
scans the source-delimited descriptor schema, injects a `difficulty_score`
field into the scanned bytes, and observes the scan reject the mutation.

Design section 14.4 defines reuse as the number of other projects or concepts
to which one answer transfers. Therefore `ReuseSummary` deduplicates by the
typed destination identity: repeated transfer to the same project or concept
counts once, while a project and a concept remain different target kinds. A
transfer whose destination identity or kind cannot be established is excluded
and surfaces an `UncountedReuseReason`. This default-deny interpretation is
also consistent with section 16.2's `reuse_across_goals` breadth and section
14.4's rejection of false precision.

## Named acceptance evidence

`cargo test -p academic-domain --test question_graph` executes all required
evidence:

| Test | Evidence |
|---|---|
| `question_schema_round_trip` | camel-case schema, time, revision, and origin survive validated JSON round-trip |
| `repo_origin_requires_snapshot_path_line` | each missing repository coordinate, zero line, and absolute path fail |
| `lifecycle_transition_table_rejects_every_non_edge` | status and edge lists are exact; every non-edge addition and every allowed-edge removal fail |
| `ai_proposal_leaves_status_unchanged` | generating a resolution candidate does not mutate the question |
| `resolution_requires_user_decision` | automatic actors fail both forged and native matrix injections; direct decision and validation-plus-approval pass only for the user |
| `obsolete_requires_reason_and_evidence` | empty evidence and status-only JSON fail; typed evidenced obsolescence passes while deferral remains separate |
| `reframe_preserves_old_text_and_links` | old identity/text remain and a fresh open question is linked by `REFRAMED_AS` |
| `workspace_region_order_is_exact` | all six regions match the specified order |
| `ai_explanation_is_region_six_and_opt_in` | hidden injection fails, requested region-six artifact is visible, and resolution still requires the user token |
| `growth_descriptors_contain_no_scalar_score` | clean schema scan passes and injected scalar field fails |
| `reuse_count_deduplicates` | repeated typed destinations count once and unresolved destinations remain surfaced but uncounted |

All fixtures are synthetic and all operations are local and deterministic.
