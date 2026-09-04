import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import test from "node:test";

import Ajv2020 from "ajv/dist/2020.js";

/**
 * Output bound for every `cargo` invocation below.
 *
 * `spawnSync` defaults to one mebibyte and silently reports `status: null`
 * with an empty stderr when a child exceeds it. `cargo metadata` over this
 * workspace is already past that, so the default turns "the graph grew by one
 * edge" into an unexplained policy failure. The bound is explicit and far
 * above any plausible workspace rather than absent.
 */
const CARGO_OUTPUT_BYTES = 64 * 1024 * 1024;

const metadataRun = spawnSync(
  "cargo",
  ["metadata", "--locked", "--offline", "--format-version", "1"],
  { encoding: "utf8", maxBuffer: CARGO_OUTPUT_BYTES },
);
assert.equal(
  metadataRun.status,
  0,
  `locked offline cargo metadata failed: ${metadataRun.stderr}`,
);
const metadata = JSON.parse(metadataRun.stdout);
const spikeMetadataRun = spawnSync(
  "cargo",
  [
    "metadata",
    "--locked",
    "--offline",
    "--format-version",
    "1",
    "--no-default-features",
    "--features",
    "academic-store/sqlcipher-spike",
  ],
  { encoding: "utf8", maxBuffer: CARGO_OUTPUT_BYTES },
);
assert.equal(
  spikeMetadataRun.status,
  0,
  `locked offline SQLCipher-spike metadata failed: ${spikeMetadataRun.stderr}`,
);
const spikeMetadata = JSON.parse(spikeMetadataRun.stdout);
const osKeystoreMetadataRun = spawnSync(
  "cargo",
  [
    "metadata",
    "--locked",
    "--offline",
    "--format-version",
    "1",
    "--features",
    "academic-crypto/os-keystore",
  ],
  { encoding: "utf8", maxBuffer: CARGO_OUTPUT_BYTES },
);
assert.equal(
  osKeystoreMetadataRun.status,
  0,
  `locked offline os-keystore metadata failed: ${osKeystoreMetadataRun.stderr}`,
);
const osKeystoreMetadata = JSON.parse(osKeystoreMetadataRun.stdout);
const osKeystorePackagesByName = new Map(
  osKeystoreMetadata.packages.map((pkg) => [pkg.name, pkg]),
);
const osKeystoreResolveNodesById = new Map(
  osKeystoreMetadata.resolve.nodes.map((node) => [node.id, node]),
);
const packagesById = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
const packagesByName = new Map(metadata.packages.map((pkg) => [pkg.name, pkg]));
const resolveNodesById = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
const spikePackagesByName = new Map(spikeMetadata.packages.map((pkg) => [pkg.name, pkg]));
const spikeResolveNodesById = new Map(
  spikeMetadata.resolve.nodes.map((node) => [node.id, node]),
);
const workspaceIds = new Set(metadata.workspace_members);
const workspacePackages = metadata.workspace_members.map((id) => packagesById.get(id));
const workspaceNames = new Set(workspacePackages.map((pkg) => pkg.name));

function cargoLockPackageTuples(cargoLock) {
  const field = (body, name) =>
    body.match(new RegExp(`^${name} = "(?<value>[^"]+)"$`, "mu"))?.groups?.value ?? null;
  return [...cargoLock.matchAll(/\[\[package\]\]\r?\n(?<body>.*?)(?=\r?\n\[\[package\]\]|\s*$)/gsu)]
    .map(({ groups }) => [
      field(groups.body, "name"),
      field(groups.body, "version"),
      field(groups.body, "source"),
      field(groups.body, "checksum"),
    ])
    .toSorted((left, right) => {
      const leftText = JSON.stringify(left);
      const rightText = JSON.stringify(right);
      return leftText < rightText ? -1 : leftText > rightText ? 1 : 0;
    });
}

/**
 * Names the workspace crates `pkg` depends on through edges of the given kinds.
 *
 * `kind` is `null` for a normal dependency, `"dev"` for a test-only one, and
 * `"build"` for a build script's. The distinction is load-bearing: a normal
 * edge ships inside the product, a dev edge exists only while a test target is
 * being compiled, and the two must never be asserted against one frozen list —
 * doing that would let a new product edge hide behind an expected dev one.
 */
function workspaceDependencyNamesOfKinds(pkg, kinds) {
  return pkg.dependencies
    .filter(
      (dependency) =>
        dependency.source === null &&
        workspaceNames.has(dependency.name) &&
        kinds.includes(dependency.kind ?? null),
    )
    .map((dependency) => dependency.name)
    .toSorted();
}

/** Workspace crates that ship inside `pkg`. */
function productDependencyNames(pkg) {
  return workspaceDependencyNamesOfKinds(pkg, [null, "build"]);
}

/** Workspace crates `pkg` links only while building a test target. */
function devDependencyNames(pkg) {
  return workspaceDependencyNamesOfKinds(pkg, ["dev"]);
}

/** Every workspace edge of `pkg`, of any kind. */
function workspaceDependencyNames(pkg) {
  return workspaceDependencyNamesOfKinds(pkg, [null, "build", "dev"]);
}

/**
 * Returns the packages that reach the fault harness through a shipping edge.
 *
 * The harness may be reached by a dev edge — that is what a dev edge is for —
 * but never by an edge that ends up in a product build.
 */
function packagesWithProductEdgeTo(packages, target) {
  return packages
    .filter((pkg) => pkg.name !== target && productDependencyNames(pkg).includes(target))
    .map((pkg) => pkg.name)
    .toSorted();
}

/**
 * Returns the fault-injection feature each package forwards, keyed by name.
 *
 * A package with no such feature is absent from the result, so a package that
 * silently gains one shows up as an unexpected key rather than as nothing.
 */
function faultFeatureForwarding(packages) {
  return Object.fromEntries(
    packages
      .filter((pkg) => FAULT_FEATURE in pkg.features)
      .map((pkg) => [pkg.name, pkg.features[FAULT_FEATURE].toSorted()])
      .toSorted(([left], [right]) => left.localeCompare(right)),
  );
}

/** Packages whose resolved feature set enables fault injection. */
function packagesResolvingFaultFeature(packages, resolveNodes) {
  return packages
    .filter((pkg) => (resolveNodes.get(pkg.id)?.features ?? []).includes(FAULT_FEATURE))
    .map((pkg) => pkg.name)
    .toSorted();
}

const FAULT_FEATURE = "phase1-fault-injection";

function assertAcyclic(graph) {
  const permanent = new Set();
  const temporary = new Set();
  const visit = (name, path) => {
    if (permanent.has(name)) {
      return;
    }
    assert.equal(
      temporary.has(name),
      false,
      `workspace dependency cycle: ${[...path, name].join(" -> ")}`,
    );
    temporary.add(name);
    for (const dependency of graph.get(name) ?? []) {
      visit(dependency, [...path, name]);
    }
    temporary.delete(name);
    permanent.add(name);
  };
  for (const name of graph.keys()) {
    visit(name, []);
  }
}

test("workspace_dependency_direction_is_acyclic", () => {
  // Only the shipping graph is frozen here. A dev edge does not travel into a
  // product build and Cargo permits it to point back the way a normal edge may
  // not, so mixing the two into one expectation would either forbid a legal
  // test edge or let a new product edge pass as an expected dev one.
  const actual = Object.fromEntries(
    workspacePackages
      .map((pkg) => [pkg.name, productDependencyNames(pkg)])
      .toSorted(([left], [right]) => left.localeCompare(right)),
  );
  assert.deepEqual(actual, {
    "academic-admission": [],
    "academic-capture-client": ["academic-policy"],
    // `P2-L1`'s device gate. Its product edges are the consent decision it
    // re-runs and the domain identifiers its audit rows carry; `libc` and
    // `windows-sys` are optional target-specific edges behind the non-default
    // `native-capture` feature and are not workspace packages, so they appear
    // in `SOCKET_CAPABLE_CLOSURES` below rather than here.
    //
    // Nothing depends on it, for `academic-worker`'s reason: it carries a
    // platform backend and a probe binary, and a product crate that linked it
    // would put both in a default build's dependency graph. The capture client
    // process crate is unchanged and still holds exactly its one process-class
    // binding.
    // `P2-L2`. The capture subsystem's two product edges: the crate that owns
    // the one section 3.7 binding, and the crate that owns the identifier and
    // digest types its journal frames carry. It deliberately has **no** edge to
    // `academic-capture-gate` — the rule in `only_egress_crate_has_a_socket`
    // forbids one — so the two crates are siblings over one binding rather than
    // a stack.
    "academic-capture": ["academic-consent", "academic-domain"],
    "academic-capture-gate": ["academic-consent", "academic-domain"],
    // `P2-U3`. The graduation audit. Four product edges, each a boundary it
    // reuses rather than rebuilds: `academic-domain` for the §3.9 proof-tree
    // vocabulary and the `AuditId` migration 0004's `audit` arm keys on,
    // `academic-requirement` for `P2-U2`'s published rule set and per-rule
    // verdict, `academic-record` for `P2-U4`'s attempt ledger, classification
    // and grade-point reading, and `academic-ingestion` for `P2-U6`'s
    // `ConflictCase` disposition. The edges it does *not* have are the point:
    // no `academic-store`, so an audit cannot write itself; no model crate, so
    // a graduation verdict cannot reach an interpreted sentence; and no
    // `academic-scenario`, so `P2-C7`'s projected values are not nameable from
    // a product file here.
    "academic-audit": [
      "academic-domain",
      "academic-ingestion",
      "academic-record",
      "academic-requirement",
    ],
    "academic-cli": [
      "academic-admission",
      "academic-core",
      "academic-daemon",
      "academic-rpc",
    ],
    // `P2-G6`'s consent ledger and section 3.7 capture-permission model. The
    // domain crate is its only product edge, and that is a constraint rather
    // than a preference: it restates `academic-retention`'s derivative-class
    // list instead of importing it, because `rotation_engine_lane_is_not_default`
    // below holds that exactly one crate declares that edge. The restatement is
    // compared against the original through a dev edge, in
    // `the_two_derivative_vocabularies_are_the_same_list`.
    "academic-consent": ["academic-domain"],
    "academic-contracts": ["academic-domain"],
    // `P2-Y1`'s section 24.1 competency. Three product edges and each one is a
    // boundary it reads a fact out of rather than a vocabulary it rebuilds:
    // `academic-domain` for section 7.1's node hierarchy, section 7.2's
    // `ENABLES_COMPETENCY` descriptor and `P2-N1`'s entity identity;
    // `academic-knowledge-state` for section 13.2's own ceiling, which is what
    // refuses a dependency-presence item rather than a rule written here; and
    // `academic-repository-competency` for `P2-R5`'s `User APPLIED Concept`,
    // which is one of the two doors a rubric cell may be founded on. The edges
    // it does not have are the point: no `academic-policy`, because it mints no
    // identity of its own; no `academic-store` -- it persists nothing and adds
    // no migration -- and no `academic-worker` and no
    // `academic-egress-boundary`, so nothing in its closure can launch a
    // process or stage a payload.
    "academic-competency": [
      "academic-domain",
      "academic-knowledge-state",
      "academic-repository-competency",
    ],
    "academic-connector": ["academic-policy"],
    // `P2-X5`'s CS map. **One** product edge, and the ones it does not have are
    // the task. `academic-domain` supplies every vocabulary the atlas draws
    // with rather than a second one: section 7.1's `NodeType`, section 7.2's
    // twenty `PredicateName`s, `MasteryLevel`, `FreshnessBand`,
    // `EpistemicStatus`, `ConfidencePermille` and `P2-C6`'s
    // `temporal::{ChangeOrigin, TimeCoordinates}`. The edges it does *not* have
    // are what keeps the picture from saying more than the record: no
    // `academic-knowledge-state` and no `academic-freshness`, so no mastery
    // ladder and no decay is in the closure and a fill is a display rather than
    // a computation; no `academic-critical-path` and no `academic-blind-spot`,
    // so the halo and the gap glyph are read off a caller-supplied reading
    // rather than recomputed -- drawing a path is not deciding one; no
    // `academic-store`, `academic-vault` or `academic-projections`, so nothing
    // here reaches the canonical writer or a snapshot sidecar and it claims no
    // migration; and no `academic-worker` and no `academic-egress-boundary`, so
    // nothing in its closure can launch a process or stage a payload.
    "academic-cs-map": ["academic-domain"],
    "academic-crypto": ["academic-keystore-platform"],
    // `P2-U1`. Section 8.2's aggregates and section 11.4's four relations.
    // `academic-domain` supplies the v3 aggregate identifiers migration 0004's
    // closure rows key on; `academic-ingestion` supplies `PublishedRules`, which
    // is the only argument `CurriculumPublication::from_official_source` takes,
    // so a curriculum version founded on an undated official source is not a
    // value that exists. The edge it does *not* have is the point: no
    // `academic-store`, so the canonical writer is not in the closure a
    // curriculum aggregate compiles against and this crate cannot write the
    // typed rows migration 0014 creates.
    "academic-curriculum": ["academic-domain", "academic-ingestion"],
    "academic-core": [
      "academic-contracts",
      "academic-domain",
      "academic-ledger",
      "academic-portability",
      "academic-projections",
      "academic-rpc",
      "academic-store",
      "academic-vault",
    ],
    "academic-daemon": [
      "academic-admission",
      "academic-core",
      "academic-rpc",
      "academic-store",
    ],
    // `P2-X1`. The desktop surface's one workspace edge is the local-core
    // contract. `desktop_cannot_open_the_database_or_read_keys` is what says
    // this row cannot grow a store, vault or key crate.
    // `P2-P2`'s deletion and retention product flow. Six product edges, each a
    // boundary it drives rather than restates: `academic-retention` for `P2-K5`'s
    // seven derivative classes, four-word vocabulary, append-only journal,
    // backup tombstone and its two seams -- this crate is the task that supplies
    // the real implementations behind them; `academic-student-voice` for
    // `P2-L5`'s section 32.5 projection walk, called rather than forked;
    // `academic-evidence-center` for `P2-X7`'s deletion receipt reference and
    // for the correction outcome a leak incident must *not* be closed by;
    // `academic-proposal` for `P2-M2`'s `UserDecision`, so a non-delegable
    // confirmation reuses that door instead of writing a second actor check;
    // and `academic-consent` and `academic-domain` for the retention terms and
    // the identifiers. `academic-vault` is optional and only the non-default
    // `deletion-engine` lane selects it, so the default graph here resolves the
    // plan and the vocabulary and **not** the object namespace that can destroy
    // a key slot -- `deletion_lane_is_not_default` proves that in both
    // directions. The edges it does *not* have are the point: no
    // `academic-store` and no `academic-store-platform`, so it persists nothing
    // and claims no migration; and no `academic-policy` and no
    // `academic-egress-boundary`, so no product file here can name a broker, a
    // capability or a staged payload -- the provider deletion receipt it links
    // is `P2-G3`'s persisted row, compared through a dev edge.
    "academic-deletion": [
      "academic-consent",
      "academic-domain",
      "academic-evidence-center",
      "academic-proposal",
      "academic-retention",
      "academic-student-voice",
      "academic-vault",
    ],
    "academic-desktop": ["academic-rpc"],
    "academic-domain": [],
    "academic-egress": ["academic-policy"],
    // `P2-G2`'s DLP rulepack, minimizer, byte-accurate preview, and the sole
    // outbound transport seam. It is a separate package from `P2-G7`'s
    // egress-proxy process entry point, whose whole manifest and whole product
    // source that task pins as one fixed process-class binding; a library
    // target inside it would have made that pin weaker rather than exact.
    // Nothing depends on this one: the section 3.6 wiring from the core is
    // `P2-G4`'s and `P2-A2`'s round, not this task's.
    "academic-egress-boundary": ["academic-policy"],
    // `P2-X7`. Section 25.13's evidence and correction centre. Three product
    // edges, each a boundary it reuses rather than rebuilds: `academic-domain`
    // for every identifier a centre entry names and for the `P2-C6` bitemporal
    // coordinate a historical view is read at, `academic-proposal` for `P2-M2`'s
    // risk tier and its user-only receipt, and `academic-ingestion` for `P2-U6`'s
    // source diff and dependency graph. The edges it does *not* have are the
    // point: no `academic-store` and no `academic-vault`, so the canonical
    // writer is absent and this crate claims no migration number; no
    // `academic-policy` and no `academic-egress-boundary` as *declared* edges,
    // though both are reachable transitively through
    // `academic-untrusted-content` -- `the_center_cannot_name_a_payload_byte`
    // says so in its own words and closes the gap with a whole-set path-root
    // allowlist rather than pretending the closure does it.
    "academic-evidence-center": [
      "academic-domain",
      "academic-ingestion",
      "academic-proposal",
    ],
    "academic-export-job": ["academic-policy"],
    "academic-indexer": ["academic-policy"],
    // `P2-P3`. Six product edges, each a boundary it reuses rather than
    // rebuilds: `academic-domain` for the UUIDv7 identifiers an
    // `ExternalIdentity` maps *onto*, `academic-policy` for the second grant a
    // private blob requires, `academic-egress-boundary` for the staging and
    // preview every assistant handoff runs through, `academic-repository` for
    // `P2-R1`'s repo-scoped read-only token, `academic-untrusted-content` for the
    // webhook body a remote server chose, and `academic-model-run` for the run
    // digest generated code records.
    //
    // It has **no** edge to `academic-competency` or
    // `academic-repository-competency`, which is what
    // `assistant_use_is_not_competency` reads rather than asserts, and no edge
    // to `academic-ledger`, so the core read path does not run through this
    // boundary at all.
    "academic-integrations": [
      "academic-domain",
      "academic-egress-boundary",
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-untrusted-content",
    ],
    // `P2-U6`. Section 29.1's ingestion contract. `academic-domain` supplies
    // the content digest and the proof-tree rule identifier an invalidated
    // requirement already cites; `academic-untrusted-content` supplies
    // `Untrusted<T>`, which is the only public route out of a raw snapshot's
    // bytes. There is deliberately no HTTP, TLS, browser, image or audio edge:
    // the conditional fetch is a trait the caller implements.
    "academic-ingestion": ["academic-domain", "academic-untrusted-content"],
    "academic-ledger": ["academic-contracts", "academic-domain"],
    // `P2-M1`'s edge to `academic-policy` is the reconciliation: the audit rows
    // it compares a recorded transmission against are the broker's, and the
    // namespace discriminator it keys on is that crate's column.
    "academic-model-run": ["academic-domain", "academic-policy"],
    // `P2-L6`. Section 12.7's next-lecture preparation. Five product edges, each
    // a boundary it reads a fact out of rather than a vocabulary it restates:
    // `academic-untrusted-content` for the `P2-G5` `Proposal` every extracted
    // claim is built from, `academic-gap` for the `P2-N5` descent the comparison
    // against the prerequisite graph *is*, `academic-lecture-document` for the
    // `P2-L4` node section 12.7's seventh place cites, `academic-ingestion` for
    // `P2-U6`'s validated calendar date section 27.1 requires beside each
    // material, and `academic-domain` for the `AI_INFERRED` standing and the
    // tier that carries no prerequisite of its own.
    //
    // It has **no** edge to `academic-knowledge-state`, which is what
    // `an_extracted_claim_is_never_confirmed` reads out of the public signatures
    // rather than asserts: no function here can return the evidence a mastery
    // promotion is read from. No `academic-store`, so no preparation reaches the
    // canonical writer and no migration is added; no `academic-worker` and no
    // `academic-egress-boundary` as a declared edge, so nothing here can launch
    // a process or stage a payload.
    "academic-next-lecture": [
      "academic-domain",
      "academic-gap",
      "academic-ingestion",
      "academic-lecture-document",
      "academic-untrusted-content",
    ],
    // `P2-M4`. The non-delegable action set and the command layer that refuses an
    // automatic actor for it. Two product edges, and the smallness is the
    // contract: `academic-domain` for the closed `Actor` enum, the subject
    // digest and the instant, and `academic-proposal` for `P2-M2`'s
    // `UserDecision` and its four section 27.4 tiers -- so the actor check and
    // the tier vocabulary are that crate's doors reused rather than a second
    // copy, which is what makes this task's compiled constant and `P2-P2`'s
    // deletion confirmation one fact. The edges it does *not* have are the
    // point: no `academic-store` and no `academic-store-platform`, so it
    // persists nothing and claims no migration; and none of the six crates that
    // own the six actions, so no product file here can name a verdict, a
    // mastery level, a broker or a staged payload. Those six are dev edges, and
    // the acceptance suite drives each of them for real.
    "academic-non-delegable": ["academic-domain", "academic-proposal"],
    // `P2-U5`. Section 8.3's four offering statuses and the calibrated forecast
    // behind them. Five product edges, each a boundary it reuses rather than
    // rebuilds: `academic-curriculum` for `P2-U1`'s `OfferingStatus`, which is
    // the four-value vocabulary and migration 0014's `CHECK`;
    // `academic-domain` for the proof-tree vocabulary and the
    // `PredictionMetadata` §2.3-15 pins at version 1; `academic-ingestion` for
    // §8.4's six source levels; `academic-model-run` for `P2-M1`'s calibration
    // registry, which is the only producer of a displayable confidence, so a
    // forecast with no fresh dataset abstains rather than showing an
    // uninterpreted number; and `academic-record` for `P2-U4`'s ordered
    // `TermKey` and its `PlanScenario`. The edges it does *not* have are the
    // point: no `academic-store`, so a forecast cannot write itself, and no
    // `academic-audit`, so a prediction is not nameable from the graduation
    // verdict's crate and the verdict is not nameable from here.
    "academic-offering": [
      "academic-curriculum",
      "academic-domain",
      "academic-ingestion",
      "academic-model-run",
      "academic-record",
    ],
    // `academic-crypto`, `academic-recovery`, and `academic-projections` are
    // all optional edges here, and the two lane features that select them are
    // mutually exclusive: `plaintext-portability` (default) selects the
    // projection engine and the plaintext store lane, `encrypted-portability`
    // selects the key schedule, the `P2-K4` recovery contract, and the
    // SQLCipher store lane. `cargo metadata` reports declared dependencies
    // rather than resolved ones, so all three are listed.
    "academic-portability": [
      "academic-admission",
      "academic-contracts",
      "academic-crypto",
      "academic-domain",
      "academic-projections",
      "academic-recovery",
      "academic-retention",
      "academic-store",
      "academic-vault",
    ],
    "academic-policy": [],
    "academic-projections": ["academic-domain", "academic-store"],
    // `P2-M2`'s proposal boundary, risk tiers and review queue. Its one product
    // edge is the domain crate, and the edges it does *not* have are the point:
    // no `academic-store`, so the canonical writer is not in the closure a
    // `Proposed<T>` compiles against and
    // `proposed_type_cannot_reach_canonical_writer` is a compile error rather
    // than a source scan; and no `academic-policy`, so no capability-bearing
    // value is nameable from a product file here.
    "academic-proposal": ["academic-domain"],
    // `P2-Y3`'s section 24.3 readiness view. Four product edges and each one is
    // a boundary it reads a fact out of rather than a vocabulary it rebuilds:
    // `academic-competency` for `P2-Y1`'s competency, criterion and stage
    // values -- a matrix row names the first, a walk ends at the second, and a
    // placement carries the third read off the record rather than asserted;
    // `academic-role-profile` for `P2-Y2`'s lineage-and-version pair, which is
    // what a matrix is *of*, and its three importance words; `academic-domain`
    // for section 13.3's `FreshnessBand`, so the sixth column carries `P2-N3`'s
    // enumeration and declares no second one; and `academic-knowledge-state`
    // for section 13.1's own sentence that no evidence is not a failed
    // examination. The edges it does *not* have are the point: no
    // `academic-store` -- it persists nothing and adds no migration -- no
    // `academic-worker` and no `academic-egress-boundary`, so nothing in its
    // closure can launch a process or stage a payload, and **no
    // `academic-export`**, because a view that linked the exporter could carry
    // a notice the exporter minted. The notice travels inside the document
    // instead, and `academic-export` is a dev edge so the export path is
    // measured rather than assumed.
    "academic-readiness": [
      "academic-competency",
      "academic-domain",
      "academic-knowledge-state",
      "academic-role-profile",
    ],
    "academic-record": ["academic-domain", "academic-transcript"],
    "academic-requirement": ["academic-domain", "academic-ingestion"],
    // `P2-R1`'s repository snapshot boundary. Three product edges, each a
    // boundary it reuses rather than rebuilds: `P2-K1`'s `DeviceKeystore` seam
    // for the GitHub credential, `P2-G1`/`P2-G7`'s broker and process matrix
    // for the permission half of the gate, and `P2-G5`'s trust boundary for the
    // repository bytes themselves. The edges it does *not* have are the point:
    // no `academic-store`, so a snapshot cannot reach the canonical writer; no
    // `academic-egress-boundary` and no `academic-worker`, so nothing in its
    // closure can stage a payload or launch a process.
    "academic-repository": [
      "academic-crypto",
      "academic-policy",
      "academic-untrusted-content",
    ],
    // `P2-R2`'s static analysis and evidence ladder. Four product edges and
    // each is a boundary it reuses rather than rebuilds: `P2-R1`'s frozen
    // snapshot, which its input type takes by reference and which is what says
    // an analysis cannot exist without a completed gate; `P2-G5`'s sealed
    // index, which every analyzed unit's digest has to appear in; `P2-M1`'s
    // calibration registry, which is the only producer of a displayable
    // confidence; and `academic-policy` for `ContentDigest`. The edges it does
    // not have are the point: no `academic-store`, so a finding cannot reach
    // the canonical writer; no `academic-worker` and no
    // `academic-egress-boundary`, so nothing in its closure can launch a
    // process or stage a payload; and no `std::fs` at all, which
    // `the_analysis_crate_touches_no_file_and_no_socket` holds as a whole-set
    // comparison of its `use` items.
    "academic-repository-analysis": [
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-untrusted-content",
    ],
    // `P2-R3`'s cross-artifact correlation and drift lanes. Five product edges
    // and each is a boundary it reuses rather than rebuilds: `P2-R1`'s frozen
    // snapshot, which every document path and every incident is checked
    // against; `P2-R2`'s findings, which are the only route from repository
    // bytes to an implementation-lane relation; `academic-ledger`, which
    // already holds section 30.3's six authority rows and their rank tables, so
    // this crate adds the two rows' qualifiers rather than a second resolver;
    // and `academic-domain` for the authority vocabulary those tables are
    // indexed by. The edges it does not have are the point: no `academic-store`
    // -- it persists nothing and adds no migration -- and no `academic-worker`
    // and no `academic-egress-boundary`, so nothing in its closure can launch a
    // process or stage a payload.
    "academic-repository-correlation": [
      "academic-domain",
      "academic-ledger",
      "academic-repository",
      "academic-repository-analysis",
    ],
    // `P2-R4`'s section 18 classification. Four product edges and each is a
    // boundary it reuses rather than rebuilds: `P2-R2`'s findings and locators,
    // which are the only route from repository bytes to a proof chain's first
    // step; `P2-R3`'s correlation, which is the only route to an `OBSERVED`;
    // `academic-domain` for section 7.4's ontology tiers, which is what makes a
    // whole field unrequirable; and `academic-policy` for the digest a
    // requirement identity is, because a joined and truncated identity collides.
    // The edges it does not have are the point: no `academic-store` -- it
    // persists nothing and adds no migration -- and no `academic-worker` and no
    // `academic-egress-boundary`, so nothing in its closure can launch a process
    // or stage a payload.
    "academic-repository-classification": [
      "academic-domain",
      "academic-policy",
      "academic-repository-analysis",
      "academic-repository-correlation",
    ],
    // `P2-R5`'s section 17.6 promotion. Five product edges, one more than
    // `P2-R4` and each a boundary it reuses rather than rebuilds: `P2-R4`'s
    // classification, which is the only route to a project observation claim;
    // `P2-R2`'s locators and path classes, which are the only places a
    // contribution may name and the vocabulary the scaffold rubric's path half
    // reads; `P2-R3`'s relation, carried through `P2-R4`'s observed proof so a
    // reader sees which relation observed the use; `academic-domain` for
    // section 13.1's mastery level, which is what says a candidate is offered
    // at `APPLIED` and nothing above it; and `academic-policy` for the digests
    // the two claim identities are, because joined and truncated identities
    // collide. The edges it does not have are the point: no `academic-store` --
    // it persists nothing and adds no migration -- and no `academic-worker` and
    // no `academic-egress-boundary`, so nothing in its closure can launch a
    // process or stage a payload.
    "academic-repository-competency": [
      "academic-domain",
      "academic-policy",
      "academic-repository-analysis",
      "academic-repository-classification",
      "academic-repository-correlation",
    ],
    "academic-repository-analyzer": ["academic-policy"],
    // `P2-K4`'s recovery-profile registry, independent backup key, and
    // rehearsal receipt. It sits above the key schedule and below the
    // portability boundary, opens no database, and reads no vault.
    "academic-recovery": ["academic-crypto"],
    // `P2-K5`'s rotation journal, recipient revocation, crypto-shred, and
    // retention vocabulary. `academic-vault` is an optional edge behind the
    // non-default `rotation-engine` feature, which is what selects the vault's
    // own non-default encrypted object lane; `cargo metadata` reports declared
    // dependencies rather than resolved ones, so it is listed here and
    // `rotation_engine_lane_is_not_default` proves it stays unresolved in a
    // default build.
    "academic-retention": ["academic-crypto", "academic-domain", "academic-vault"],
    // `P2-Y2`'s section 24.2 role bundle. Three product edges and each one is a
    // boundary it reads a fact out of rather than a vocabulary it rebuilds:
    // `academic-competency` for `P2-Y1`'s competency identity, which is the only
    // thing a bundle entry may name in that position; `academic-domain` for
    // section 7.1's `RoleProfile` node type and section 7.2's `RELEVANT_TO_ROLE`
    // descriptor, whose one required qualifier -- `role_profile_version`, of
    // kind `PositiveInteger` -- is where the version's shape comes from; and
    // `academic-ingestion` for `dating::Date` alone, because section 24.2's
    // `validAt` is the document's own date and that module already owns the
    // separation of valid time from the wall clock. The edges it does *not*
    // have are the point: no `academic-store` -- it persists nothing and adds no
    // migration -- no `academic-worker` and no `academic-egress-boundary`, so
    // nothing in its closure can launch a process or stage a payload, and no
    // HTTP or feed edge of any kind, which is what `GATE-38-029` staying open
    // looks like in the dependency graph.
    "academic-role-profile": [
      "academic-competency",
      "academic-domain",
      "academic-ingestion",
    ],
    // `P2-U8`'s course-review boundary. Five product edges and not one of them a
    // transport, a store or a serializer: section 29.5 keeps somebody else's
    // writing private and never redistributes it, and a crate with no way to
    // fetch and no way to serialise is where that starts. `academic-curriculum`
    // supplies the instructor name and term code two of the four scope
    // dimensions are, `academic-domain` the offering and course identifiers,
    // `academic-ingestion` `P2-U6`'s four fallbacks and single denial route,
    // `academic-proposal` the `AI_INFERRED` constant section 29.5 writes as a
    // literal, and `academic-untrusted-content` `P2-G5`'s trust label. The edge
    // that is deliberately absent in the other direction is
    // `academic-curriculum` -> `academic-review`: a `Course` that could name a
    // review reading is section 34's *Course와 Offering 혼동* row.
    "academic-review": [
      "academic-curriculum",
      "academic-domain",
      "academic-ingestion",
      "academic-proposal",
      "academic-untrusted-content",
    ],
    "academic-rpc": ["academic-admission", "academic-contracts", "academic-domain"],
    "academic-scenario": ["academic-domain"],
    // `academic-crypto` is an optional edge behind `sqlcipher-store`. It is
    // listed here because `cargo metadata` reports declared dependencies, not
    // resolved ones: the encrypted lane's `SKEY_p` comes from the `P2-K1` key
    // schedule rather than from a second derivation inside the store, and
    // `sqlcipher_feature_is_not_default` proves the edge stays unresolved in a
    // default build.
    "academic-store": [
      "academic-contracts",
      "academic-crypto",
      "academic-domain",
      "academic-ledger",
      "academic-store-platform",
      "academic-vault",
    ],
    "academic-keystore-platform": [],
    "academic-store-platform": [],
    "academic-test-support": [],
    // `P2-U7`'s transcript ingestion boundary. `academic-vault` is an optional
    // edge behind the non-default `encrypted-vault` feature, which is what
    // selects the vault's own non-default `AEAD_CHUNKED_V2` lane; `cargo
    // metadata` reports declared dependencies rather than resolved ones, so it
    // is listed here and `transcript_encrypted_lane_is_not_default` proves it
    // stays unresolved in a default build.
    "academic-transcript": ["academic-admission", "academic-domain", "academic-vault"],
    // `P2-G5`'s untrusted-content boundary. Its one product edge is the egress
    // boundary, and that is the whole reuse claim: `ingest_provider_response`
    // takes the `AcceptedResponse` that `P2-G2`'s provider-response scan is the
    // only producer of, so a response this crate is handed has been scanned.
    // `academic-policy` is deliberately a dev edge below rather than here, so a
    // product file cannot name `PermissionBroker`, `CapabilityToken`,
    // `RuntimeToolCall`, or `ProcessCapabilityToken` at all.
    // `P2-L3`. Five product edges, each one a boundary this crate refuses to
    // restate. `academic-capture` is where a chunk journal's header carries the
    // section 3.7 capability token an input is admitted against, and where the
    // `CaptureRecorder` that opens a binding comes from;
    // `academic-egress-boundary` is where an `AcceptedResponse` comes from, so
    // the scoped-remote route cannot be handed a response that spent no grant;
    // `academic-untrusted-content` is the one trust label a raw provider
    // response leaves the archive under; `academic-model-run` is the twelve
    // section 27.3 fields and the `RawScore` that cannot be ordered; and
    // `academic-proposal` is the three dispositions a user correction is one
    // of. There is deliberately no edge to `academic-worker` -- no workspace
    // crate may depend on that package at all -- and none to `academic-store`,
    // which is what makes "this crate persists nothing" a graph fact.
    // `P2-L4`. Four product edges. `academic-transcription` is the versioned
    // transcript every mapping names -- the document is built over
    // `TranscriptSegment` and `EffectiveToken`, never over a raw type;
    // `academic-capture` is the journal whose frames are the audio timeline and
    // whose gap frames are the only evidence a `UNTRANSCRIBED_FAILURE` status
    // may cite; `academic-model-run` is the calibration a low-confidence span is
    // decided by, because a provider's raw number has no ordering; and
    // `academic-domain` carries `P2-C5`'s deterministic engine signature. There
    // is deliberately no `academic-store` edge, which is what makes "this crate
    // persists nothing and adds no migration" a graph fact, and no
    // `academic-egress-boundary` edge, because nothing here transmits.
    "academic-lecture-document": [
      "academic-capture",
      "academic-domain",
      "academic-model-run",
      "academic-transcription",
    ],
    "academic-transcription": [
      "academic-capture",
      "academic-domain",
      "academic-egress-boundary",
      "academic-model-run",
      "academic-proposal",
      "academic-untrusted-content",
    ],
    "academic-untrusted-content": ["academic-egress-boundary"],
    // `academic-crypto` is an optional edge behind `aead-objects`, the same
    // shape the store's encrypted lane uses: `cargo metadata` reports declared
    // dependencies rather than resolved ones, and
    // `encrypted_object_lane_is_not_default` proves the edge stays unresolved
    // in a default build.
    "academic-vault": ["academic-crypto", "academic-domain"],
    // `P2-G4`'s sandbox. Its one product edge is the domain, for the
    // `ModelRunId` a resource receipt is paired with. `libc` and `windows-sys`
    // are optional target-specific edges behind the non-default
    // `native-sandbox` feature and are not workspace packages, so they appear
    // in `SOCKET_CAPABLE_CLOSURES` below rather than here.
    // `P2-N2`'s section 13 knowledge state. Four product edges and each is a
    // boundary it reads a fact out of rather than a vocabulary it restates:
    // `academic-domain` for section 13.1's `MasteryLevel`, section 13.3's
    // `FreshnessBand` and ADR-003's actor matrix, which is what makes a user
    // confirmation unmintable by a model run; `academic-ledger` for `P2-M3`'s
    // `ConflictReason`, so the review card carries the conflict token section
    // 30.3 already fixed rather than a second one; `academic-lecture-document`
    // so a teaching site is a node of a real `P2-L4` document rather than a
    // string; and `academic-repository-classification` so the difference between
    // section 13.2's fourth and seventh rows is `P2-R4`'s `ObservedProof`
    // rather than a heuristic. The edges it does not have are the point: no
    // `academic-store` -- it persists nothing and adds no migration -- and no
    // `academic-worker` and no `academic-egress-boundary`, so nothing in its
    // closure can launch a process or stage a payload.
    "academic-knowledge-state": [
      "academic-domain",
      "academic-lecture-document",
      "academic-ledger",
      "academic-repository-classification",
    ],
    // `P2-N8`. Section 22's what-if semester simulator, and the one task in the
    // knowledge slice whose job is to put four other engines side by side. Each
    // edge is a boundary it reads rather than a vocabulary it restates:
    // `academic-scenario` for `P2-C7`'s whole projected lane, which supplies
    // section 22.3's first three bullets unmodified and the sealed wrapper the
    // workload proposal lives in; `academic-critical-path` for `P2-N6`'s
    // `CriticalPathResult::roles`, so the coverage projection is an overlap
    // with that engine's answer and not a second path; `academic-offering` for
    // `P2-U5`'s `ConfirmedSeat`, whose one producer is `ConfirmedStanding::seat`
    // — a `HISTORICALLY_LIKELY` offering has no seat, so the deterministic lane
    // cannot be computed over a predicted one; `academic-record` for `P2-U4`'s
    // versioned `GradingScheme`, so a stated-grade GPA reads section 10's table
    // rather than a copy of it; `academic-review` for `P2-U8`'s
    // `BiasDisclosure`, taken by value so a workload has no form without its
    // bias; `academic-proposal` for `P2-M2`'s `UserDecision`, which is what
    // makes section 22.5's recomputation consent a human's; and
    // `academic-curriculum` and `academic-domain` for the catalogue and
    // identifier vocabularies. The edges it does not have carry the claim: no
    // `academic-store` and no `academic-store-platform`, which is `INV-C-009`
    // as a graph fact and the reason this task adds no migration; and no
    // `academic-audit` of any edge kind, so the hypothetical graduation mode
    // has no route to `P2-U3`'s three-gate `DETERMINATE` rule and
    // `a_plan_cannot_name_the_graduation_verdict` is an unresolved module
    // rather than a refused call.
    "academic-what-if": [
      "academic-critical-path",
      "academic-curriculum",
      "academic-domain",
      "academic-offering",
      "academic-proposal",
      "academic-record",
      "academic-review",
      "academic-scenario",
    ],
    "academic-worker": ["academic-domain"],
    // `academic-domain` for section 13.3's `FreshnessBand`, `ClaimObject::Freshness`
    // as the wire shape of a claim about one, section 7.2's `PredicateName` for
    // the four edges a spillover may be cited on, and ADR-003's actor matrix,
    // which is what makes a recall confirmation unmintable by a model run; and
    // `academic-knowledge-state` for `EligibleEvidence`, so freshness reads only
    // evidence that passed section 13.4's four checks, and for `FreshnessInput`,
    // which is the one value it hands back. The edge it does **not** have is the
    // point: `academic-knowledge-state` re-exports `LADDER`, `rung`,
    // `AutomaticLevel` and `MasteryProjection` and this crate imports none of
    // them, so `시간 decay는 freshness projection에만 적용한다` is a fact about
    // the import graph rather than a rule inside one signature. No
    // `academic-store` -- it persists nothing and adds no migration -- and no
    // `academic-worker` and no `academic-egress-boundary`, so nothing in its
    // closure can launch a process or stage a payload.
    "academic-freshness": ["academic-domain", "academic-knowledge-state"],
    // `P2-N7` reads two boundaries and computes neither.
    // `academic-domain` is `P2-C1` and `P2-N1`: section 7.4's three primary node
    // types, so the granularity the user selects is one of that crate's tiers
    // and a concept resolves to its field through a `VersionedTaxonomyImport`
    // whose identity binds the release the scope names; `FreshnessBand`, which
    // this crate carries and never computes; and ADR-003's actor matrix, which
    // is what makes a disposition unmintable by a model run and therefore what
    // makes `새로운 AI run이 경고를 되살리지 않는다` a property of the type.
    // `academic-knowledge-state` is `P2-N2` for `EligibleEvidence`, so coverage
    // counts evidence that passed section 13.4's four checks rather than
    // anything a caller calls evidence, and for `Outcome`, so `WEAK` is that
    // crate's own record of an attempt that did not succeed. The edges it does
    // **not** have are the point: no `academic-gap`, so `모든 분야를 균등하게
    // 채우라는 목표를 만들지 않는다` is a graph fact -- a goal this engine could
    // emit would first have to be a goal it could name -- and no
    // `academic-freshness`, so which concept is stale arrives as `P2-N3`'s band
    // rather than as a threshold here. No `academic-store` -- it persists
    // nothing and adds no migration -- and no `academic-worker` and no
    // `academic-egress-boundary`, so nothing in its closure can launch a process
    // or stage a payload.
    "academic-blind-spot": ["academic-domain", "academic-knowledge-state"],
    // `P2-R6`. Section 20's Build -> Learn mode and section 21's course-to-project
    // mapping. Six product edges and each is a boundary it reads a decided fact
    // out of: `academic-critical-path` for section 16.1's AND/OR hypergraph and
    // its satisfaction answer, so section 20.2's `concept requirements with
    // AND/OR branches` is that crate's structure and that crate's solver and no
    // path length exists here; `academic-gap` for the admitted
    // `PrerequisiteEdge` a hyperedge member is built over, for `gap_bearing`,
    // which refuses a whole-field requirement, and for the four-dimension
    // `ConceptState` overlay the readiness comparison reads;
    // `academic-knowledge-state` for `SufficiencyGap`, which is what `충분하고
    // 최근인 evidence` means; `academic-repository-classification` for `P2-R4`'s
    // `BenefitContract`, carried whole so the `trigger 기반 benefit` row shows
    // the trigger and the trade-off that crate published; `academic-curriculum`
    // for `CourseRevision`, `CourseOffering` and section 8.3's four standings,
    // so a course's canonical coverage and an offering's actual coverage are
    // that crate's values rather than a title read here; and `academic-domain`
    // for the identities and the two readings a finding carries. The edges it
    // does not have are the point: no `academic-freshness` as a product edge, so
    // which concept is stale arrives as `P2-N3`'s band through `P2-N6`'s own
    // predicate rather than as a threshold here; no `academic-store`, so a plan
    // cannot reach the canonical writer and it adds no migration; and no
    // `academic-worker`, no `academic-egress-boundary` and no
    // `academic-model-run`, so nothing in its closure can launch a process,
    // stage a payload or call a model.
    "academic-build-learn": [
      "academic-critical-path",
      "academic-curriculum",
      "academic-domain",
      "academic-gap",
      "academic-knowledge-state",
      "academic-repository-classification",
    ],
    // `P2-L5`'s student voice, diarization measurement and capture PII hold.
    // Five product edges and each is a boundary it resolves rather than
    // restates: `P2-L4`'s `RedactionPolicyRef`, whose digest that crate's
    // contract page leaves for this task to resolve; `P2-L3`'s `Speaker`, which
    // is section 12.4's own three shapes and not a second vocabulary; `P2-G6`'s
    // `RetentionTerms` and deletion preview, so the inheritance rule is called
    // rather than copied; `P2-L2`'s `CaptureBytes`, which a hold holds and
    // hands out to nobody; and `academic-domain` for the actor and digest
    // types. The edges it does not have are the point: no `academic-retention`,
    // because `rotation_engine_lane_is_not_default` holds that exactly one
    // crate declares it and a redaction has no business inside a boundary that
    // can destroy a key slot; no `academic-store`, so it persists nothing and
    // adds no migration; and no `academic-policy`, `academic-egress-boundary`
    // or `academic-worker`, so no product file here can name a broker, an
    // egress or a job.
    "academic-student-voice": [
      "academic-capture",
      "academic-consent",
      "academic-domain",
      "academic-lecture-document",
      "academic-transcription",
    ],
    // `P2-N5` reads three boundaries and computes none of them.
    // `academic-domain` for section 7.2's predicate registry -- its
    // `prerequisite` column is what admits a traversal edge, so `REQUIRES와
    // 강한 BUILDS_ON` is eighteen rows refused there rather than an allowlist
    // here -- and for `EntityKind`, whose `FIELD` tier `carries no independent
    // prerequisite of its own` and is therefore what refuses section 15.3's
    // `데이터베이스를 더 공부하세요` without this crate holding a phrase.
    // `academic-knowledge-state` for two of section 15.2 step 3's four overlay
    // dimensions and for the admission decision behind them, and
    // `academic-freshness` for the third. The `academic-freshness` edge is the
    // one that needs a reason: section 13.3 licenses a spillover on four
    // section 7.2 edges and **two of those four are the edges this engine
    // descends**, so a band raised by a neighbour on the node's own blocking
    // path is refused by name. No `academic-store` -- it persists nothing and
    // adds no migration -- and no `academic-worker` and no
    // `academic-egress-boundary`, so nothing in its closure can launch a
    // process or stage a payload.
    "academic-gap": [
      "academic-domain",
      "academic-freshness",
      "academic-knowledge-state",
    ],
    // `P2-X2`. Section 25.2's `Home / Today`. Two product edges and no third.
    // `academic-domain` supplies every identifier a card names and the
    // `FreshnessBand` its eighth line is about; `academic-consent` supplies
    // `P2-G6`'s `CaptureStatus`, which the four permission words are the total
    // image of rather than a second status vocabulary. The edges it does not
    // have are the point: no `academic-knowledge-state` and no
    // `academic-freshness`, so no mastery ladder is in the closure this
    // surface compiles against -- `P2-N3`'s rule that decay reaches a freshness
    // projection and never a mastery is held here the way it is held there,
    // and `the_home_surface_cannot_name_a_mastery` is the measurement; no
    // `academic-store` and no `academic-vault`, so it persists nothing and
    // claims no migration number; and no `academic-policy` and no
    // `academic-egress-boundary` of any edge kind, so it holds a permission
    // *status* and can name no grant, token or staged payload at all.
    "academic-home": ["academic-consent", "academic-domain"],
    // `P2-P1`'s section 37 graduation export. Three product edges, and the
    // edges it does not have are the whole point: `INV-C-015` is the claim that
    // a user can read their own record when this product and their school
    // account are both gone, so a bundle writer that needed the database engine
    // and a reader that needed the key hierarchy would make that claim about
    // software the user no longer has. There is no `academic-store`, no
    // `academic-vault`, no `academic-crypto`, no `academic-keystore-platform`,
    // no `academic-recovery`, no `academic-retention`, no `academic-projections`
    // and no `academic-rpc`. `academic-domain` carries the identifiers, the
    // content digest and the section 3.9 engine vocabulary the recorded audit
    // is expressed in; `academic-audit` and `academic-requirement` are what make
    // the clean-room restore re-run `P2-U3`'s selection and evaluation rather
    // than re-read a verdict.
    "academic-export": [
      "academic-audit",
      "academic-domain",
      "academic-requirement",
    ],
    // `P2-N6` reads four boundaries and computes none of them.
    // `academic-gap` is the input: this engine plans around a `GapCase` rather
    // than around a concept and a state, so section 15.1's restraint carries
    // forward as a graph fact -- there is no other producer of one and no way
    // to call this engine without it -- and a hyperedge member is that crate's
    // admitted `PrerequisiteEdge`, so `P2-C4`'s prerequisite column is what
    // admits an edge and there is no allowlist here.
    // `academic-domain` is `P2-C1` and `P2-C5`: the identifiers, the six
    // freshness bands section 16.3's seventh constraint matches over, and the
    // whole engine-harness module that makes determinism a byte comparison.
    // **No registry row is added**: section 28 tabulates twelve engines and
    // none of them is a critical path engine.
    // `academic-curriculum` is `P2-U1`, for section 8.3's four offering
    // standings and for the meetings and credits section 16.3's third
    // constraint reads. `academic-freshness` is `P2-N3`, because which concept
    // is stale is that crate's band and not a threshold here.
    // No `academic-store` -- it persists nothing and adds no migration -- and
    // no `academic-worker`, `academic-egress-boundary` or `academic-model-run`,
    // so no product file here can launch a process, stage a payload or call a
    // model.
    "academic-critical-path": [
      "academic-curriculum",
      "academic-domain",
      "academic-freshness",
      "academic-gap",
    ],
  });
  const graph = new Map(Object.entries(actual));
  assertAcyclic(graph);

  // Test-only edges are frozen separately. `academic-daemon` owns the X1 exit
  // harness in `tests/phase1_exit.rs`, which drives projection rebuilds and
  // reads published backup, restore, and vault evidence, so it links those
  // three crates while a test target is compiling and never in a product build.
  const devEdges = Object.fromEntries(
    workspacePackages
      .map((pkg) => [pkg.name, devDependencyNames(pkg)])
      .filter(([, dependencies]) => dependencies.length > 0)
      .toSorted(([left], [right]) => left.localeCompare(right)),
  );
  assert.deepEqual(devEdges, {
    // `P2-L1`. The two boundaries a quarantined capture must not reach are dev
    // edges rather than product ones: what this crate owns is that a
    // quarantined artefact hands out no bytes, and
    // `violation_risk_blocks_share_and_ai_processing` observes that against the
    // real staging pipeline and the real prompt envelope rather than against a
    // local imitation. `academic-policy` arrives with them. `academic-consent`
    // and `academic-domain` are declared twice for the `trybuild` reason
    // `academic-scenario` gives below.
    "academic-capture-gate": [
      "academic-consent",
      "academic-domain",
      "academic-egress-boundary",
      "academic-policy",
      "academic-untrusted-content",
    ],
    // `P2-G6` restates `academic-retention`'s derivative-class list rather than
    // importing it, and this is the edge that keeps the restatement honest:
    // `the_two_derivative_vocabularies_are_the_same_list` compares both lists
    // whole. A dev edge reaches no product binary, so the rotation engine stays
    // out of every shipping graph. `academic-domain` is declared twice for the
    // `trybuild` reason `academic-scenario` gives below.
    "academic-consent": ["academic-domain", "academic-retention"],
    // `academic-core` owns `tests/scenario_isolation.rs`, which needs the
    // projection engine and the canonical writer in one process to prove that
    // driving the first leaves the second byte-identical. `academic-scenario`
    // links its own domain crate a second time as a dev edge because the
    // `trybuild` cases compile against the crate under test plus that crate's
    // dev-dependencies, and a case has to name the canonical types a projection
    // must never become.
    "academic-core": ["academic-scenario"],
    // `P2-U3` links its own domain, record and requirement crates a second
    // time as dev edges for the `trybuild` reason `academic-scenario` gives
    // below, and links `academic-scenario` itself for one reason: a
    // compile-fail case has to name the `Proposed<T>` it proves cannot enter an
    // audit. That is a test edge only -- `no_product_file_names_a_projection_and_only_one_names_a_plan`
    // sweeps every product file of the crate for the same name and requires it
    // to be absent.
    "academic-audit": [
      "academic-domain",
      "academic-record",
      "academic-requirement",
      "academic-scenario",
    ],
    // `P2-U1` links its own domain crate a second time as a dev edge for the
    // `trybuild` reason `academic-scenario` gives below: a compile-fail case
    // compiles against the crate under test plus that crate's dev-dependencies,
    // and a case has to name the domain identifiers an aggregate is built from.
    // `P2-X5`'s suite builds every graph in process. `academic-domain` is
    // declared a second time for the `trybuild` reason `academic-scenario`
    // gives below: a compile-fail case compiles against the crate under test
    // plus that crate's dev-dependencies, and six of the eight cases name a
    // domain value. `serde_json` is how a frame's channel key set is compared
    // as a whole set rather than field by field, and `uuid` derives the fixture
    // identities from a digest of a tag rather than from a clock.
    "academic-cs-map": ["academic-domain"],
    "academic-curriculum": ["academic-domain"],
    // `P2-M4`'s acceptance suite drives the real doors rather than describing
    // them, and each dev edge is one of the six actions its compiled constant
    // has to agree with: `academic-domain` for the question resolution that
    // already refuses an automatic actor, `academic-knowledge-state` for
    // `P2-N2`'s `UserConfirmation` and the `AutomaticLevel` with no `Fluent`
    // variant, `academic-deletion` for `P2-P2`'s `DeletionConfirmation` --
    // the one action this task must not implement twice --
    // `academic-record` and `academic-consent` for the two constructors that
    // take **no** actor, which is how "the record and consent layers cannot
    // refuse this" is measured rather than assumed, and `academic-policy` for
    // the broker whose two rules differing only in `actor_id` both allow.
    // `academic-audit` is named by the `trybuild` case that observes
    // `DeterminateVerdict::new` is `pub(crate)`. `academic-retention` and
    // `academic-student-voice` are two arguments and nothing else: the
    // `DerivativeClass` in the `DerivativeIndex` signature and the
    // `EvidenceIndex` the real preview takes. `academic-domain` and
    // `academic-proposal` are declared a second time for the `trybuild` reason
    // `academic-scenario` gives below.
    "academic-non-delegable": [
      "academic-audit",
      "academic-consent",
      "academic-deletion",
      "academic-domain",
      "academic-knowledge-state",
      "academic-policy",
      "academic-proposal",
      "academic-record",
      "academic-retention",
      "academic-student-voice",
    ],
    // `P2-U5` links four of its own product crates a second time as dev edges
    // for the `trybuild` reason `academic-scenario` gives below: a compile-fail
    // case compiles against the crate under test plus that crate's
    // dev-dependencies, and the seven cases have to name a `CourseCode`, a
    // `TermKey`, a `TimestampMillis` and a `CalibratedConfidence` between them.
    "academic-offering": [
      "academic-curriculum",
      "academic-domain",
      "academic-model-run",
      "academic-record",
    ],
    // `P2-P2`'s acceptance suite drives the real things rather than fabricating
    // them: `academic-policy`'s broker really registers a provider policy,
    // issues a grant, executes a capability and stores the deletion receipt
    // against its allow-audit row, so the link this crate adds is to a row that
    // exists; `academic-crypto` builds the profile the `RB01` child reopens
    // from its own recipient record; and `serde_json` hands that child the
    // descriptor the parent sealed. `academic-domain`, `academic-proposal` and
    // `academic-retention` are declared a second time for the `trybuild` reason
    // `academic-scenario` gives below. `academic-policy` stays a **dev** edge:
    // a deletion flow whose product closure held the broker could name a type
    // that owns a transmitted byte, and `every_field_type_in_this_crate_is_reviewed`
    // rests on that closure not holding one.
    "academic-deletion": [
      "academic-crypto",
      "academic-domain",
      "academic-policy",
      "academic-proposal",
      "academic-retention",
    ],
    // `P2-X7` links its own domain and proposal crates a second time as dev
    // edges for the `trybuild` reason `academic-scenario` gives below: a
    // compile-fail case compiles against the crate under test plus that
    // crate's dev-dependencies, and a case has to name the identifiers and the
    // `UserDecision` a centre entry is built from.
    "academic-evidence-center": ["academic-domain", "academic-proposal"],
    // `P2-X2` links its own consent and domain crates a second time as dev
    // edges for the same `trybuild` reason: a compile-fail case compiles
    // against the crate under test plus that crate's dev-dependencies, and the
    // four cases have to name an `EntityId`, a `TimestampMillis` and a
    // `FreshnessBand`. `academic-consent` is declared twice so that a case can
    // reach the `CaptureStatus` the four permission words are the image of.
    "academic-home": ["academic-consent", "academic-domain"],
    // `P2-Y3`'s acceptance suite runs the chains rather than fabricating their
    // outputs: `P2-R1`'s capture, `P2-R2`'s ladder, `P2-R3`'s correlation,
    // `P2-R4`'s classification and `P2-R5`'s promotion for the one personal
    // application claim a `FROM_PROJECT` walk starts from, and `P2-Y1`'s and
    // `P2-Y2`'s own `declare` for every competency and bundle.
    // `academic-competency` and `academic-role-profile` are declared a second
    // time for the `trybuild` reason `academic-scenario` gives below.
    // `academic-export` is the edge that carries the task: it is a **dev** edge,
    // so `non_guarantee_disclaimer_survives_export` writes a real `P2-P1`
    // bundle and reads it back with no key and no vendor, and the product
    // closure above still holds no exporter. `academic-audit`,
    // `academic-ingestion`, `academic-record` and `academic-requirement` are
    // what `P2-U3`'s fixture module needs, reached by `#[path]` the way
    // `academic-export`'s own suite reaches it.
    "academic-readiness": [
      "academic-audit",
      "academic-competency",
      "academic-export",
      "academic-ingestion",
      "academic-model-run",
      "academic-policy",
      "academic-record",
      "academic-repository",
      "academic-repository-analysis",
      "academic-repository-classification",
      "academic-repository-competency",
      "academic-repository-correlation",
      "academic-requirement",
      "academic-role-profile",
      "academic-untrusted-content",
    ],
    // `P2-P3`. Six dev edges and no seventh. `academic-ledger` is the real
    // `LedgerState` `core_graph_opens_with_every_connector_down` opens while
    // every connector is down, with `academic-contracts` and `ed25519-dalek`
    // signing the batch it accepts, so the core the test reads is one that went
    // through the real acceptance path. It is a **dev** edge on purpose: the
    // claim is that the core read path does not run through this boundary, and a
    // product edge would put the ledger inside the closure of the crate that is
    // supposed to be irrelevant to it. `academic-crypto` is the in-memory
    // `DeviceKeystore` double the GitHub credential fixture uses. `academic-domain`,
    // `academic-model-run` and `academic-repository` are declared a second time
    // for the `trybuild` reason `academic-scenario` gives below: a compile-fail
    // case compiles against the crate under test plus that crate's
    // dev-dependencies, and the three cases have to name a `ModelRunId`, a
    // `Digest32` and a `TimestampMillis`.
    "academic-integrations": [
      "academic-contracts",
      "academic-crypto",
      "academic-domain",
      "academic-ledger",
      "academic-model-run",
      "academic-repository",
    ],
    // `P2-L6`. Thirteen dev edges and no fourteenth. Eleven of them are what
    // `crates/knowledge-state/tests/common/mod.rs` needs, included here by
    // `#[path]` through `P2-N5`'s own fixture module rather than restated, so
    // the lecture a claim's exposure evidence rests on is a `P2-L4` document
    // that a real `P2-L2` capture and a real `P2-L3` run produced. The two this
    // suite adds are `academic-freshness`, whose `project` builds the band a
    // `ConceptReading` carries, and `academic-home`, whose `LOWEST_BRIEF` and
    // `HIGHEST_BRIEF` `morning_home_contract` compares its own bound against.
    // `academic-home` is a **dev** edge on purpose: the claim is that two crates
    // offering the same card cannot drift apart, and a product edge would make
    // one crate's bound the other's by construction rather than by comparison.
    // `academic-domain` is declared a second time for the `trybuild` reason
    // `academic-scenario` gives below.
    "academic-next-lecture": [
      "academic-capture",
      "academic-consent",
      "academic-domain",
      "academic-freshness",
      "academic-home",
      "academic-knowledge-state",
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-repository-analysis",
      "academic-repository-classification",
      "academic-repository-correlation",
      "academic-transcription",
    ],
    "academic-requirement": ["academic-domain"],
    "academic-daemon": ["academic-portability", "academic-projections", "academic-vault"],
    // The encrypted portability acceptance suite builds its keys through the
    // `P2-K1` public schedule rather than fabricating them, exactly as the
    // encrypted object suite does. The product edge is the optional one above.
    "academic-portability": ["academic-crypto"],
    // `academic-vault` owns the encrypted-object acceptance suite, which builds
    // its keys through the `P2-K1` public schedule rather than fabricating
    // them. That is a test edge only; the product edge is the optional one
    // above.
    "academic-vault": ["academic-crypto"],
    // `P2-U7`'s encrypted-lane suite builds `KEK_d` through the same public
    // schedule, for the same reason. `academic-transcript` has no product edge
    // to the key schedule at all: the vault handle it seals through is opened
    // by its caller.
    "academic-transcript": ["academic-crypto"],
    // `P2-R2`'s `Finding` is the third type in this workspace whose
    // construction is closed by compilation rather than by a check, beside
    // `Proposed<T>` and `VerifiedAdmission`. Its four `compile_fail` cases live
    // in this suite rather than in a fourth `compile_fail` target, so the
    // README's verification block gains no command; a case compiles against the
    // crate under test plus that crate's dev-dependencies, which is why the
    // edge is here.
    // `P2-U8` links its own trust-label crate a second time as a dev edge, for
    // the `trybuild` reason above and for one more: the acceptance suite seals a
    // retained review through `P2-G5`'s boundary and reads the provenance that
    // comes back, so a test target has to name `SourceId` and `SourceKind`. It
    // is the crate's only dev workspace edge -- there is no store, no vault and
    // no export crate in its test closure either.
    "academic-review": ["academic-untrusted-content"],
    "academic-scenario": [
      "academic-admission",
      "academic-domain",
      "academic-repository-analysis",
    ],
    // `P2-M2` links its own domain crate a second time as a dev edge for the
    // `trybuild` reason `academic-scenario` gives above: a compile-fail case
    // compiles against the crate under test plus that crate's dev-dependencies,
    // and a case has to name the domain types a `Proposed<T>` is built from.
    "academic-proposal": ["academic-domain"],
    // `P2-N8` links its own eight product crates a second time as dev edges for
    // the `trybuild` reason `academic-scenario` gives above, and adds two the
    // product path does not need: `academic-ingestion` for the `SourceCategory`
    // a `P2-U5` official listing is built from, and `academic-model-run` for the
    // `CalibrationRegistry` that crate's `resolve` takes by reference. The
    // acceptance suite drives that `resolve` rather than assembling a seat,
    // because `ConfirmedStanding::seat` is the only producer of a
    // `ConfirmedSeat` in this workspace. There is no store, no vault, no audit
    // and no export crate in its test closure either.
    "academic-what-if": [
      "academic-critical-path",
      "academic-curriculum",
      "academic-domain",
      "academic-ingestion",
      "academic-model-run",
      "academic-offering",
      "academic-proposal",
      "academic-record",
      "academic-review",
      "academic-scenario",
    ],
    // `P2-G5` needs a real `PermissionBroker` to build an `EgressProxy` and a
    // real `ProcessCapability` to enumerate what a privileged action is. Both
    // are test-only: keeping `academic-policy` off the product edge above is
    // what makes "the adjudicator receives no capability" a compile error
    // rather than a source scan.
    // `P2-L3` drives a real capture through `academic_capture::begin` and a
    // real `EgressProxy` over a real `PermissionBroker`, so `academic-consent`
    // and `academic-policy` are test edges. Keeping `academic-policy` off the
    // product edge above is what makes "this crate mints no grant and holds no
    // broker" a compile error rather than a source scan, and keeping
    // `academic-consent` off it is what makes "this crate adds no second
    // section 3.7 comparison" the same. `academic-domain` is declared twice for
    // the `trybuild` reason `academic-scenario` gives above.
    // `P2-L4`'s acceptance suite drives a real `academic_capture::begin` and a
    // real `P2-L3` pipeline run, so the journal its gap check reads is the file
    // the real capture surface wrote. There is no `academic-policy` and no
    // `academic-egress-boundary` edge of either kind, which is what makes "this
    // crate mints no grant, holds no broker and opens no egress" a compile error
    // rather than a source scan. `academic-domain` is declared twice for the
    // `trybuild` reason `academic-scenario` gives above.
    "academic-lecture-document": ["academic-consent", "academic-domain"],
    "academic-transcription": [
      "academic-consent",
      "academic-domain",
      "academic-policy",
    ],
    "academic-untrusted-content": ["academic-policy"],
    // `P2-R3`'s acceptance suite runs `P2-R1`'s capture and `P2-R2`'s ladder
    // over synthetic corpora rather than fabricating a finding, so it needs the
    // calibration registry an `OBSERVED` rung requires, the digest a snapshot
    // request carries, and the sealed index `AnalysisInput::of` checks against.
    // All three are test edges; the product edges above hold none of them.
    "academic-repository-correlation": [
      "academic-model-run",
      "academic-policy",
      "academic-untrusted-content",
    ],
    // `P2-R4`'s acceptance suite runs `P2-R1`'s capture, `P2-R2`'s ladder and
    // `P2-R3`'s correlation over synthetic corpora rather than fabricating a
    // finding, so it needs the calibration registry an `OBSERVED` rung requires,
    // the snapshot request builder, and the sealed index `AnalysisInput::of`
    // checks against. `academic-policy` is a product edge here rather than a
    // test one, so it does not appear below.
    "academic-repository-classification": [
      "academic-model-run",
      "academic-repository",
      "academic-untrusted-content",
    ],
    // `P2-N2`'s acceptance suite builds its lecture evidence by driving a real
    // `P2-L2` capture and a real `P2-L3` run, and its project evidence by
    // driving `P2-R1`'s capture, `P2-R2`'s ladder and `P2-R3`'s correlation,
    // rather than fabricating a document node or a relation edge. Every edge
    // here is one of those two chains. `academic-policy` is a test edge rather
    // than a product one, which is what makes "this crate mints no grant and
    // holds no broker" a compile error rather than a source scan.
    "academic-knowledge-state": [
      "academic-capture",
      "academic-consent",
      "academic-domain",
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-repository-analysis",
      "academic-repository-correlation",
      "academic-transcription",
      "academic-untrusted-content",
    ],
    // `P2-Y1`'s acceptance suite runs the whole repository chain below it --
    // `P2-R1`'s capture, `P2-R2`'s ladder, `P2-R3`'s correlation, `P2-R4`'s
    // classification and `P2-R5`'s promotion -- over synthetic corpora, and
    // admits its knowledge-state items through `P2-N2`'s own four eligibility
    // checks, rather than fabricating a finding or an admitted item.
    // `academic-policy` is a test edge rather than a product one, which is what
    // makes "this crate mints no identity of its own" a compile error rather
    // than a source scan. `academic-domain` is declared twice for the
    // `trybuild` reason `academic-scenario` gives below.
    "academic-competency": [
      "academic-domain",
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-repository-analysis",
      "academic-repository-classification",
      "academic-repository-correlation",
      "academic-untrusted-content",
    ],
    // `P2-R5`'s acceptance suite runs the whole chain below it over synthetic
    // corpora rather than fabricating a finding, so it needs the same three
    // `P2-R4` does. It needs no fourth: `academic-repository-correlation` is a
    // product edge here rather than a test one, because a project claim's
    // provenance carries `P2-R3`'s own relation.
    "academic-repository-competency": [
      "academic-model-run",
      "academic-repository",
      "academic-untrusted-content",
    ],
    // `P2-Y2`'s acceptance suite builds its bundle entries by running `P2-Y1`'s
    // own `CompetencyId::new` rather than fabricating a member, so
    // `academic-competency` is declared twice for the `trybuild` reason
    // `academic-scenario` gives below: a compile-fail case compiles against this
    // crate plus its dev-dependencies. It needs no second test edge -- the
    // remaining inputs of a bundle are the user's own words and a date, and a
    // date is `academic-ingestion`'s, which is already a product edge.
    "academic-role-profile": ["academic-competency"],
    // `P2-N3`'s acceptance suite dates real `EligibleEvidence`, and section
    // 13.2's first row -- the exposure side of section 13.3's persistence
    // sentence -- is a node of a document `P2-L4` produced. It reaches that
    // through `P2-N2`'s own fixture module by `#[path]` rather than restating
    // it, the way `academic-curriculum` includes `academic-ingestion`'s, so
    // every edge here is one that module's two chains need.
    "academic-freshness": [
      "academic-capture",
      "academic-consent",
      "academic-domain",
      "academic-lecture-document",
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-repository-analysis",
      "academic-repository-classification",
      "academic-repository-correlation",
      "academic-transcription",
      "academic-untrusted-content",
    ],
    // `P2-N7`'s acceptance suite counts real `EligibleEvidence`, and section
    // 23's first exposure source -- `강의` -- is a node of a document `P2-L4`
    // produced. It reaches that through `P2-N2`'s own fixture file by `#[path]`,
    // the way `academic-freshness` and `academic-gap` reach the module above it,
    // and it takes the **lecture half only**: this suite needs no `P2-R4`
    // stance, so the repository chain those two carry is absent here.
    // `academic-knowledge-state` is a dev edge as well as a product one for the
    // `trybuild` reason `academic-scenario` gives: a compile-fail case compiles
    // against this crate plus its dev-dependencies, and
    // `an_exposure_item_takes_only_admitted_evidence` names that crate's
    // `BlockedEvidence` to show inadmissible evidence cannot be counted as
    // exposure.
    "academic-blind-spot": [
      "academic-capture",
      "academic-consent",
      "academic-domain",
      "academic-knowledge-state",
      "academic-lecture-document",
      "academic-model-run",
      "academic-transcription",
    ],
    // `P2-R6`. The acceptance suite plans over a real `P2-N5` `ConceptState`
    // overlay, and a real overlay needs a real `P2-N2` eligibility and a real
    // `P2-N3` band, so `crates/gap/tests/common/mod.rs` is included by `#[path]`
    // and every edge that module's two chains need is a dev edge here — the same
    // set `academic-critical-path` declares for the same reason.
    // `academic-critical-path`, `academic-domain`, `academic-gap` and
    // `academic-knowledge-state` are declared a second time for the `trybuild`
    // reason `academic-scenario` gives below: a compile-fail case compiles
    // against the crate under test plus that crate's dev-dependencies, and six
    // of the eight cases name a domain value. `academic-repository-analysis`
    // supplies the `SubjectId` `P2-R4`'s `BenefitDraft` takes, so the
    // `LATER_SCALE` fixture's contract is built through that crate's own
    // builder. `serde_json` is how the goal's, the constraint's, the decision's
    // and the motivation display's key sets are compared as whole sets rather
    // than field by field, and `uuid` derives the fixture identities from a
    // digest of a tag rather than from a clock.
    "academic-build-learn": [
      "academic-capture",
      "academic-consent",
      "academic-critical-path",
      "academic-domain",
      "academic-freshness",
      "academic-gap",
      "academic-knowledge-state",
      "academic-lecture-document",
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-repository-analysis",
      "academic-repository-correlation",
      "academic-transcription",
      "academic-untrusted-content",
    ],
    // `P2-L5`. `academic-retention` is a dev edge for the reason
    // `academic-consent` takes it as one: `GATE-38-026`'s statement and the
    // derivative-class vocabulary are compared against `P2-K5`'s own rather
    // than trusted to stay in step, and the scan that refuses an
    // `OriginalVoiceAuthority` has to name the type it refuses. The other two
    // are so the acceptance suite can drive a real `academic_transcription::run`
    // end to end rather than fabricating utterances; keeping them off the
    // product edge is what makes "this crate mints no provenance record and
    // holds no broker" a compile error rather than a source scan.
    "academic-student-voice": [
      "academic-consent",
      "academic-domain",
      "academic-model-run",
      "academic-retention",
    ],
    // `P2-N5`'s acceptance suite overlays real state, and a real overlay needs
    // an `EligibleEvidence` whose section 13.2 row is a node of a document
    // `P2-L4` produced. It reaches that through `P2-N2`'s own fixture module by
    // `#[path]`, the way `academic-freshness` does, so every workspace edge here
    // is one that module's two chains need. `serde_json` is `gap_case_round_trip`
    // driving section 15.1's schema through a real encoder.
    "academic-gap": [
      "academic-capture",
      "academic-consent",
      "academic-domain",
      "academic-lecture-document",
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-repository-analysis",
      "academic-repository-classification",
      "academic-repository-correlation",
      "academic-transcription",
      "academic-untrusted-content",
    ],
    // `P2-P1`. Six dev edges and not one of them a product edge, which is the
    // claim rather than an accident: the round trip needs a real committed
    // watermark, so the fixture creates a synthetic profile through
    // `academic-store`, seals artifacts through `academic-vault`, signs and
    // verifies batches through `academic-contracts`, and reads the canonical
    // rows back through `academic-portability` -- and the bundle is then
    // compared against those rows rather than against the value it was written
    // from. `academic-ingestion` and `academic-record` arrive with `P2-U3`'s
    // fixture module, which this crate's fixture includes by path rather than
    // transcribing. Keeping all six off the product edge is what makes "a
    // bundle is readable without this product" a graph fact.
    "academic-export": [
      "academic-contracts",
      "academic-ingestion",
      "academic-portability",
      "academic-record",
      "academic-store",
      "academic-vault",
    ],
    // `P2-N6`'s acceptance suite plans around a real `P2-N5` `GapCase`, and a
    // real one needs a real overlay over a real `EligibleEvidence` and a real
    // `FreshnessProjection`. It reaches that through `P2-N5`'s own fixture
    // module by `#[path]`, which reaches `P2-N2`'s the same way, so every
    // workspace edge here is one those two chains need.
    // `academic-knowledge-state` is a dev edge and **not** a product one: this
    // engine reads a decided `GapCase` and never a knowledge state, which is
    // what makes "taking a course changes no mastery" a compile-time fact
    // rather than a rule somebody remembers.
    "academic-critical-path": [
      "academic-capture",
      "academic-consent",
      "academic-domain",
      "academic-gap",
      "academic-knowledge-state",
      "academic-lecture-document",
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-repository-analysis",
      "academic-repository-classification",
      "academic-repository-correlation",
      "academic-transcription",
      "academic-untrusted-content",
    ],
  });

  assert.deepEqual(
    packagesWithProductEdgeTo(workspacePackages, "academic-test-support"),
    [],
    "product crates must not depend on the fault/test harness",
  );
  // The harness crate is reached by `#[path]` includes rather than by a Cargo
  // edge, so it must have no dependents of any kind at all.
  assert.equal(
    workspacePackages
      .filter((pkg) => pkg.name !== "academic-test-support")
      .some((pkg) => workspaceDependencyNames(pkg).includes("academic-test-support")),
    false,
    "nothing may depend on the fault/test harness crate",
  );
});

function defaultProductPackageNames() {
  const roots = workspacePackages
    .filter((pkg) => pkg.name !== "academic-test-support")
    .map((pkg) => pkg.id);
  const seen = new Set();
  const pending = [...roots];
  while (pending.length > 0) {
    const id = pending.pop();
    if (id === undefined || seen.has(id)) {
      continue;
    }
    seen.add(id);
    const node = resolveNodesById.get(id);
    if (node === undefined) {
      continue;
    }
    for (const dependency of node.deps) {
      const isProductEdge = dependency.dep_kinds.some((kind) => kind.kind !== "dev");
      if (isProductEdge) {
        pending.push(dependency.pkg);
      }
    }
  }
  return new Set([...seen].map((id) => packagesById.get(id).name));
}

async function rustSources(root) {
  const entries = await readdir(root, { withFileTypes: true });
  const sources = [];
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      sources.push(...(await rustSources(path)));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      sources.push([path, await readFile(path, "utf8")]);
    }
  }
  return sources;
}

/**
 * Names the canonical writer crates: the ones that own a handle able to change
 * accepted state.
 *
 * `academic-store` exposes the single acceptance writer, and
 * `academic-store-platform` is the private FFI leaf it opens files through.
 * Reaching either from a projection crate — by a normal, build, or dev edge —
 * would mean a projected value could be compiled into the same binary as the
 * write it must never reach.
 */
const CANONICAL_WRITER_CRATES = ["academic-store", "academic-store-platform"];

/**
 * Every workspace crate reachable from `root` through edges of any kind.
 *
 * Declared edges rather than resolved ones, and every kind rather than only the
 * shipping ones: a dev edge is still a compiled edge, and a projection crate
 * that dev-depended on the writer would be able to name it in a test.
 */
function workspaceClosureOfEveryKind(root, packages) {
  const byName = new Map(packages.map((pkg) => [pkg.name, pkg]));
  const names = new Set(packages.map((pkg) => pkg.name));
  const reached = new Set();
  const pending = [root];
  while (pending.length > 0) {
    const name = pending.pop();
    const pkg = byName.get(name);
    if (pkg === undefined) {
      continue;
    }
    for (const dependency of pkg.dependencies) {
      if (dependency.source !== null || !names.has(dependency.name)) {
        continue;
      }
      if (!reached.has(dependency.name)) {
        reached.add(dependency.name);
        pending.push(dependency.name);
      }
    }
  }
  return reached;
}

/**
 * Every package reachable from `rootId` in the resolved graph, across all edge
 * kinds, named rather than identified.
 *
 * The declared-edge walk above reads the manifests; this one reads what Cargo
 * actually resolved, so a writer that arrived through a renamed dependency or a
 * feature-activated optional edge is caught as well.
 */
function resolvedClosureNames(rootId, nodesById, packagesById) {
  const seen = new Set();
  const pending = [rootId];
  while (pending.length > 0) {
    const id = pending.pop();
    if (id === undefined || seen.has(id)) {
      continue;
    }
    seen.add(id);
    for (const dependency of nodesById.get(id)?.deps ?? []) {
      pending.push(dependency.pkg);
    }
  }
  return new Set([...seen].map((id) => packagesById.get(id)?.name));
}

test("scenario_crate_has_no_writer_dependency", () => {
  // Judged from the Cargo dependency graph, never from the source text. A
  // source grep would pass for a crate that linked the writer and simply did
  // not mention it yet, which is the state one edit away from a leak.
  const scenario = packagesByName.get("academic-scenario");
  assert.ok(scenario, "academic-scenario is not a workspace member");

  assert.deepEqual(
    productDependencyNames(scenario),
    ["academic-domain"],
    "the projection crate ships only the domain vocabulary",
  );

  const declared = workspaceClosureOfEveryKind("academic-scenario", workspacePackages);
  for (const writer of CANONICAL_WRITER_CRATES) {
    assert.equal(
      declared.has(writer),
      false,
      `academic-scenario reaches the canonical writer ${writer} through a declared edge`,
    );
  }

  const resolved = resolvedClosureNames(scenario.id, resolveNodesById, packagesById);
  for (const writer of CANONICAL_WRITER_CRATES) {
    assert.equal(
      resolved.has(writer),
      false,
      `academic-scenario reaches the canonical writer ${writer} in the resolved graph`,
    );
  }
  // The vault and the ledger are the writer's own seam and its append-only
  // rules. A projection crate has no business holding either.
  for (const canonical of ["academic-vault", "academic-ledger", "academic-portability"]) {
    assert.equal(
      declared.has(canonical),
      false,
      `academic-scenario reaches the canonical crate ${canonical}`,
    );
  }

  // The reverse edge would be worse than the forward one: it would let the
  // writer take a projected value as an argument.
  for (const writer of CANONICAL_WRITER_CRATES) {
    assert.equal(
      workspaceDependencyNames(packagesByName.get(writer)).includes("academic-scenario"),
      false,
      `${writer} depends on the projection crate`,
    );
  }
});

test("scenario_writer_gate_rejects_its_violation", () => {
  // The assertion above is a fact about the real graph. If the closure walk
  // were wrong, the real graph would still satisfy it and the test would still
  // pass, so the walk is put in front of graphs that do violate the invariant
  // and required to say so.
  const pkg = (name, dependencies = []) => ({
    id: `${name}-id`,
    name,
    dependencies: dependencies.map(([dependencyName, kind]) => ({
      name: dependencyName,
      kind,
      source: null,
      features: [],
    })),
    features: {},
  });

  const direct = [
    pkg("academic-scenario", [
      ["academic-domain", null],
      ["academic-store", null],
    ]),
    pkg("academic-store", []),
    pkg("academic-domain", []),
  ];
  assert.equal(
    workspaceClosureOfEveryKind("academic-scenario", direct).has("academic-store"),
    true,
    "a direct writer edge must be reported",
  );

  // A dev edge is still a compiled edge, so it must be reported too. This is
  // the case a shipping-graph-only check would miss.
  const devOnly = [
    pkg("academic-scenario", [
      ["academic-domain", null],
      ["academic-store", "dev"],
    ]),
    pkg("academic-store", []),
    pkg("academic-domain", []),
  ];
  assert.equal(
    workspaceClosureOfEveryKind("academic-scenario", devOnly).has("academic-store"),
    true,
    "a dev writer edge must be reported",
  );

  // And an edge that arrives two crates away, which is how a writer would
  // realistically reappear once other projection crates exist.
  const transitive = [
    pkg("academic-scenario", [["academic-projections", null]]),
    pkg("academic-projections", [["academic-store", null]]),
    pkg("academic-store", []),
  ];
  assert.equal(
    workspaceClosureOfEveryKind("academic-scenario", transitive).has("academic-store"),
    true,
    "a transitive writer edge must be reported",
  );

  const clean = [pkg("academic-scenario", [["academic-domain", null]]), pkg("academic-domain", [])];
  assert.equal(
    workspaceClosureOfEveryKind("academic-scenario", clean).has("academic-store"),
    false,
    "a clean graph must not be reported",
  );

  // The resolved walk is checked the same way, because it is the one that sees
  // a renamed or feature-activated edge.
  const resolvedNodes = new Map([
    ["academic-scenario-id", { deps: [{ pkg: "academic-projections-id" }] }],
    ["academic-projections-id", { deps: [{ pkg: "academic-store-id" }] }],
    ["academic-store-id", { deps: [] }],
  ]);
  const resolvedPackages = new Map(
    ["academic-scenario", "academic-projections", "academic-store"].map((name) => [
      `${name}-id`,
      { name },
    ]),
  );
  assert.equal(
    resolvedClosureNames("academic-scenario-id", resolvedNodes, resolvedPackages).has(
      "academic-store",
    ),
    true,
    "the resolved walk must report a writer two edges away",
  );
});

test("store_platform_native_unsafe_boundary_is_isolated", async () => {
  const [manifest, publicFacade, windowsSource, sources] = await Promise.all([
    readFile("crates/store-platform/Cargo.toml", "utf8"),
    readFile("crates/store-platform/src/lib.rs", "utf8"),
    readFile("crates/store-platform/src/windows.rs", "utf8"),
    rustSources("crates/store-platform/src"),
  ]);
  assert.match(manifest, /^unsafe_code = "deny"$/mu);
  assert.doesNotMatch(manifest, /^unsafe_code = "(?:allow|warn|forbid)"$/mu);
  for (const lint of [
    "missing_debug_implementations = \"deny\"",
    "unused_must_use = \"deny\"",
    "all = { level = \"deny\", priority = -1 }",
    "unwrap_used = \"deny\"",
    "expect_used = \"deny\"",
    "panic = \"deny\"",
  ]) {
    assert.ok(manifest.includes(lint), `store-platform omitted workspace deny: ${lint}`);
  }
  assert.doesNotMatch(manifest, /\[lints\]\s*\nworkspace\s*=\s*true/u);
  assert.doesNotMatch(publicFacade, /\bpub\s+unsafe\b|\b(?:RawHandle|HANDLE)\b/u);
  assert.doesNotMatch(windowsSource, /#!\[allow\(unsafe_code\)\]|\bunsafe\s+fn\b/u);

  const allowances = windowsSource.match(/#\[allow\(unsafe_code\)\]/gu) ?? [];
  const privateFunctionAllowances =
    windowsSource.match(/#\[allow\(unsafe_code\)\]\s*\n\s*fn\s+[a-z_][a-z0-9_]*\s*\(/gu) ?? [];
  assert.equal(
    privateFunctionAllowances.length,
    allowances.length,
    "unsafe_code allowance must be attached directly to a private function",
  );

  for (const [path, source] of sources) {
    assert.doesNotMatch(
      source,
      /\b(?:Command|Child|Stdio)::|std::process|powershell|cmd\.exe|\/bin\/(?:ba)?sh/u,
      `store-platform contains a shell/process probe in ${path}`,
    );
    for (const match of source.matchAll(/\bunsafe\s*\{/gu)) {
      assert.ok(path.endsWith(join("crates", "store-platform", "src", "windows.rs")));
      const prefix = source.slice(0, match.index);
      const functionMatches = [
        ...prefix.matchAll(/(?:^|\n)\s*fn\s+[a-z_][a-z0-9_]*\s*\([^)]*\)[^{]*\{/gsu),
      ];
      const currentFunction = functionMatches.at(-1);
      assert.ok(currentFunction, `unsafe block has no private function in ${path}`);
      const allowancePrefix = source.slice(
        Math.max(0, currentFunction.index - 80),
        currentFunction.index,
      );
      assert.match(allowancePrefix, /#\[allow\(unsafe_code\)\]/u);
      assert.match(source.slice(Math.max(0, match.index - 400), match.index), /\/\/ SAFETY:/u);
    }
  }
});

test("phase1_default_features_have_no_product_network", async () => {
  const productPackages = defaultProductPackageNames();
  const forbiddenExact = [
    "curl",
    "curl-sys",
    "h3",
    "http",
    "http-body",
    "hyper",
    "hyper-util",
    "native-tls",
    "quinn",
    "reqwest",
    "surf",
    "tonic",
    "tower-http",
    "ureq",
  ];
  for (const name of forbiddenExact) {
    assert.equal(productPackages.has(name), false, `default product graph contains ${name}`);
  }
  for (const name of productPackages) {
    assert.equal(/^aws-|^azure_|^gcp-/u.test(name), false, `default product graph contains ${name}`);
  }
  assert.equal(productPackages.has("openssl"), false, "default graph must not select OpenSSL");
  assert.equal(productPackages.has("openssl-sys"), false, "default graph must not select OpenSSL");

  const tokioPackage = packagesByName.get("tokio");
  const tokioNode = resolveNodesById.get(tokioPackage.id);
  for (const forbiddenFeature of ["fs", "full", "io-std", "io-uring", "process"]) {
    assert.equal(
      tokioNode.features.includes(forbiddenFeature),
      false,
      `tokio default graph selected ${forbiddenFeature}`,
    );
  }

  const productSourceRoots = workspacePackages
    .filter((pkg) => pkg.name !== "academic-test-support")
    .map((pkg) => join(dirname(pkg.manifest_path), "src"));
  const prohibitedBehavior =
    /\b(?:TcpListener|TcpSocket|TcpStream|ToSocketAddrs|UdpSocket|getaddrinfo|lookup_host)\b|(?:hyper|reqwest|tonic)::/u;
  for (const root of productSourceRoots) {
    for (const [path, source] of await rustSources(root)) {
      assert.doesNotMatch(source, prohibitedBehavior, `product network behavior in ${path}`);
    }
  }
});

// Placed here rather than in `tools/security-baseline.mjs`, which t068 section
// 2.3-14 names, because the two other feature-resolved graphs this repository
// asserts against -- the default one and the SQLCipher spike -- already live in
// this file with their own `cargo metadata` runs, and splitting the third one
// into another tool would put the same kind of claim in two places.
//
// The rule being enforced is 2.3-14's: an exception is enumerated **by crate**
// rather than granted globally. Cargo unifies features across the whole graph,
// so enabling `os-keystore` cannot be stopped from making a capability
// *available*; what can be pinned is exactly which capabilities appear and
// which crate is responsible for each. A capability that appears with no crate
// named for it fails here.
test("os_keystore_lane_expands_tokio_only_by_named_crate", () => {
  // Every tokio feature the `os-keystore` lane adds on top of the default
  // graph, and the crate that requires it. `zbus` declares its optional tokio
  // dependency with `features = [..., "fs", "process", "tracing", ...]` and
  // without `default-features = false`, which is where all four come from.
  // `zbus` itself enters only through `academic-keystore-platform`'s
  // non-default `secret-service` feature.
  const expectedAdditions = {
    default: "zbus (declares tokio without default-features = false)",
    fs: "zbus (tokio feature list, for its Unix-domain socket transport)",
    process: "zbus (tokio feature list)",
    tracing: "zbus (tokio feature list, its logging facade)",
  };

  const tokioFeaturesIn = (packagesByNameMap, nodesByIdMap) => {
    const tokioPackage = packagesByNameMap.get("tokio");
    assert.ok(tokioPackage, "tokio must be present in the resolved graph");
    const node = nodesByIdMap.get(tokioPackage.id);
    assert.ok(node, "tokio must have a resolve node");
    return new Set(node.features);
  };

  const defaultFeatures = tokioFeaturesIn(packagesByName, resolveNodesById);
  const laneFeatures = tokioFeaturesIn(
    osKeystorePackagesByName,
    osKeystoreResolveNodesById,
  );

  // The lane may only ever add. Losing a default-graph feature would mean the
  // two lanes are not the same build plus a broker.
  const removed = [...defaultFeatures].filter((feature) => !laneFeatures.has(feature));
  assert.deepEqual(removed, [], "the os-keystore lane dropped a default tokio feature");

  const added = [...laneFeatures].filter((feature) => !defaultFeatures.has(feature)).toSorted();
  assert.deepEqual(
    added,
    Object.keys(expectedAdditions).toSorted(),
    "the os-keystore lane changed which tokio features it adds; enumerate the new one with the crate that requires it",
  );

  // The crate named for each addition must actually be in the lane's graph.
  const laneNames = new Set(osKeystoreMetadata.packages.map((pkg) => pkg.name));
  for (const [feature, justification] of Object.entries(expectedAdditions)) {
    const crate = justification.split(" ")[0];
    assert.equal(
      laneNames.has(crate),
      true,
      `${feature} is justified by ${crate}, which is not in the os-keystore graph`,
    );
  }

  // The exception is scoped to this lane. The default graph keeps the Phase 1
  // posture that `phase1_default_features_have_no_product_network` asserts, and
  // this test does not restate or relax it.
  for (const forbiddenInDefault of ["fs", "full", "io-std", "io-uring", "process"]) {
    assert.equal(
      defaultFeatures.has(forbiddenInDefault),
      false,
      `the default graph gained ${forbiddenInDefault}`,
    );
  }

  // The lane still buys no network stack, whatever it does to tokio.
  for (const forbidden of ["full", "io-std", "io-uring"]) {
    assert.equal(
      laneFeatures.has(forbidden),
      false,
      `the os-keystore lane selected tokio/${forbidden}`,
    );
  }
  for (const forbidden of ["hyper", "reqwest", "tonic", "ureq", "curl", "native-tls", "openssl"]) {
    assert.equal(
      laneNames.has(forbidden),
      false,
      `the os-keystore graph contains ${forbidden}`,
    );
  }

  // Only one crate may pull `zbus` in, and only through its non-default feature.
  const zbusOwners = osKeystoreMetadata.packages
    .filter((pkg) => pkg.dependencies.some((dependency) => dependency.name === "zbus"))
    .map((pkg) => pkg.name)
    .toSorted();
  assert.deepEqual(zbusOwners, ["academic-keystore-platform"]);
  const leaf = osKeystorePackagesByName.get("academic-keystore-platform");
  assert.deepEqual(leaf.features.default, []);
  assert.deepEqual(leaf.features["secret-service"], ["dep:tokio", "dep:zbus"]);
});

// The other half of 2.3-14's "by crate" rule. The test above pins which
// capabilities the `os-keystore` lane makes *available*; this one proves they
// are not *used*. Cargo can only be stopped from granting a capability
// globally, so the executable claim has to be that no crate but the one that
// owns the broker reaches for it. Same shape as the `prohibitedBehavior` scan
// in `phase1_default_features_have_no_product_network`, applied to the
// filesystem and subprocess capability instead of the network one.
test("os_keystore_capabilities_are_available_but_unused", async () => {
  // `academic-keystore-platform` is exempt because it is the crate the
  // capability is admitted for. `academic-test-support` is excluded on the
  // existing convention that it is test-only and ships in no product build,
  // the same exclusion the network scan already makes.
  //
  // The walk root is the package directory, not `<crate>/src`. `T146` measured
  // the difference: `std::process::Command` in
  // `crates/record/examples/emit_harness.rs` -- no feature gate, compiled by
  // `cargo clippy --workspace --all-targets`, run by the documented
  // `pnpm harness:emit` script -- passed this scan, and
  // `crates/worker/probes/worker_probe.rs` had been spawning a subprocess
  // unseen since `P2-G4`. Widening it brings the eight files outside `src` that
  // already spell `process::Command` into the allowlist below, each with the
  // reason it is allowed, which is the review those eight had never had.
  const scanned = workspacePackages
    .filter(
      (pkg) =>
        pkg.name !== "academic-test-support" && pkg.name !== "academic-keystore-platform",
    )
    .map((pkg) => [pkg.name, dirname(pkg.manifest_path)]);
  assert.ok(scanned.length >= 10, "the capability scan covers too few crates to be meaningful");

  // Matches `tokio::fs`, `tokio::process`, and the grouped `use tokio::{fs, ..}`
  // spelling, which a bare path regex would miss.
  const directTokioCapability = /\btokio::(?:fs|process)\b/u;
  const groupedTokioImport = /\buse\s+tokio::\{(?<items>[^}]*)\}/gsu;
  const groupedCapability = /(?:^|[\s,{])(?:fs|process)(?:[\s,:}]|$)/u;

  // `std::process::Command` predates this task in two files. Each is enumerated
  // with its reason rather than the rule being dropped; every other file fails.
  const commandAllowlist = new Map([
    [
      join("crates", "cli", "src", "commands", "doctor.rs"),
      "product: `doctor` observes an external tool's own `--version` output. It " +
        "starts no daemon, opens no socket, and reads no file through the child.",
    ],
    [
      join("crates", "core", "src", "service.rs"),
      "test-only: inside `#[cfg(test)] mod tests`, spawning the IPC02 fault " +
        "child. Ships in no product build.",
    ],
    [
      join("crates", "worker", "src", "sandbox", "linux.rs"),
      "product, and the point of the crate: the P2-G4 parent launches the " +
        "sandboxed worker. It is behind the non-default `native-sandbox` " +
        "feature, the child it starts is the process the sandbox contains, and " +
        "the child itself is refused `clone`, `fork`, `vfork` and `execve` by " +
        "the seccomp filter this file installs.",
    ],
    // The eight below are the files outside `src` that the `<crate>/src` walk
    // never read. Seven are test targets, which ship in no product build; one
    // is the sandbox probe, which is a `[[bin]]` with `required-features` and
    // is reached by no crate. None of them is new -- what is new is that each
    // now carries the reason it is allowed instead of being invisible.
    [
      join("crates", "worker", "probes", "worker_probe.rs"),
      "test-only, and the process P2-G4's sandbox contains: proving the " +
        "sandbox refuses process creation means asking for it. It is a " +
        "`[[bin]]` with `required-features = [\"native-sandbox\"]` and a `path` " +
        "outside `src`, and no workspace crate depends on `academic-worker`, " +
        "both of which `only_egress_crate_has_a_socket` reads from " +
        "`cargo metadata` rather than taking on trust.",
    ],
    [
      join("crates", "worker", "tests", "containment.rs"),
      "test-only: the P2-G4 acceptance suite launches the probe binary above, " +
        "once outside the sandbox as its baseline and once inside it as the " +
        "claim. Ships in no product build.",
    ],
    [
      join("crates", "capture-gate", "src", "native", "linux.rs"),
      "behind the non-default `native-capture` feature: `P2-L1`'s Linux " +
        "device layer launches the capture process with the Landlock ruleset " +
        "installed between `fork` and `exec`, so the process that opens a " +
        "device is never the one that decided what it may open. It ships in " +
        "no default build -- `default = []` and the feature is the only way " +
        "in -- and the probe it launches is a `[[bin]]` with " +
        "`required-features` and a `path` outside `src`.",
    ],
    [
      join("crates", "capture-gate", "src", "native", "windows.rs"),
      "the same, for the Windows AppContainer, which is applied by " +
        "`CreateProcessW` in the parent. The uncontained arm uses " +
        "`process::Command` for the paired permission run, which is what " +
        "makes a refusal inside the container evidence.",
    ],
    [
      join("crates", "core", "tests", "projection_generation.rs"),
      "test-only: re-runs this repository's own generator so a committed " +
        "projection can be compared against a fresh one. Ships in no product " +
        "build.",
    ],
    [
      join("crates", "crypto", "tests", "key_faults.rs"),
      "test-only, and behind the non-default `phase2-fault-injection` " +
        "feature: spawns a child process to observe a fault that kills one. " +
        "Ships in no product build.",
    ],
    [
      join("crates", "daemon", "tests", "phase1_exit.rs"),
      "test-only: builds the default-feature `academicd` binary in its own " +
        "target directory so the link half of " +
        "`phase1_exit_has_no_product_network` scans an image this repository " +
        "produced. Behind the non-default `phase1-fault-injection` feature.",
    ],
    [
      join("crates", "store", "tests", "acceptance.rs"),
      "test-only: builds the store binary in a separate lane to read what the " +
        "default feature set links. Ships in no product build.",
    ],
    [
      join("crates", "store", "tests", "encrypted_profile.rs"),
      "test-only: the same separate build, asserting that a default binary " +
        "links no SQLCipher. Compiled only when `sqlcipher-store` is off. " +
        "Ships in no product build.",
    ],
    [
      join("crates", "store", "tests", "sqlcipher_spike.rs"),
      "test-only: the native-toolchain spike lane, which builds and inspects " +
        "its own binary. Ships in no product build.",
    ],
  ]);
  const commandUse = /\bprocess::Command\b/u;
  const seenCommandFiles = new Set();

  for (const [crateName, root] of scanned) {
    for (const [path, source] of await rustSources(root)) {
      assert.doesNotMatch(
        source,
        directTokioCapability,
        `${crateName} reaches for a tokio filesystem or subprocess capability in ${path}`,
      );
      for (const grouped of source.matchAll(groupedTokioImport)) {
        assert.doesNotMatch(
          grouped.groups.items,
          groupedCapability,
          `${crateName} imports a tokio filesystem or subprocess capability in ${path}`,
        );
      }

      if (!commandUse.test(source)) {
        continue;
      }
      const relative = path.slice(path.indexOf("crates"));
      assert.equal(
        commandAllowlist.has(relative),
        true,
        `${relative} spawns a subprocess and is not one of the reviewed sites`,
      );
      seenCommandFiles.add(relative);
    }
  }

  // An allowlist entry that no longer matches anything is a stale exception.
  assert.deepEqual(
    [...seenCommandFiles].toSorted(),
    [...commandAllowlist.keys()].toSorted(),
    "a subprocess allowlist entry no longer applies and must be removed",
  );

  // The test-only entry must stay test-only: every `process::Command` in that
  // file has to sit after the `#[cfg(test)]` marker, or the exception has
  // quietly become a product one.
  const serviceRelative = join("crates", "core", "src", "service.rs");
  const serviceSource = await readFile(serviceRelative, "utf8");
  const testModuleAt = serviceSource.indexOf("#[cfg(test)]");
  assert.ok(testModuleAt > 0, `${serviceRelative} no longer has a #[cfg(test)] module`);
  for (const match of serviceSource.matchAll(/\bprocess::Command\b/gu)) {
    assert.ok(
      match.index > testModuleAt,
      `${serviceRelative} uses process::Command outside its #[cfg(test)] module`,
    );
  }
});

const PROCESS_BOUNDARIES = new Map([
  ["academic-capture-client", ["capture-client", "CaptureClient"]],
  ["academic-indexer", ["indexer", "Indexer"]],
  ["academic-repository-analyzer", ["repository-analyzer", "RepositoryAnalyzer"]],
  ["academic-connector", ["connector", "Connector"]],
  ["academic-egress", ["egress", "EgressProxy"]],
  ["academic-export-job", ["export-job", "ExportJob"]],
]);

const PROCESS_POLICY_CLOSURE = [
  "academic-policy",
  "bitflags",
  "block-buffer",
  "cc",
  "cfg-if",
  "cpufeatures",
  "crypto-common",
  "digest",
  "fallible-iterator",
  "fallible-streaming-iterator",
  "find-msvc-tools",
  "generic-array",
  "libc",
  "libsqlite3-sys",
  "pkg-config",
  "proc-macro2",
  "quote",
  "rusqlite",
  "sha2",
  "shlex",
  "smallvec",
  "subtle",
  "syn",
  "thiserror",
  "thiserror-impl",
  "typenum",
  "unicode-ident",
  "vcpkg",
  "version_check",
];

function resolvedShippingPackageNames(rootName) {
  const root = packagesByName.get(rootName);
  assert.ok(root, `${rootName} is not a workspace package`);
  const visited = new Set();
  const pending = [root.id];
  while (pending.length > 0) {
    const id = pending.shift();
    if (visited.has(id)) {
      continue;
    }
    visited.add(id);
    const node = resolveNodesById.get(id);
    assert.ok(node, `${id} has no resolved node`);
    for (const dependency of node.deps) {
      const ships = dependency.dep_kinds.some(({ kind }) => kind !== "dev");
      if (ships) {
        pending.push(dependency.pkg);
      }
    }
  }
  return [...visited].map((id) => packagesById.get(id).name).toSorted();
}

function expectedProcessManifest(packageName) {
  return `[package]
name = "${packageName}"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish.workspace = true

[dependencies]
academic-policy = { path = "../policy" }

[lints]
workspace = true
`;
}

function expectedProcessSource(processClass) {
  return `use academic_policy::ProcessClass;

const PROCESS_CLASS: ProcessClass = ProcessClass::${processClass};

fn main() {
    let _capability_set = PROCESS_CLASS.capabilities();
}
`;
}

async function assertExactProcessBoundary(packageName) {
  const [directory, processClass] = PROCESS_BOUNDARIES.get(packageName);
  const pkg = packagesByName.get(packageName);
  assert.ok(pkg, `${packageName} is absent`);
  assert.deepEqual(productDependencyNames(pkg), ["academic-policy"]);
  assert.deepEqual(devDependencyNames(pkg), []);
  assert.deepEqual(Object.keys(pkg.features), []);
  assert.deepEqual(
    pkg.targets.map((target) => ({
      name: target.name,
      kind: target.kind,
      crate_types: target.crate_types,
    })),
    [{ name: packageName, kind: ["bin"], crate_types: ["bin"] }],
    `${packageName} gained another executable, library, example, or build script`,
  );
  assert.equal(
    (await readFile(join("crates", directory, "Cargo.toml"), "utf8")).replaceAll("\r\n", "\n"),
    expectedProcessManifest(packageName),
    `${packageName}'s whole manifest changed; review the complete process boundary`,
  );
  const sources = await rustSources(join("crates", directory, "src"));
  assert.deepEqual(
    sources.map(([path, source]) => [path.split("\\").join("/"), source.replaceAll("\r\n", "\n")]),
    [[join("crates", directory, "src", "main.rs").split("\\").join("/"), expectedProcessSource(processClass)]],
    `${packageName}'s complete product source is no longer its one fixed process-class binding`,
  );
}

test("six_process_entrypoints_are_exact_and_distinct", async () => {
  for (const packageName of PROCESS_BOUNDARIES.keys()) {
    await assertExactProcessBoundary(packageName);
  }
});

test("indexer_cannot_open_a_socket", async () => {
  await assertExactProcessBoundary("academic-indexer");
  assert.deepEqual(
    resolvedShippingPackageNames("academic-indexer"),
    ["academic-indexer", ...PROCESS_POLICY_CLOSURE].toSorted(),
    "the indexer feature graph changed; the entire new closure must be reviewed for socket capability",
  );
});

test("export_job_cannot_read_keys", async () => {
  await assertExactProcessBoundary("academic-export-job");
  assert.deepEqual(
    resolvedShippingPackageNames("academic-export-job"),
    ["academic-export-job", ...PROCESS_POLICY_CLOSURE].toSorted(),
    "the export-job feature graph changed; the entire new closure must be reviewed for key access",
  );
});

// `P2-X1`. The desktop surface's blocking boundary, judged the way
// `only_egress_crate_has_a_socket` judges the socket one: from the declared
// dependency graph, from the resolved link closure, and from the source text,
// because each of the three is blind to a different way of acquiring the
// capability. A declared-edge check misses an optional dependency a feature
// turns on; a resolved-closure check misses a crate that links the capability
// and does not use it yet; and a source scan misses everything that spells no
// forbidden name, which is the whole of "add a dependency".
//
// ADR-001's surface table is what is being enforced: the desktop must not
// "open DB directly, hold provider/root keys, unrestricted filesystem/network".

/** The desktop package directory, walked whole rather than by `src`. */
const DESKTOP_PACKAGE = "crates/desktop";

/**
 * Workspace crates the desktop must not reach by any edge of any kind.
 *
 * The first of them own the canonical database or a key: the store and its FFI
 * leaf, the embedded policy database, the object vault, and the key hierarchy.
 * The rest hold key material of their own -- the OS keystore binding, the
 * backup key, and the rotation engine -- or open the store on the desktop's
 * behalf, which is the same authority one call away.
 */
const DESKTOP_FORBIDDEN_WORKSPACE_CRATES = [
  "academic-core",
  "academic-crypto",
  "academic-keystore-platform",
  "academic-policy",
  "academic-projections",
  "academic-recovery",
  "academic-retention",
  "academic-store",
  "academic-store-platform",
  "academic-vault",
];

/** External crates that can open a database. */
const DATABASE_CAPABLE_CRATES = [
  "diesel",
  "duckdb",
  "heed",
  "libsqlite3-sys",
  "native_db",
  "redb",
  "rocksdb",
  "rusqlite",
  "sea-orm",
  "sled",
  "sqlite",
  "sqlite3-src",
  "sqlx",
];

/**
 * External crates that derive, wrap, store or unwrap key material.
 *
 * `ed25519-dalek`, `sha2`, `hmac` and `zeroize` are deliberately absent: they
 * are in the desktop's closure, through `academic-admission`'s receipt
 * signature verification and `academic-domain`'s digests, and verifying a
 * signature over public evidence is not holding a key. What is refused here is
 * custody -- a KDF, an AEAD, a keyring, or a TLS stack that would carry one.
 */
const KEY_CUSTODY_CRATES = [
  "aead",
  "aes-gcm",
  "argon2",
  "chacha20",
  "chacha20poly1305",
  "hkdf",
  "keyring",
  "openssl",
  "p256",
  "pbkdf2",
  "poly1305",
  "ring",
  "rustls",
  "scrypt",
  "secret-service",
  "security-framework",
  "zbus",
];

/**
 * The desktop's whole resolved shipping closure.
 *
 * Pinned entire, as `PROCESS_POLICY_CLOSURE` is, so that a dependency added
 * anywhere below the surface is a review of the whole new closure rather than a
 * silent widening that the two capability tables above happen not to name.
 * Duplicated names are two major versions of one crate and are kept, because
 * the comparison is against `resolvedShippingPackageNames`'s own output.
 */
const DESKTOP_SHIPPING_CLOSURE = [
  "academic-admission",
  "academic-contracts",
  "academic-desktop",
  "academic-domain",
  "academic-rpc",
  "aho-corasick",
  "anyhow",
  "base64ct",
  "bitflags",
  "block-buffer",
  "bumpalo",
  "bytes",
  "cfg-if",
  "ciborium",
  "ciborium-io",
  "ciborium-ll",
  "const-oid",
  "cpufeatures",
  "crunchy",
  "crypto-common",
  "curve25519-dalek",
  "curve25519-dalek-derive",
  "der",
  "digest",
  "ed25519",
  "ed25519-dalek",
  "either",
  "equivalent",
  "errno",
  "fastrand",
  "fiat-crypto",
  "fixedbitset",
  "futures-core",
  "futures-task",
  "futures-util",
  "generic-array",
  "getrandom",
  "getrandom",
  "half",
  "hashbrown",
  "heck",
  "hex",
  "hmac",
  "indexmap",
  "itertools",
  "itoa",
  "js-sys",
  "libc",
  "linux-raw-sys",
  "log",
  "memchr",
  "mio",
  "multimap",
  "once_cell",
  "petgraph",
  "pin-project-lite",
  "pkcs8",
  "proc-macro2",
  "prost",
  "prost-build",
  "prost-derive",
  "prost-types",
  "protoc-bin-vendored",
  "protoc-bin-vendored-linux-aarch_64",
  "protoc-bin-vendored-linux-ppcle_64",
  "protoc-bin-vendored-linux-s390_64",
  "protoc-bin-vendored-linux-x86_32",
  "protoc-bin-vendored-linux-x86_64",
  "protoc-bin-vendored-macos-aarch_64",
  "protoc-bin-vendored-macos-x86_64",
  "protoc-bin-vendored-win32",
  "quote",
  "r-efi",
  "rand_core",
  "regex",
  "regex-automata",
  "regex-syntax",
  "rustc_version",
  "rustix",
  "rustversion",
  "semver",
  "serde",
  "serde_core",
  "serde_derive",
  "serde_json",
  "sha2",
  "signal-hook-registry",
  "signature",
  "slab",
  "socket2",
  "spki",
  "subtle",
  "syn",
  "syn",
  "tempfile",
  "thiserror",
  "thiserror-impl",
  "tokio",
  "tokio-macros",
  "typenum",
  "unicode-ident",
  "uuid",
  "version_check",
  "wasi",
  "wasm-bindgen",
  "wasm-bindgen-macro",
  "wasm-bindgen-macro-support",
  "wasm-bindgen-shared",
  "windows-link",
  "windows-sys",
  "zerocopy",
  "zerocopy-derive",
  "zeroize",
  "zeroize_derive",
  "zmij",
];

/**
 * Every identifier the desktop's source may write a `::` after.
 *
 * A closed world over path roots rather than a list of forbidden names. A
 * fully qualified `rusqlite::Connection::open` writes no `use`, so a `use`-root
 * allowlist would not see it; this does, because `rusqlite` is not on the list.
 * Local types and modules are here for the same reason -- the rule is "every
 * root was reviewed", and a rule with exceptions is not that rule.
 */
const DESKTOP_PATH_ROOTS = [
  "Borrow",
  "Command",
  "DesktopCommand",
  "NotCanonical",
  "Optimistic",
  "Self",
  "SyntheticFixtureId",
  "TestCases",
  "academic_desktop",
  "academic_rpc",
  "borrow",
  "bytes",
  "collections",
  "command",
  "core",
  "fmt",
  "generated",
  "mutable_request",
  "optimistic",
  "serde_json",
  "std",
  "str",
  "thiserror",
  "trybuild",
  "u64",
];

/**
 * A floor under the walk.
 *
 * `S-12` in `docs/contracts/policy-source-scans.md` is the walk that reads
 * fewer files than it thinks. A walk that returned nothing would satisfy every
 * "no file contains X" assertion below.
 */
const DESKTOP_SOURCE_FLOOR = 9;

test("desktop_cannot_open_the_database_or_read_keys", async () => {
  const desktop = packagesByName.get("academic-desktop");
  assert.ok(desktop, "academic-desktop is not a workspace package");

  // The graph half, from declared edges of every kind. A dev edge is still a
  // compiled edge: a desktop that dev-depended on the store could name it in a
  // test, and a test is a place a key can be printed.
  assert.deepEqual(
    [...workspaceClosureOfEveryKind("academic-desktop", workspacePackages)].toSorted(),
    ["academic-admission", "academic-contracts", "academic-domain", "academic-rpc"],
    "the desktop's declared workspace closure changed; review the whole new closure",
  );

  // The same half from the resolved graph, which sees a renamed dependency and
  // a feature-activated optional edge that the declared walk above does not.
  const resolved = resolvedClosureNames(desktop.id, resolveNodesById, packagesById);
  for (const forbidden of DESKTOP_FORBIDDEN_WORKSPACE_CRATES) {
    assert.equal(
      resolved.has(forbidden),
      false,
      `the desktop reaches ${forbidden}, which owns the database or a key`,
    );
  }

  // The link half. The capability tables say which crates could open a store or
  // hold key material; the closure pin says nothing else arrived either.
  for (const forbidden of [...DATABASE_CAPABLE_CRATES, ...KEY_CUSTODY_CRATES]) {
    assert.equal(resolved.has(forbidden), false, `the desktop links ${forbidden}`);
  }
  assert.deepEqual(
    resolvedShippingPackageNames("academic-desktop"),
    DESKTOP_SHIPPING_CLOSURE.toSorted(),
    "the desktop feature graph changed; the entire new closure must be reviewed for database and key access",
  );

  // The source half, over the whole package rather than `src`.
  const sources = await rustSources(DESKTOP_PACKAGE);
  assert.ok(
    sources.length >= DESKTOP_SOURCE_FLOOR,
    `the desktop walk read ${sources.length} files, below its floor of ${DESKTOP_SOURCE_FLOOR}`,
  );

  const walked = new Set(sources.map(([path]) => resolve(path)));
  const roots = new Set();
  for (const [path, source] of sources) {
    const code = rustCodeOnly(source);

    // Every path root is one that was reviewed. A fully qualified call spells
    // no `use`, so this reads the roots rather than the imports.
    for (const [, root] of code.matchAll(/\b([A-Za-z_][A-Za-z0-9_]*)\s*::/gu)) {
      roots.add(root);
      assert.ok(
        DESKTOP_PATH_ROOTS.includes(root),
        `${path} names the unreviewed path root ${root}`,
      );
    }
    for (const [, root] of code.matchAll(/\bextern\s+crate\s+([A-Za-z_][A-Za-z0-9_]*)/gu)) {
      assert.ok(DESKTOP_PATH_ROOTS.includes(root), `${path} links the unreviewed crate ${root}`);
    }

    // No foreign function, no environment, no process, no unsafe block. Each is
    // a way to reach a file, a key or a database without naming a crate.
    assert.doesNotMatch(code, /\bextern\s+"/u, `${path} declares a foreign function`);
    assert.doesNotMatch(code, /\bunsafe\b/u, `${path} contains an unsafe block`);
    assert.doesNotMatch(
      code,
      /\b(?:env|option_env|include_bytes|include_str|include)\s*!/u,
      `${path} reads the environment or embeds a file`,
    );

    // The tripwire: every module this file pulls in is a file the walk read.
    for (const [, target] of source.matchAll(/#\[\s*path\s*=\s*"([^"]*)"\s*\]/gu)) {
      assert.ok(
        walked.has(resolve(dirname(path), target)),
        `${path} pulls in ${target}, which this walk did not read`,
      );
    }
    for (const [, name] of code.matchAll(/^\s*(?:pub\s+)?mod\s+([a-z_][a-z0-9_]*)\s*;/gmu)) {
      const directory = dirname(path);
      const flat = resolve(directory, `${name}.rs`);
      const nested = resolve(directory, name, "mod.rs");
      assert.ok(
        walked.has(flat) || walked.has(nested),
        `${path} declares module ${name}, which this walk did not read`,
      );
    }
  }

  // The root allowlist has no dead entry, so it cannot quietly accumulate a
  // permission for something the crate stopped doing.
  assert.deepEqual(
    DESKTOP_PATH_ROOTS.filter((root) => !roots.has(root)),
    [],
    "the desktop path-root allowlist names roots the crate no longer writes",
  );
});

// `P2-X1`. The desktop names one synthetic fixture and `academic-core` defines
// which fixture that is. The comparison is a source scan because the desktop
// must not have a dependency edge to `academic-core`, which opens the store:
// the two constants can only be compared as text.
test("desktop_names_only_the_core_fixture_allowlist", async () => {
  const coreSource = await readFile("crates/core/src/local_service.rs", "utf8");
  const coreMatch = /pub const PHASE1_SYNTHETIC_FIXTURE_ID: &str = "([^"]+)";/u.exec(coreSource);
  assert.ok(coreMatch, "academic-core no longer defines PHASE1_SYNTHETIC_FIXTURE_ID as one literal");

  const desktopSource = rustCodeOnly(
    await readFile("crates/desktop/src/command.rs", "utf8"),
    true,
  );
  // The `as_str` arms of `SyntheticFixtureId` and nothing else. Reading the
  // whole file would also collect `DesktopCommand::capability_id`'s arms, which
  // are capability identifiers rather than fixtures.
  const implStart = desktopSource.indexOf("impl SyntheticFixtureId {");
  assert.ok(implStart >= 0, "academic-desktop no longer has an `impl SyntheticFixtureId` block");
  const implEnd = desktopSource.indexOf("\n}", implStart);
  assert.ok(implEnd > implStart, "the `impl SyntheticFixtureId` block is not closed at column zero");
  const desktopFixtures = [
    ...desktopSource
      .slice(implStart, implEnd)
      .matchAll(/Self::[A-Za-z0-9_]+\s*=>\s*"([^"]+)"/gu),
  ].map(([, id]) => id);
  assert.deepEqual(
    desktopFixtures.toSorted(),
    [coreMatch[1]],
    "the desktop's fixture allowlist and academic-core's fixture identifier have diverged",
  );
});

// The P2-G2 half of 2.3-14. `os_keystore_lane_expands_tokio_only_by_named_crate`
// pins which capabilities a lane makes *available* and
// `os_keystore_capabilities_are_available_but_unused` pins whether they are
// used. Tokio's `net` feature is already resolved in the default graph, for the
// named pipe and Unix-domain socket the daemon runs on, so availability is not
// what can be refused here — use is.
//
// The `P2-K6` audit put five key substitutions past a guard that read one file
// against a token list, and the shape that answered it in
// `crates/cli/src/main.rs` is what this follows: read every file, and pin what
// is there rather than forbid a list of names. A per-file allowance of exact
// spellings refuses the three shapes a token list cannot see —
// `use tokio::net as t`, a re-export of the module from a permitted file, and a
// foreign function declaration that spells no Rust socket name at all — because
// each of them is a spelling that is not in the allowance, or a structural rule
// below that has nothing to do with names.

/**
 * Removes comments and, unless `keepStrings`, string and character literals.
 *
 * Prose must neither trip the scan nor hide from it. The socket scan reads the
 * form with literals removed; the `#[path]` scan needs its target, so it reads
 * the form that keeps them.
 */
function rustCodeOnly(source, keepStrings = false) {
  let out = "";
  let cursor = 0;
  while (cursor < source.length) {
    const two = source.slice(cursor, cursor + 2);
    if (two === "//") {
      const newline = source.indexOf("\n", cursor);
      cursor = newline === -1 ? source.length : newline;
      continue;
    }
    if (two === "/*") {
      const close = source.indexOf("*/", cursor + 2);
      cursor = close === -1 ? source.length : close + 2;
      out += " ";
      continue;
    }
    const character = source[cursor];

    // A raw string. Without this arm the quote count goes odd at the first
    // `r#"..."#` holding a quote, and from there every literal in the file is
    // read as code and every stretch of code as a literal -- so a socket
    // spelling after one is invisible here. Three files in this repository
    // contain raw strings, two of them from before `P2-G4`.
    if (character === "r" && !keepStrings) {
      let hashes = 0;
      while (source[cursor + 1 + hashes] === "#") {
        hashes += 1;
      }
      if (source[cursor + 1 + hashes] === '"') {
        const closing = '"' + "#".repeat(hashes);
        const at = source.indexOf(closing, cursor + 2 + hashes);
        cursor = at === -1 ? source.length : at + closing.length;
        out += '""';
        continue;
      }
    }
    if (character === '"' && !keepStrings) {
      let end = cursor + 1;
      while (end < source.length) {
        if (source[end] === "\\") {
          end += 2;
          continue;
        }
        if (source[end] === '"') {
          end += 1;
          break;
        }
        end += 1;
      }
      cursor = end;
      out += '""';
      continue;
    }
    if (character === "'" && !keepStrings) {
      const literal = /^'(?:\\.|[^'\\])'/u.exec(source.slice(cursor));
      if (literal !== null) {
        cursor += literal[0].length;
        out += "''";
        continue;
      }
    }
    out += character;
    cursor += 1;
  }
  return out;
}

/**
 * Every spelling that reaches a socket, outbound or local.
 *
 * Whitespace around `::` is normalized out of the recorded spelling, so
 * `tokio :: net` and `tokio::net` are one entry and neither can hide from the
 * allowance by being formatted differently.
 */
const SOCKET_SPELLINGS = [
  /\b(?:std|core)\s*::\s*net\b/gu,
  /\btokio\s*::\s*net\b/gu,
  /\brustix\s*::\s*net\b/gu,
  /\bmio\s*::\s*net\b/gu,
  /\bnix\s*::\s*sys\s*::\s*socket\b/gu,
  /\bsocket2\b/gu,
  /\b(?:TcpStream|TcpListener|TcpSocket|UdpSocket|ToSocketAddrs)\b/gu,
  /\b(?:SocketAddr|SocketAddrV4|SocketAddrV6|IpAddr|Ipv4Addr|Ipv6Addr)\b/gu,
  /\b(?:lookup_host|getaddrinfo|connect_timeout)\b/gu,
  /\bWinSock\b/gu,
  /\bWSA[A-Za-z0-9_]*\b/gu,
  /\blibc\s*::\s*(?:socket|connect|bind|listen|sendto|recvfrom|getaddrinfo)\b/gu,
  /\b(?:UnixStream|UnixListener|UnixDatagram)\b/gu,
  // A socket reached by number rather than by name. `libc::syscall(SYS_socket,
  // ...)` opens one and spells nothing else on this list; `P2-G4` used exactly
  // that shape as an injection and it passed every rule here before these two
  // patterns existed. A bare numeric `libc::syscall(41, ...)` is refused by the
  // first-argument rule below, and a call that avoids the path spelling by
  // importing the function is refused by `LIBC_SYSCALL_IMPORTS` above -- that
  // second half is what `S-11` in `docs/contracts/policy-source-scans.md`
  // recorded as open until `P2-RF11`. The link half below is what bounds who
  // can reach `libc` at all.
  /\blibc\s*::\s*syscall\b/gu,
  /\bSYS_(?:socket|socketpair|socketcall|connect|bind|listen|accept4?|sendto|recvfrom|sendmsg|recvmsg)\b/gu,
  /\bNamedPipe[A-Za-z]*\b/gu,
  /\bnamed_pipe\b/gu,
];

/**
 * The local same-host transports 2.3-14 admits. Everything else is outbound.
 *
 * `tokio::net` is on this list because the module is where the named pipe and
 * the Unix-domain socket live. What keeps that from being a hole is that a
 * file's whole spelling set is pinned: a file allowed `tokio::net` is not
 * allowed `TcpStream`, and reaching one through the other spells it.
 */
const LOCAL_IPC_SPELLINGS = new Set([
  "tokio::net",
  "UnixStream",
  "UnixListener",
  "NamedPipe",
  "NamedPipeServer",
  "NamedPipeClient",
  "named_pipe",
]);

/**
 * Every file that may spell a socket, and exactly which spellings.
 *
 * The five daemon and client files run the section 3.6 local IPC seam.
 * `academic-egress` is the crate the section 3.6 topology allows an outbound
 * socket in -- it is the egress-proxy process, and `P2-G7`'s `ProcessClass`
 * matrix gives only that class the `OpenOutboundSocket` capability. Its
 * allowance is empty: none ships. ADR-002 is unaccepted, the admission receipt
 * is incomplete, and `product_network` is `NONE`, so there is nothing for a
 * socket to legitimately connect to yet. `academic-egress-boundary`, which
 * stages and previews what that process may send, is empty for the same reason
 * and stays empty: the seam it owns is a trait the caller supplies. The day a
 * socket is written, this table changes in the same commit, which is the
 * review.
 */
const SOCKET_ALLOWANCE = new Map([
  ["crates/cli/src/client.rs", ["NamedPipe", "UnixStream", "named_pipe", "tokio::net"]],
  ["crates/daemon/src/transport/mod.rs", ["NamedPipe"]],
  ["crates/daemon/src/transport/unix.rs", ["NamedPipe", "UnixListener", "UnixStream", "tokio::net"]],
  [
    "crates/daemon/src/transport/windows.rs",
    ["NamedPipe", "NamedPipeServer", "named_pipe", "tokio::net"],
  ],
  ["crates/daemon/tests/phase1_exit.rs", ["NamedPipe", "UnixStream", "named_pipe", "tokio::net"]],
  ["crates/daemon/tests/support/mod.rs", ["NamedPipe", "UnixStream", "named_pipe", "tokio::net"]],
  ["crates/daemon/tests/unix_socket.rs", ["NamedPipe"]],
  ["crates/daemon/tests/windows_pipe.rs", ["NamedPipe", "named_pipe", "tokio::net"]],
  // `P2-G4`. These two are the first files in this repository allowed an
  // outbound socket spelling, and they are allowed opposite halves of one.
  //
  // The probe is the process the sandbox contains. Proving that the operating
  // system refuses a socket means asking it for one, so this file asks: it
  // binds a loopback listener as its own positive control and connects to an
  // RFC 5737 documentation address as the claim. It is a `[[bin]]` with
  // `required-features = ["native-sandbox"]` and a `path` outside `src`, so no
  // default build and no product crate reaches it -- the two rules below check
  // both, and `probe_targets_are_not_in_any_default_build` in
  // `crates/worker/tests/capability.rs` reads the manifest for the same thing.
  //
  // The Linux backend names the socket *syscalls*, and it names them to put
  // them in a seccomp deny list. The rule below is what makes that structural
  // rather than a promise: every `SYS_` spelling in that file has to appear
  // inside its `denied_syscalls` function.
  [
    "crates/worker/probes/worker_probe.rs",
    [
      "Ipv4Addr",
      "SocketAddr",
      "SocketAddrV4",
      "TcpListener",
      "TcpStream",
      "connect_timeout",
    ],
  ],
  // `P2-L1`. The capture gate's Linux device layer reaches Landlock the same
  // way and for the same reason: there is no libc wrapper for it. It names no
  // socket syscall at all -- the three it makes are the Landlock trio -- so its
  // allowance is `libc::syscall` and nothing else, and `RAW_SYSCALL_FILES`
  // below carries the reviewed set that the first-argument rule reads it
  // against. A fourth name, or a bare number, fails that rule here and
  // `the_linux_backend_names_only_the_three_syscalls_it_installs` in
  // `crates/capture-gate/tests/capture_scans.rs` independently.
  ["crates/capture-gate/src/native/linux.rs", ["libc::syscall"]],
  [
    "crates/worker/src/sandbox/linux.rs",
    [
      "SYS_accept4",
      "SYS_bind",
      "SYS_connect",
      "SYS_listen",
      "SYS_recvfrom",
      "SYS_recvmsg",
      "SYS_sendmsg",
      "SYS_sendto",
      "SYS_socket",
      "SYS_socketpair",
      "libc::syscall",
    ],
  ],
]);

/** The sandbox probe's file, which is the one binary allowed to ask for a socket. */
const SANDBOX_PROBE = "crates/worker/probes/worker_probe.rs";

/** The Linux backend, which names socket syscalls only to refuse them. */
const SANDBOX_DENY_LIST = "crates/worker/src/sandbox/linux.rs";

/** `P2-L1`'s Linux device layer, which names Landlock and no socket at all. */
const CAPTURE_DEVICE_LAYER = "crates/capture-gate/src/native/linux.rs";

/**
 * The syscalls the Linux backend *makes*, and why each one is not a refusal.
 *
 * Every other `SYS_` name in that file has to sit inside `denied_syscalls`.
 * These four cannot: they are how the sandbox is installed. Landlock and
 * seccomp have no libc wrapper on the glibc versions this repository builds
 * against, so they are reached through `libc::syscall`, which is why that
 * spelling is on the file's socket allowance at all.
 */
const CALLED_SYSCALLS = new Map([
  ["SYS_landlock_create_ruleset", "creates the filesystem ruleset, and probes the ABI version"],
  ["SYS_landlock_add_rule", "adds one path-beneath rule to that ruleset"],
  ["SYS_landlock_restrict_self", "applies the ruleset to this process, irrevocably"],
  ["SYS_seccomp", "installs the filter, and asks whether an action is available"],
]);

/**
 * Every file allowed to spell `libc::syscall`, and the syscalls it may make.
 *
 * `P2-G4` wrote the first-argument rule for one file and keyed it on that one
 * file's name. `P2-L1` is the second file that has to reach a syscall with no
 * libc wrapper, and a second allowance entry with no rule behind it is exactly
 * the hole `docs/contracts/policy-source-scans.md` is about. So the rule is
 * keyed on this map instead: a file on the socket allowance for
 * `libc::syscall` that is not a key here fails, and a call whose first argument
 * is not one of that file's own reviewed names fails.
 *
 * The worker's entry is `CALLED_SYSCALLS`, which keeps its extra rule -- every
 * *other* `SYS_` name in that file must sit inside `denied_syscalls` -- because
 * that file also builds a seccomp deny list and this one does not.
 */
const RAW_SYSCALL_FILES = new Map([
  [
    "crates/capture-gate/src/native/linux.rs",
    new Map([
      ["SYS_landlock_create_ruleset", "creates the device ruleset, and probes the ABI version"],
      ["SYS_landlock_add_rule", "adds one path-beneath rule for a granted device tree"],
      ["SYS_landlock_restrict_self", "applies the ruleset to the forked child, irrevocably"],
    ]),
  ],
  ["crates/worker/src/sandbox/linux.rs", CALLED_SYSCALLS],
]);

/**
 * The three rules that make a raw syscall readable, applied to one file.
 *
 * Every mention of `libc::syscall` is a call, so its arguments stay in sight;
 * every call's first argument is a `libc::SYS_` path, so a number is refused;
 * and every such name is one the file's own reviewed set lists.
 */
function assertRawSyscallsAreReviewed(file, whole, reviewed) {
  const calls = [...whole.matchAll(/\blibc\s*::\s*syscall\s*\(/gu)];
  assert.ok(
    calls.length >= 3,
    `${file} makes only ${calls.length} raw syscalls, so this rule read almost nothing`,
  );
  const mentions = [...whole.matchAll(/\blibc\s*::\s*syscall\b/gu)];
  assert.equal(
    mentions.length,
    calls.length,
    `${file} names libc::syscall ${mentions.length - calls.length} time(s) ` +
      "without calling it, so its arguments are not read",
  );
  const seen = new Set();
  for (const call of calls) {
    const first = whole
      .slice(call.index + call[0].length)
      .split(",")[0]
      .trim();
    assert.match(
      first,
      /^libc::SYS_[A-Za-z0-9_]+$/u,
      `${file} calls libc::syscall with ${first} rather than a libc::SYS_ name`,
    );
    const name = first.slice("libc::".length);
    assert.equal(
      reviewed.has(name),
      true,
      `${file} calls ${first}, which is not one of the reviewed syscalls it installs with`,
    );
    seen.add(name);
  }
  for (const name of reviewed.keys()) {
    assert.equal(seen.has(name), true, `${file} no longer calls ${name}`);
  }
}

/** Path segments that lead to a socket; renaming one hides everything under it. */
const SOCKET_MODULE_SEGMENTS = new Set(["net", "socket", "sys", "WinSock", "named_pipe"]);

/**
 * Import shapes that bring `libc::syscall` into scope under a bare name.
 *
 * The first-argument rule below reads the *call* spelling `libc::syscall(`, so
 * it only ever sees a call written as a path. `T149` wrote three that are not:
 * `use libc::syscall;` then `syscall(41, 2, 1, 0)`, the same through
 * `use libc::syscall as raw;`, and `use libc::*;` in a file whose allowance
 * lists no socket spelling at all. All three open an AF_INET stream socket by
 * number, all three passed every scan here, and all three compiled clean under
 * `cargo clippy -p academic-worker --all-targets --features native-sandbox
 * -- -D warnings`. The `use` item itself carries the spelling `libc::syscall`,
 * so it satisfied the allowance while the call matched nothing.
 *
 * Forbidding the import is what makes the call spell `libc::syscall(`, which is
 * what the first-argument rule reads. It is checked in every file rather than
 * only in the ones with an allowance, because a glob import spells no path and
 * therefore reaches no allowance: that is `S-13`'s real content, not the
 * future-second-entry case the row used to describe.
 *
 * Renaming the crate root rather than the function -- `use libc as l;`,
 * `use libc::{self as l};`, `extern crate libc as raw;` -- is refused by
 * `socketPathAliases` instead, because `libc` is an aliasable root.
 */
const LIBC_SYSCALL_IMPORTS = [
  /\buse\s+(?:::\s*)?libc\s*::\s*(?:\{[^;]*?\b)?syscall\b/gu,
  /\buse\s+(?:::\s*)?libc\s*::\s*(?:\{[^;]*?)?\*/gu,
];

/** Crate roots whose paths can reach a socket; an alias of one hides the rest. */
const ALIASABLE_ROOTS = new Set([
  "std",
  "core",
  "alloc",
  "tokio",
  "rustix",
  "libc",
  "windows_sys",
  "socket2",
  "mio",
  "nix",
]);

/**
 * The one `include!`, spelled out.
 *
 * `#[path]` sites are checked by resolving their target instead of being listed:
 * what matters is that the file they pull in is one this scan already reads.
 * `include!` cannot be checked that way because its argument is computed at
 * build time, so the single site is pinned as whole text and its build script
 * is pinned below.
 */
const GENERATED_SOURCE_INCLUDES = new Map([
  [
    "crates/rpc/src/generated.rs",
    ['include!(concat!(env!("OUT_DIR"), "/academic.v1.rs"));'],
  ],
]);

/** External crates that can open a socket, for the link half. */
const SOCKET_CAPABLE_CRATES = new Set([
  "libc",
  "mio",
  "nix",
  "rustix",
  "socket2",
  "tokio",
  "windows-sys",
]);

/**
 * Which workspace crates link something that could open a socket.
 *
 * This is the link half. The source half proves nobody writes a socket; this
 * proves nobody quietly acquires the ability to by adding a dependency, which
 * is the one bypass that spells no forbidden name anywhere. `libc` reaches
 * almost everything through `libsqlite3-sys` and is listed rather than excused.
 */
const SOCKET_CAPABLE_CLOSURES = {
  "academic-admission": ["libc"],
  "academic-capture-client": ["libc"],
  // `P2-L1`. `libc` reaches it through `academic-domain`. Its own `libc` and
  // `windows-sys` edges are optional and target-specific and this resolve is
  // the default feature set on this host, which is itself the claim that the
  // default lane links no device backend. The source half above is what says
  // the crate names `libc::syscall` to install a Landlock ruleset rather than
  // to open a socket.
  // `P2-L2`. `libc` reaches it through `academic-domain`. The crate spells no
  // socket construct, which is why its `SOCKET_ALLOWANCE` entry is absent
  // rather than empty, and it declares no binary target at all.
  "academic-capture": ["libc"],
  "academic-capture-gate": ["libc"],
  "academic-cli": ["libc", "mio", "rustix", "socket2", "tokio", "windows-sys"],
  "academic-connector": ["libc"],
  // `P2-G6`. `libc` reaches it through `academic-domain`. The crate spells no
  // socket construct, which is why its `SOCKET_ALLOWANCE` entry is absent
  // rather than empty.
  "academic-consent": ["libc"],
  "academic-contracts": ["libc"],
  "academic-core": ["libc", "mio", "rustix", "socket2", "tokio", "windows-sys"],
  // `P2-X5`. `libc` reaches it through `academic-domain`, the same way it
  // reaches `P2-U8` and every other pure crate whose only edge is that one. The
  // crate spells no socket construct, which is why its `SOCKET_ALLOWANCE` entry
  // is absent rather than empty; it opens nothing at all, reads no clock, and
  // takes every graph, reading, lens, coordinate and instant as an argument.
  "academic-cs-map": ["libc"],
  "academic-crypto": ["libc"],
  "academic-daemon": ["libc", "mio", "rustix", "socket2", "tokio", "windows-sys"],
  // `P2-X1`. The desktop links the local-core RPC contract, which links tokio
  // for the named pipe and Unix-domain socket the daemon runs on. Availability
  // is what this row records; the crate spells no socket construct, which is
  // why its `SOCKET_ALLOWANCE` entry is absent rather than empty.
  "academic-desktop": ["libc", "mio", "rustix", "socket2", "tokio", "windows-sys"],
  "academic-domain": ["libc"],
  "academic-egress": ["libc"],
  "academic-egress-boundary": ["libc"],
  // `P2-X7`. `libc` reaches it through `academic-policy`'s bundled SQLite,
  // which arrives transitively through `academic-ingestion` ->
  // `academic-untrusted-content` -> `academic-egress-boundary`. Availability is
  // what this row records; the crate spells no socket construct, which is why
  // its `SOCKET_ALLOWANCE` entry is absent rather than empty, and its own
  // path-root allowlist refuses every one of those three crate roots by name.
  // `P2-U3`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `academic-ingestion`. The crate spells no socket construct, which is
  // why its `SOCKET_ALLOWANCE` entry is absent rather than empty; it opens
  // nothing at all and takes every fact it reads as a frozen input.
  "academic-audit": ["libc"],
  "academic-evidence-center": ["libc"],
  "academic-export-job": ["libc"],
  "academic-indexer": ["libc"],
  // `P2-P3`. `libc` reaches it through `academic-policy`'s bundled SQLite. The
  // crate spells no socket construct, which is why its `SOCKET_ALLOWANCE` entry
  // is absent rather than empty; it ships no transport and every seam it names
  // -- the core graph, the connector fleet, the editor workspace -- is a trait
  // the caller supplies.
  "academic-integrations": ["libc"],
  // `P2-U6`. `libc` reaches it through `academic-domain`. The crate spells no
  // socket construct, which is why its `SOCKET_ALLOWANCE` entry is absent
  // rather than empty, and it implements `ConditionalFetch` nowhere.
  "academic-curriculum": ["libc"],
  "academic-ingestion": ["libc"],
  "academic-keystore-platform": ["windows-sys"],
  // `P2-U5`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `academic-model-run`. The crate spells no socket construct, which is
  // why its `SOCKET_ALLOWANCE` entry is absent rather than empty; it runs no
  // connector and takes every official reading as a value.
  "academic-offering": ["libc"],
  // `P2-L6`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `academic-untrusted-content` and again by way of `academic-gap`. The
  // crate spells no socket construct, which is why its `SOCKET_ALLOWANCE` entry
  // is absent rather than empty; it ships no transport, opens no file and reads
  // no clock, and `no_clock_socket_or_file_reaches_this_crate` in
  // `crates/next-lecture/tests/next_lecture_scans.rs` compares its whole `use`,
  // path and macro inventories against pinned sets in both directions.
  "academic-next-lecture": ["libc"],
  "academic-ledger": ["libc"],
  "academic-policy": ["libc"],
  "academic-portability": ["libc", "rustix", "windows-sys"],
  "academic-projections": ["libc", "rustix", "windows-sys"],
  "academic-record": ["libc"],
  "academic-recovery": ["libc"],
  // `P2-R1`. `libc` reaches it through `academic-policy`'s bundled SQLite. The
  // crate spells no socket construct, which is why its `SOCKET_ALLOWANCE` entry
  // is absent rather than empty; `GitHubRepositoryReader` is a trait with no
  // shipped implementation, the way `academic-egress-boundary`'s transport is.
  "academic-repository": ["libc"],
  // `P2-R2`. `libc` reaches it through `academic-policy`'s bundled SQLite, the
  // same way it reaches `P2-R1`. The crate spells no socket construct, which is
  // why its `SOCKET_ALLOWANCE` entry is absent rather than empty; it opens
  // nothing at all, and takes the bytes it analyzes as an argument.
  "academic-repository-analysis": ["libc"],
  // `P2-R3`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `P2-R1`. The crate spells no socket construct, which is why its
  // `SOCKET_ALLOWANCE` entry is absent rather than empty; it opens nothing at
  // all, and every artifact that is not a `P2-R2` finding arrives as an
  // argument naming subject identifiers.
  "academic-repository-correlation": ["libc"],
  // `P2-R4`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `P2-R1` and `P2-R2`. The crate spells no socket construct, which is
  // why its `SOCKET_ALLOWANCE` entry is absent rather than empty; it opens
  // nothing at all, and takes the classification inputs as arguments.
  "academic-repository-classification": ["libc"],
  // `P2-R5`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `P2-R1`, `P2-R2` and `P2-R4`. The crate spells no socket construct,
  // which is why its `SOCKET_ALLOWANCE` entry is absent rather than empty; it
  // opens nothing at all, and takes every contribution, mapping, rubric and
  // outcome as an argument.
  "academic-repository-competency": ["libc"],
  // `P2-Y1`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `P2-R1`, `P2-R2`, `P2-R4` and `P2-R5`. The crate spells no socket
  // construct, which is why its `SOCKET_ALLOWANCE` entry is absent rather than
  // empty; it opens nothing at all, reads no clock, and takes every competency,
  // criterion, rubric and record as an argument.
  "academic-competency": ["libc"],
  // `P2-Y2`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `P2-R1`, `P2-R2`, `P2-R4`, `P2-R5` and `P2-Y1`. The crate spells no
  // socket construct, which is why its `SOCKET_ALLOWANCE` entry is absent
  // rather than empty; it opens nothing at all, reads no clock, and takes every
  // bundle, entry, adjustment and date as an argument. There is no feed edge of
  // any kind, which is `GATE-38-029` in this table.
  "academic-role-profile": ["libc"],
  // `P2-Y3`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `P2-R1`, `P2-R2`, `P2-R4`, `P2-R5` and `P2-Y1`. The crate spells no
  // socket construct, which is why its `SOCKET_ALLOWANCE` entry is absent
  // rather than empty; it opens nothing at all, reads no clock, and takes every
  // matrix, placement, weight and band as an argument.
  "academic-deletion": ["libc"],
  // `P2-M4`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of the six dev edges its acceptance suite drives. The crate spells no
  // socket construct, it opens nothing at all, reads no clock and reads no
  // environment -- `this_crate_reads_no_clock_and_no_environment` in
  // `crates/non-delegable/tests/non_delegable_scans.rs` is its own whole-file
  // sweep for those.
  "academic-non-delegable": ["libc"],
  "academic-readiness": ["libc"],
  "academic-repository-analyzer": ["libc"],
  "academic-requirement": ["libc"],
  "academic-retention": ["libc"],
  // `P2-U8`. `libc` reaches it through `academic-domain`, the same way `P2-U6`
  // does. The crate spells no socket construct, which is why its
  // `SOCKET_ALLOWANCE` entry is absent rather than empty; it opens nothing at
  // all, and its own whole-set use-statement, signature and field sweeps refuse
  // an addition of any kind at its boundary.
  "academic-review": ["libc"],
  "academic-rpc": ["libc", "mio", "rustix", "socket2", "tokio", "windows-sys"],
  "academic-scenario": ["libc"],
  "academic-store": ["libc", "rustix", "windows-sys"],
  "academic-store-platform": ["libc", "rustix", "windows-sys"],
  "academic-test-support": [],
  "academic-transcript": ["libc"],
  "academic-vault": ["libc", "rustix", "windows-sys"],
  // `P2-N8`. `libc` reaches it through `academic-domain` and through
  // `academic-policy` by way of `academic-offering`'s forecast edge. The crate
  // spells no socket construct, which is why its `SOCKET_ALLOWANCE` entry is
  // absent rather than empty; it opens nothing at all, reads no clock, and
  // every instant it compares arrived inside a caller-supplied value.
  "academic-what-if": ["libc"],
  // `P2-G4`. `libc` and `windows-sys` are direct edges here rather than
  // inherited ones: the sandbox backends are syscalls. Only `libc` appears,
  // because both are optional and target-specific and this resolve is the
  // default feature set on this host -- which is itself the claim that the
  // default lane links no sandbox. The source half above is what says the
  // crate names those syscalls to refuse a socket rather than to open one.
  "academic-worker": ["libc"],
  // `P2-G5`. `libc` reaches it through `academic-egress-boundary`, which
  // reaches it through `academic-policy`'s bundled SQLite. The crate spells no
  // socket construct, which is why its `SOCKET_ALLOWANCE` entry is absent
  // rather than empty.
  // `P2-L3`. `libc` reaches it through `academic-policy`, which arrives with
  // `academic-model-run` and `academic-egress-boundary`. This crate names no
  // socket construct of its own and implements no `OutboundTransport`.
  // `P2-L4`. `libc` reaches it through `academic-policy`, which arrives with
  // `academic-model-run`. This crate names no socket construct and implements
  // no transport.
  "academic-lecture-document": ["libc"],
  "academic-transcription": ["libc"],
  "academic-untrusted-content": ["libc"],
  // `P2-M1`. `libc` reaches it through `academic-policy`'s bundled SQLite, and
  // nothing in this closure can open a socket.
  "academic-model-run": ["libc"],
  // `P2-M2`. `libc` reaches it through the domain crate's own closure; this
  // crate has no edge to the policy, store, egress or worker packages at all.
  "academic-proposal": ["libc"],
  // `P2-N2`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `P2-R1` and `P2-R4`. The crate spells no socket construct, which is
  // why its `SOCKET_ALLOWANCE` entry is absent rather than empty; it opens
  // nothing at all, reads no clock, and takes every evidence input as an
  // argument.
  "academic-knowledge-state": ["libc"],
  // `P2-N3` reaches `libc` the same way and for the same reason: through
  // `academic-policy`'s bundled SQLite by way of `academic-knowledge-state`. The
  // crate spells no socket construct, opens nothing at all, reads no clock, and
  // takes every instant as a `TimestampMillis` argument.
  "academic-freshness": ["libc"],
  // `P2-N7` reaches `libc` the same way and for the same reason: through
  // `academic-policy`'s bundled SQLite by way of `academic-knowledge-state`. The
  // crate spells no socket construct, opens nothing at all, reads no clock, and
  // every instant it holds arrived as a `TimestampMillis` argument.
  "academic-blind-spot": ["libc"],
  // `P2-R6`. `libc` reaches it through `academic-domain` and through
  // `academic-policy`'s bundled SQLite by way of `academic-curriculum` and
  // `academic-repository-classification`, the same way it reaches every crate
  // above those. The crate spells no socket construct, which is why its
  // `SOCKET_ALLOWANCE` entry is absent rather than empty; it opens nothing at
  // all, reads no clock, and takes every goal, overlay, revision and offering as
  // an argument.
  "academic-build-learn": ["libc"],
  // `P2-L5`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `P2-M1` and `P2-G2`, which arrive with `P2-L3`. The crate spells no
  // socket construct, which is why its `SOCKET_ALLOWANCE` entry is absent
  // rather than empty; it opens nothing at all, reads no clock, and takes every
  // corpus, transcript, capture and instant as an argument.
  "academic-student-voice": ["libc"],
  // `P2-N5` reaches `libc` the same way and for the same reason: through
  // `academic-policy`'s bundled SQLite by way of `academic-knowledge-state`. The
  // crate spells no socket construct, opens nothing at all, reads no clock, and
  // every instant it holds arrived inside a `P2-N3` value.
  "academic-gap": ["libc"],
  // `P2-X2`. `libc` reaches it through `academic-domain`, which is the whole of
  // how it arrives: this crate's other product edge, `academic-consent`, adds
  // nothing socket-capable. The crate spells no socket construct, which is why
  // its `SOCKET_ALLOWANCE` entry is absent rather than empty; it opens nothing
  // at all, reads no clock, and every instant it compares arrived as an
  // argument.
  "academic-home": ["libc"],
  // `P2-P1`. `libc` reaches it through `academic-policy`'s bundled SQLite, by
  // way of `P2-U3` and `P2-U6`. The crate spells no socket construct, which is
  // why its `SOCKET_ALLOWANCE` entry is absent rather than empty; it opens
  // nothing at all, and its own whole-set path-root, `std`-module and macro
  // sweeps refuse an addition of any kind at its boundary.
  "academic-export": ["libc"],
  // `P2-N6` reaches `libc` the same way and for the same reason: through
  // `academic-policy`'s bundled SQLite by way of `academic-gap` and
  // `academic-knowledge-state`. The crate spells no socket construct, opens
  // nothing at all, reads no clock, and every instant it holds arrived as a
  // caller-supplied day count or inside a `P2-N3` value.
  "academic-critical-path": ["libc"],
};
async function rustSourcesIfPresent(root) {
  try {
    return await rustSources(root);
  } catch {
    return [];
  }
}

/**
 * The `use` and `extern crate` statements in `code` that rename a socket path.
 *
 * An alias hides every later mention of what it renames, so the two things that
 * could hide a socket may only ever be renamed to `_` -- the trait-import
 * spelling, which cannot be written as a path.
 *
 * The first is a crate root: `use tokio as t;` leaves `t::net::TcpStream`
 * spelling neither `tokio::net` nor anything else on the list. The second is a
 * socket module inside a braced group: `use tokio::{net as n};` spells the
 * module in a shape the `tokio::net` anchor does not match, which is why the
 * whole statement is read and not one path.
 *
 * `self` is a third: it renames whatever path the brace hangs off, which is a
 * segment that appears nowhere in the rename itself. It is resolved to that
 * segment and then judged by the same two questions, so `use libc::{self as l};`
 * is refused -- it renames the crate root -- and `use rustix::fs::{self as rfs};`
 * is not, because `fs` is neither the root nor a socket module. The owner is
 * read as the innermost `name::{` before the `self`, so a nested group like
 * `use rustix::{fs::{self as rfs}, io::Errno};` resolves to `fs` and not
 * `rustix`.
 *
 * `extern crate` is read beside `use` because it renames the same thing.
 *
 * A rename of anything else -- `process::Command as ProcessCommand`,
 * `Ordering as AtomicOrdering` -- is not on a socket path and is left alone;
 * forbidding those would be a rule about imports, not about sockets, and this
 * repository already has several.
 *
 * `T151` reached a numeric socket through `extern crate libc as raw;` and again
 * through `use libc::{self as l};` after the `use libc::syscall` shapes were
 * closed; both are refused here.
 */
function socketPathAliases(code) {
  const found = [];
  for (const match of code.matchAll(/\b(?:use|extern\s+crate)\s+([A-Za-z0-9_]+)\b[^;]*;/gu)) {
    if (!ALIASABLE_ROOTS.has(match[1])) {
      continue;
    }
    for (const [, renamed, alias] of match[0].matchAll(
      /\b([A-Za-z0-9_]+)\s+as\s+([A-Za-z0-9_]+)/gu,
    )) {
      if (renamed === "self") {
        continue;
      }
      const hidesASocketPath =
        renamed === match[1] || SOCKET_MODULE_SEGMENTS.has(renamed);
      if (hidesASocketPath && alias !== "_") {
        found.push(match[0]);
      }
    }
    for (const [, owner, alias] of match[0].matchAll(
      /([A-Za-z0-9_]+)\s*::\s*\{[^{}]*\bself\s+as\s+([A-Za-z0-9_]+)/gu,
    )) {
      const hidesASocketPath = owner === match[1] || SOCKET_MODULE_SEGMENTS.has(owner);
      if (hidesASocketPath && alias !== "_") {
        found.push(match[0]);
      }
    }
  }
  return found;
}

test("only_egress_crate_has_a_socket", async () => {
  // The scan is not vacuous: every pattern matches the call it names, and the
  // stripper does not blind it. A rule that matched nothing would be a rule
  // that proved nothing, which is the failure this repository has hit before.
  const sample =
    "TcpStream::connect(); TcpListener::bind(); UdpSocket::bind(); std::net::Ipv4Addr; " +
    "core::net::SocketAddr; tokio :: net :: TcpStream; rustix::net::socket(); mio::net::TcpStream; " +
    "nix::sys::socket::socket(); socket2::Socket::new(); addr.to_socket_addrs(); ToSocketAddrs; " +
    "lookup_host(); getaddrinfo(); connect_timeout(); WinSock::connect; WSAConnect(); " +
    "libc::connect(fd); UnixStream::connect(); UnixListener::bind(); NamedPipeServer; named_pipe; " +
    "libc::syscall(libc::SYS_socket, 2, 1, 0); SYS_socketpair; SYS_socketcall; SYS_connect; " +
    "SYS_bind; SYS_listen; SYS_accept; SYS_accept4; SYS_sendto; SYS_recvfrom; SYS_sendmsg; " +
    "SYS_recvmsg;";
  for (const pattern of SOCKET_SPELLINGS) {
    assert.match(sample, new RegExp(pattern.source, "u"), `${pattern} matches nothing`);
  }
  assert.equal(rustCodeOnly('let s = "TcpStream::connect";').includes("TcpStream"), false);
  assert.equal(rustCodeOnly("// TcpStream::connect\n").includes("TcpStream"), false);
  assert.equal(rustCodeOnly("/* TcpStream */ let x = 1;").includes("TcpStream"), false);
  assert.equal(rustCodeOnly("let c = '\\n'; TcpStream").includes("TcpStream"), true);

  // The two import shapes are not vacuous either: each matches the statement it
  // names, and neither matches the spelling the first-argument rule reads.
  for (const [sample, expected] of [
    ["use libc::syscall;", true],
    ["use libc::syscall as raw;", true],
    ["use libc::{c_int, syscall};", true],
    ["use libc :: { sys :: socket :: bind , syscall } ;", true],
    ["pub use libc::syscall;", true],
    ["use libc::*;", true],
    ["use libc::{self, *};", true],
    ["use libc::{c_int, syscall_thing};", false],
    ["unsafe { libc::syscall(libc::SYS_socket, 2, 1, 0) }", false],
  ]) {
    // `use libc::{self as l};` is refused by the alias rule below, not here.
    const hit = LIBC_SYSCALL_IMPORTS.some((pattern) =>
      new RegExp(pattern.source, "u").test(sample),
    );
    assert.equal(hit, expected, `the libc import rule reads ${sample} as ${hit}`);
  }

  // And the alias rule reads both spellings of a rename.
  for (const [sample, expected] of [
    ["use tokio as t;", true],
    ["extern crate tokio as t;", true],
    ["extern crate libc as raw;", true],
    ["use tokio::{net as n};", true],
    ["use libc::{self as l};", true],
    ["use tokio::net::{self as n};", true],
    ["use tokio as _;", false],
    ["use rustix::fs::{self as rfs, Mode, OFlags};", false],
    ["use rustix::{fs::{self as rfs, Mode}, io::Errno};", false],
    ["use std::process::Command as ProcessCommand;", false],
  ]) {
    const found = socketPathAliases(sample);
    assert.equal(found.length > 0, expected, `the alias rule reads ${sample} wrongly`);
  }

  const observed = new Map();
  const aliases = [];
  const syscallImports = [];
  const foreign = [];
  const generated = new Map();
  const pathIncludes = [];
  const readFiles = new Set();
  for (const pkg of workspacePackages) {
    const crateRoot = dirname(pkg.manifest_path);
    // Every `.rs` anywhere under the crate, not three directory names plus a
    // build script. `P2-G4` added `crates/worker/probes/`, a `[[bin]]` with an
    // explicit `path` outside `src`, and the three-name walk read none of it --
    // the first shape `docs/contracts/policy-source-scans.md` calls a scan that
    // stops short. A recursive walk from the crate root reaches `build.rs`
    // without naming it, so the pinned inventory below is what bounds build
    // scripts rather than the walk.
    const files = await rustSourcesIfPresent(crateRoot);
    for (const [path, raw] of files) {
      const relative = path.slice(path.indexOf("crates")).split("\\").join("/");
      readFiles.add(resolve(path));
      const code = rustCodeOnly(raw);

      const spellings = new Set();
      for (const pattern of SOCKET_SPELLINGS) {
        for (const match of code.matchAll(pattern)) {
          spellings.add(match[0].replace(/\s+/gu, ""));
        }
      }
      if (spellings.size > 0) {
        observed.set(relative, [...spellings].toSorted());
      }

      // Renames that hide a socket path, in one definition shared with the
      // vacuity samples above so the two cannot drift apart.
      for (const statement of socketPathAliases(code)) {
        aliases.push(`${relative}: ${statement}`);
      }

      // `libc::syscall` may not be imported, in any file. The rule that bounds
      // it reads the call spelling `libc::syscall(`, and an import is what lets
      // the call be written without it.
      for (const pattern of LIBC_SYSCALL_IMPORTS) {
        for (const match of code.matchAll(pattern)) {
          syscallImports.push(`${relative}: ${match[0].replace(/\s+/gu, " ")}`);
        }
      }

      // A foreign function declaration reaches a socket without spelling one.
      // `unsafe_code = "forbid"` refuses these in every crate but the four
      // reviewed leaves, and none of those four declares one today.
      for (const match of code.matchAll(/extern\s*"|#\[\s*link\s*\(|no_mangle/gu)) {
        foreign.push(`${relative}: ${match[0]}`);
      }

      // Source pulled in from outside the scanned trees is source this scan did
      // not read. String literals are stripped from `code`, so the targets are
      // read from a copy that keeps them.
      //
      // What makes that true is membership in the read set, which is only known
      // once every package has been walked -- so the targets are collected here
      // and checked after the loop. Requiring the target merely to exist under
      // `crates/` and end in `.rs` is what this did, and `T141` walked through
      // it: `crates/admission/authority.rs` sits at a crate root, in no walked
      // tree, and satisfied both of those.
      const withStrings = rustCodeOnly(raw, true);
      for (const match of withStrings.matchAll(/#\[\s*path\s*=\s*"([^"]*)"\s*\]/gu)) {
        pathIncludes.push([relative, match[1], resolve(dirname(path), match[1])]);
      }
      const includes = [];
      for (const match of withStrings.matchAll(/include!\s*\([^;]*\);/gu)) {
        includes.push(match[0].split(/\s+/u).join(" "));
      }
      if (includes.length > 0) {
        generated.set(relative, includes.toSorted());
      }
    }
  }

  assert.deepEqual(
    pathIncludes
      .filter(([, , target]) => !readFiles.has(target))
      .map(([relative, spelling]) => `${relative}: ${spelling}`),
    [],
    "a #[path] pulls in source no walked tree contains",
  );

  assert.deepEqual(
    Object.fromEntries([...observed].toSorted(([left], [right]) => left.localeCompare(right))),
    Object.fromEntries(
      [...SOCKET_ALLOWANCE].toSorted(([left], [right]) => left.localeCompare(right)),
    ),
    "a file spells a socket that its allowance does not list",
  );
  // Every file allowed to spell `libc::syscall` is read against its own
  // reviewed set, whichever file it is. This runs before the per-file branches
  // below so a new backend cannot be added to the allowance without being added
  // here too.
  for (const [file, spellings] of observed) {
    if (!spellings.includes("libc::syscall")) {
      continue;
    }
    const reviewed = RAW_SYSCALL_FILES.get(file);
    assert.ok(
      reviewed,
      `${file} is allowed libc::syscall and is not in RAW_SYSCALL_FILES, so nothing reads ` +
        "which syscalls it makes",
    );
    const whole = rustCodeOnly(await readFile(join(...file.split("/")), "utf8"));
    assertRawSyscallsAreReviewed(file, whole, reviewed);
  }
  for (const file of RAW_SYSCALL_FILES.keys()) {
    assert.equal(
      SOCKET_ALLOWANCE.get(file)?.includes("libc::syscall") ?? false,
      true,
      `${file} is reviewed for raw syscalls but no longer spells one`,
    );
  }

  for (const [file, spellings] of observed) {
    if (file === SANDBOX_DENY_LIST) {
      // Every socket syscall this file names has to be inside the function
      // that builds the deny list. A file that names one anywhere else is
      // calling it, not refusing it.
      const source = await readFile(join(...file.split("/")), "utf8");
      const denied = rustCodeOnly(source).match(
        /fn denied_syscalls\(\)\s*->\s*Vec<i64>\s*\{[^]*?\n\}/u,
      );
      assert.ok(denied, `${file} no longer has a denied_syscalls function`);
      const whole = rustCodeOnly(source);
      for (const spelling of spellings) {
        if (spelling === "libc::syscall") {
          // Every raw syscall this file makes has to name the syscall it makes.
          //
          // `libc::syscall(41, 2, 1, 0)` opens an AF_INET stream socket and
          // spells nothing any pattern above can match. `S-11` in
          // `docs/contracts/policy-source-scans.md` recorded that as open on the
          // grounds that nothing in the repository had one and that P2-G4's
          // sandbox would refuse it whatever spelled it. `T146` falsified both:
          // it added exactly that call to this file, every scan passed, and
          // `cargo clippy -p academic-worker --features native-sandbox
          // -- -D warnings` compiled it -- because this file is now on the
          // allowance for `libc::syscall`, and because it holds the parent-side
          // `launch` as well as the child-side `enter`, so the parent runs
          // outside the sandbox it installs.
          //
          // A first argument that is not a `libc::SYS_` name fails here. The
          // split is on the first comma, so an expression is refused too, which
          // is the safe direction: a call whose syscall number is computed is
          // not one this rule can read.
          const calls = [...whole.matchAll(/\blibc\s*::\s*syscall\s*\(/gu)];
          assert.ok(
            calls.length >= 3,
            `${file} makes only ${calls.length} raw syscalls, so this rule read almost nothing`,
          );
          // Every mention of the name is a call. The rule below reads the first
          // argument of a call, so a mention that is not one is a mention it
          // never reads: `T151` wrote `let raw = libc::syscall;` and then
          // `raw(41, 2, 1, 0)`, which satisfies this file's allowance -- the
          // spelling is on it -- and passed. Taking the function as a value is
          // the same reach with the arguments moved out of the rule's sight.
          const mentions = [...whole.matchAll(/\blibc\s*::\s*syscall\b/gu)];
          assert.equal(
            mentions.length,
            calls.length,
            `${file} names libc::syscall ${mentions.length - calls.length} time(s) ` +
              "without calling it, so its arguments are not read",
          );
          for (const call of calls) {
            const first = whole
              .slice(call.index + call[0].length)
              .split(",")[0]
              .trim();
            assert.match(
              first,
              /^libc::SYS_[A-Za-z0-9_]+$/u,
              `${file} calls libc::syscall with ${first} rather than a libc::SYS_ name`,
            );
            assert.equal(
              CALLED_SYSCALLS.has(first.slice("libc::".length)),
              true,
              `${file} calls ${first}, which is not one of the reviewed syscalls it installs with`,
            );
          }
          continue;
        }
        // Counted, not merely present. A spelling that is inside the deny list
        // *and also* somewhere else passes an `includes` check while naming a
        // socket syscall the file does not refuse.
        const inside = denied[0].split(spelling).length - 1;
        const anywhere = whole.split(spelling).length - 1;
        assert.equal(
          anywhere,
          inside,
          `${file} names ${spelling} ${anywhere - inside} time(s) outside its seccomp deny list`,
        );
      }

      // The other half, and the one the allowance-driven loop above cannot
      // reach: *every* `SYS_` name in this file is either one of the four it
      // installs the sandbox with, or it sits inside `denied_syscalls`.
      //
      // The loop above reads the ten socket names the allowance lists, so a
      // non-socket one was free. `T146` put `libc::SYS_memfd_create` outside
      // `denied_syscalls` and every scan passed. This is what the contract
      // pages claimed and the code did not do.
      const named = new Set(whole.match(/\bSYS_[A-Za-z0-9_]+\b/gu) ?? []);
      assert.ok(named.size >= 20, `${file} names only ${named.size} syscalls`);
      for (const name of named) {
        if (CALLED_SYSCALLS.has(name)) {
          continue;
        }
        const inside = denied[0].split(name).length - 1;
        const anywhere = whole.split(name).length - 1;
        assert.equal(
          anywhere,
          inside,
          `${file} names ${name} ${anywhere - inside} time(s) outside its seccomp deny list, ` +
            "and it is not one of the reviewed syscalls this file installs with",
        );
      }
      // An entry nobody calls any more is a stale exception.
      for (const name of CALLED_SYSCALLS.keys()) {
        assert.equal(named.has(name), true, `${file} no longer calls ${name}`);
      }
      continue;
    }
    if (file === SANDBOX_PROBE) {
      // The probe may ask for a socket because the sandbox has to refuse one.
      // What keeps that scoped is that no default build and no product crate
      // can reach the target; both are read from `cargo metadata` rather than
      // taken on trust.
      const worker = packagesByName.get("academic-worker");
      assert.ok(worker, "academic-worker is absent");
      const probeTargets = worker.targets.filter((target) => target.kind.includes("bin"));
      assert.deepEqual(
        probeTargets.map((target) => target.name),
        ["academic-worker-probe"],
        "the worker gained a binary target beside the sandbox probe",
      );
      assert.deepEqual(
        probeTargets.map((target) => target["required-features"] ?? []),
        [["native-sandbox"]],
        "the sandbox probe is buildable without the native-sandbox feature",
      );
      assert.deepEqual(
        workspacePackages
          .filter((pkg) => workspaceDependencyNames(pkg).includes("academic-worker"))
          .map((pkg) => pkg.name),
        [],
        "a crate depends on academic-worker, so the probe is reachable from it",
      );
      continue;
    }
    if (file === CAPTURE_DEVICE_LAYER) {
      // `P2-L1`'s device layer spells `libc::syscall` and nothing else. Its
      // three syscalls, the rule that every mention of the name is a call, and
      // the rule that every call's first argument is one of those three are all
      // applied above, from `RAW_SYSCALL_FILES`, before this loop runs. What is
      // checked here is the other half of the worker's bargain: the probe it
      // launches is a `[[bin]]` with `required-features` and no workspace crate
      // depends on the package, so neither reaches a default build.
      assert.deepEqual(spellings, ["libc::syscall"], `${file} spells more than its allowance`);
      const gate = packagesByName.get("academic-capture-gate");
      assert.ok(gate, "academic-capture-gate is absent");
      const probeTargets = gate.targets.filter((target) => target.kind.includes("bin"));
      assert.deepEqual(
        probeTargets.map((target) => target.name),
        ["academic-capture-probe"],
        "the capture gate gained a binary target beside the device probe",
      );
      assert.deepEqual(
        probeTargets.map((target) => target["required-features"] ?? []),
        [["native-capture"]],
        "the device probe is buildable without the native-capture feature",
      );
      assert.deepEqual(
        workspacePackages
          .filter((pkg) => workspaceDependencyNames(pkg).includes("academic-capture-gate"))
          .map((pkg) => pkg.name),
        [],
        "a crate depends on academic-capture-gate, so the probe is reachable from it",
      );
      continue;
    }
    for (const spelling of spellings) {
      assert.equal(
        LOCAL_IPC_SPELLINGS.has(spelling),
        true,
        `${file} spells the outbound socket construct ${spelling}; only the egress crates may`,
      );
      assert.equal(
        file.startsWith("crates/cli/") || file.startsWith("crates/daemon/"),
        true,
        `${file} is not one of the local IPC transports`,
      );
    }
  }
  assert.deepEqual(aliases, [], "a socket-capable path is aliased to a usable name");
  assert.deepEqual(
    syscallImports,
    [],
    "libc::syscall is imported, so a call to it need not spell the path the syscall rule reads",
  );
  assert.deepEqual(foreign, [], "a foreign function is declared in a workspace crate");
  assert.deepEqual(
    Object.fromEntries([...generated].toSorted(([left], [right]) => left.localeCompare(right))),
    Object.fromEntries(
      [...GENERATED_SOURCE_INCLUDES].toSorted(([left], [right]) => left.localeCompare(right)),
    ),
    "a source file is pulled in by an include! this scan does not read",
  );
  assert.deepEqual(
    workspacePackages
      .filter((pkg) => pkg.targets.some((target) => target.kind.includes("custom-build")))
      .map((pkg) => pkg.name)
      .toSorted(),
    ["academic-rpc"],
    "a crate gained a build script, which can generate source this scan never sees",
  );

  // The link half. A crate that never spells a socket can still acquire one by
  // linking a crate that has it, and that edit spells no forbidden name.
  const closureOf = (id) => {
    const seen = new Set();
    const pending = [id];
    while (pending.length > 0) {
      const current = pending.pop();
      if (current === undefined || seen.has(current)) {
        continue;
      }
      seen.add(current);
      const node = resolveNodesById.get(current);
      if (node === undefined) {
        continue;
      }
      for (const dependency of node.deps) {
        if (dependency.dep_kinds.some((kind) => kind.kind !== "dev")) {
          pending.push(dependency.pkg);
        }
      }
    }
    return [...seen]
      .map((id) => packagesById.get(id).name)
      .filter((name) => SOCKET_CAPABLE_CRATES.has(name))
      .toSorted();
  };
  assert.deepEqual(
    Object.fromEntries(
      workspacePackages
        .map((pkg) => [pkg.name, closureOf(pkg.id)])
        .toSorted(([left], [right]) => left.localeCompare(right)),
    ),
    Object.fromEntries(
      Object.entries(SOCKET_CAPABLE_CLOSURES).toSorted(([left], [right]) =>
        left.localeCompare(right),
      ),
    ),
    "a workspace crate's link closure gained or lost a socket-capable crate",
  );

  // The egress-proxy process and the boundary that stages for it are the two
  // crates the topology permits a socket in, and neither has one: no spelling,
  // and nothing in either link closure that could open one.
  for (const directory of ["crates/egress/", "crates/egress-boundary/"]) {
    assert.equal(
      [...observed.keys()].some((file) => file.startsWith(directory)),
      false,
      `${directory} now spells a socket; update SOCKET_ALLOWANCE in the same commit`,
    );
  }
  assert.deepEqual(SOCKET_CAPABLE_CLOSURES["academic-egress"], ["libc"]);
  assert.deepEqual(SOCKET_CAPABLE_CLOSURES["academic-egress-boundary"], ["libc"]);
});

// t068 section 3.9. A deterministic engine has no clock, no RNG, no network,
// and no model. That is enforced here rather than by a comment, in the two
// halves 2.3-14 already establishes for a capability: which capabilities the
// engine crate's graph makes *available*, and whether engine source *uses* one.
//
// The available half cannot simply forbid an RNG. 2.3-18 admits `getrandom` for
// a synthetic nonce and locator seed, and `uuid`'s v7 feature reaches it, so the
// executable claim is that the engine crate's product closure is exactly the
// reviewed set, that `getrandom` enters it through `uuid` alone, and that no
// clock, network, or model crate is in it at all.
//
// The used half is the source scan. Two engines are implemented -- `GPA` and
// `CREDIT_ACCOUNTING`, both in `academic-record` -- so the scanned set is the
// harness module, its generated registry, the reference engine in the harness
// test, and every source file of that crate. An entry whose lifecycle changes
// without its sources moving with it fails the lifecycle map below rather than
// quietly leaving an implementation unscanned.
//
// Note that this scan matches its API spellings anywhere in a file, comments
// included. That is stricter than the comment two paragraphs down claims, and
// deliberately so: the right response to it is to not spell a clock API in
// prose, not to teach the scan to skip comments.
test("engine_source_contains_no_clock_rng_network_or_model", async () => {
  const registry = JSON.parse(
    await readFile("schemas/registry/engine-registry-v1.json", "utf8"),
  );
  assert.deepEqual(
    registry.engines.map((entry) => entry.name),
    [
      "GPA",
      "CREDIT_ACCOUNTING",
      "GRADUATION_AUDIT",
      "TIMETABLE",
      "OFFICIAL_PREREQUISITE",
      "EQUIVALENCY",
      "TRANSCRIPT_COVERAGE",
      "ARTIFACT_INTEGRITY",
      "REPOSITORY_DIFF",
      "OVERRIDE_RESOLVER",
      "PERMISSION_BROKER",
      "RETENTION_DELETION",
    ],
    "the scan must cover exactly the engines the §28 table names",
  );
  // `P2-U4` built `GPA` and `CREDIT_ACCOUNTING` in `academic-record`, `P2-L4`
  // built `TRANSCRIPT_COVERAGE` in `academic-lecture-document`, and `P2-U3`
  // built `GRADUATION_AUDIT` in `academic-audit`, so those crates' sources are
  // engine sources and are scanned. The map is enumerated rather than counted:
  // a fifth engine flipping while one of these flipped back would keep any
  // count intact.
  const IMPLEMENTED_ENGINES = new Map([
    ["GPA", "academic-record"],
    ["CREDIT_ACCOUNTING", "academic-record"],
    ["GRADUATION_AUDIT", "academic-audit"],
    ["TRANSCRIPT_COVERAGE", "academic-lecture-document"],
  ]);
  for (const engine of registry.engines) {
    const expected = IMPLEMENTED_ENGINES.has(engine.name) ? "IMPLEMENTED" : "PLANNED";
    assert.equal(
      engine.lifecycle,
      expected,
      `${engine.name} changed lifecycle; add or remove its source files in this scan`,
    );
  }

  // The harness module, its generated registry, the reference engine, and
  // every source file of the crate that implements the two live engines.
  //
  // The record half is a recursive walk with a floor rather than a fixed list.
  // `docs/contracts/policy-source-scans.md` records that a fixed path set is
  // the weakest shape a scan can have -- a file split leaves the assertions
  // reading the half that stayed -- and an engine crate is exactly where a new
  // module would appear.
  const recordSources = (await rustSources(join("crates", "record", "src"))).map(([path]) => path);
  assert.ok(
    recordSources.length >= 12,
    `the record engine walk found only ${recordSources.length} files; it stopped short`,
  );
  // `P2-L4` implemented `TRANSCRIPT_COVERAGE` in `academic-lecture-document`,
  // so that crate's sources are engine sources too. Same shape as the record
  // half: a walk with a floor rather than a fixed list.
  const lectureSources = (await rustSources(join("crates", "lecture-document", "src"))).map(
    ([path]) => path,
  );
  assert.ok(
    lectureSources.length >= 10,
    `the coverage engine walk found only ${lectureSources.length} files; it stopped short`,
  );
  // `P2-U3` implemented `GRADUATION_AUDIT` in `academic-audit`, so that
  // crate's sources are engine sources too. Same shape again: a walk with a
  // floor rather than a fixed list.
  const auditSources = (await rustSources(join("crates", "audit", "src"))).map(([path]) => path);
  assert.ok(
    auditSources.length >= 12,
    `the graduation engine walk found only ${auditSources.length} files; it stopped short`,
  );
  const scanned = [
    join("crates", "domain", "src", "engines.rs"),
    join("crates", "domain", "src", "engines", "generated.rs"),
    join("crates", "domain", "tests", "engine_harness.rs"),
    ...recordSources,
    ...lectureSources,
    ...auditSources,
  ];

  // API spellings, not prose: a comment that says "no clock" must not trip the
  // scan and a call that reads one must.
  const forbidden = [
    ["clock", /\bSystemTime\b/u],
    ["clock", /\bInstant::/u],
    ["clock", /\bstd::time\b/u],
    ["clock", /\bchrono::/u],
    ["clock", /\bUtc::now\b/u],
    ["clock", /\bnow_v7\b/u],
    ["RNG", /\bgetrandom\b/u],
    ["RNG", /\brand::/u],
    ["RNG", /\bthread_rng\b/u],
    ["RNG", /\bOsRng\b/u],
    ["RNG", /\bnew_v4\b/u],
    ["network", /\bTcp(?:Stream|Listener)\b/u],
    ["network", /\bUdpSocket\b/u],
    ["network", /\bstd::net\b/u],
    ["network", /\btokio::net\b/u],
    ["network", /\breqwest\b/u],
    // `Actor::ModelRun` is `academic-domain`'s closed actor enum, and a match
    // arm naming it is a *refusal* of an automatic actor -- the opposite of a
    // model call. `P2-L4` refuses a model-authored coverage exclusion with an
    // exhaustive match over that enum, so the bare-name rule read three
    // refusals as three model calls. The lookbehind narrows the rule to what
    // this scan's own comment already says it is -- an API spelling, not a
    // name -- and the control below pins both directions so the narrowing
    // cannot widen into a hole.
    ["model", /(?<!Actor::)\bModelRun\b/u],
    ["model", /\bModelProvider\b/u],
    ["model", /\bInferenceRun\b/u],
  ];
  for (const path of scanned) {
    const source = await readFile(path, "utf8");
    for (const [capability, pattern] of forbidden) {
      assert.doesNotMatch(source, pattern, `${path} reaches for a ${capability} capability`);
    }
  }

  // The scan is not vacuous: each rule matches the call it forbids.
  for (const [capability, pattern] of forbidden) {
    const sample = {
      clock: "let at = SystemTime::now(); Instant::now(); std::time::Duration; chrono::Utc::now(); Uuid::now_v7();",
      RNG: "getrandom::fill(&mut seed); rand::random(); thread_rng(); OsRng.fill(); Uuid::new_v4();",
      network: "TcpStream::connect(); TcpListener::bind(); UdpSocket::bind(); std::net::Ipv4Addr; tokio::net::TcpStream; reqwest::get();",
      model: "ModelRun::record(); ModelProvider::call(); InferenceRun::start();",
    }[capability];
    assert.match(sample, pattern, `the ${capability} rule matches nothing`);
  }

  // The one narrowing this scan carries, pinned in both directions: a model
  // call still trips and the actor-enum variant that refuses one does not.
  const modelRule = forbidden.find(([, pattern]) => String(pattern).includes("ModelRun"));
  assert.ok(modelRule, "the model rule is gone");
  assert.match("ModelRun::record();", modelRule[1]);
  assert.match("let run: ModelRun = value;", modelRule[1]);
  assert.doesNotMatch("Actor::ModelRun { .. } => refuse(),", modelRule[1]);

  // The available half. Names only: versions are pinned by the lockfile gate,
  // and what matters here is that no new capability entered the graph.
  //
  // `--target all` rather than the host target. A host-resolved closure differs
  // between Windows and Linux -- `getrandom` reaches `libc` on one and not the
  // other -- so a host-resolved list would be a Windows claim asserted on every
  // runner. The union over every target in the lockfile is the same everywhere.
  const engineRun = spawnSync(
    "cargo",
    [
      "tree",
      "--locked",
      "--offline",
      "--edges",
      "normal",
      "--target",
      "all",
      "-p",
      "academic-domain",
    ],
    { encoding: "utf8", maxBuffer: CARGO_OUTPUT_BYTES },
  );
  assert.equal(engineRun.status, 0, `locked offline cargo tree failed: ${engineRun.stderr}`);
  const crates = new Set(
    engineRun.stdout
      .replaceAll(/\([^)]*\)/gu, "")
      .split("\n")
      .map((line) => line.replace(/^[^A-Za-z]*/u, "").split(" ")[0].trim())
      .filter((name) => name.length > 0),
  );
  assert.deepEqual(
    [...crates].toSorted(),
    [
      "academic-domain",
      "block-buffer",
      "cfg-if",
      "cpufeatures",
      "crypto-common",
      "digest",
      "generic-array",
      "getrandom",
      "hex",
      "hmac",
      "libc",
      "proc-macro2",
      "quote",
      "r-efi",
      "serde",
      "serde_core",
      "serde_derive",
      "sha2",
      "subtle",
      "syn",
      "thiserror",
      "thiserror-impl",
      "typenum",
      "unicode-ident",
      "uuid",
    ],
    "the engine crate's product closure changed; review the new capability",
  );

  // `getrandom` is reachable, and only under `uuid`. Nothing else may bring it.
  const getrandomOwners = metadata.packages
    .filter((pkg) => crates.has(pkg.name))
    .filter((pkg) => pkg.dependencies.some((dependency) => dependency.name === "getrandom"))
    .map((pkg) => pkg.name)
    .toSorted();
  assert.deepEqual(getrandomOwners, ["uuid"]);

  // The same two halves for the crate that implements `GPA` and
  // `CREDIT_ACCOUNTING`. Its closure is wider than `academic-domain`'s because
  // it depends on `academic-transcript` for the confirmed row, which depends on
  // `academic-admission` for the import gate -- so the Ed25519 stack is in the
  // graph. None of it is a clock, a socket, or a model, and `getrandom` still
  // enters through `uuid` alone; the list is enumerated so that a new edge is a
  // failure rather than a thing to notice later.
  const recordRun = spawnSync(
    "cargo",
    [
      "tree",
      "--locked",
      "--offline",
      "--edges",
      "normal",
      "--target",
      "all",
      "-p",
      "academic-record",
    ],
    { encoding: "utf8", maxBuffer: CARGO_OUTPUT_BYTES },
  );
  assert.equal(recordRun.status, 0, `locked offline cargo tree failed: ${recordRun.stderr}`);
  const recordCrates = new Set(
    recordRun.stdout
      .replaceAll(/\([^)]*\)/gu, "")
      .split("\n")
      .map((line) => line.replace(/^[^A-Za-z]*/u, "").split(" ")[0].trim())
      .filter((name) => name.length > 0),
  );
  assert.deepEqual(
    [...recordCrates].toSorted(),
    [
      "academic-admission",
      "academic-domain",
      "academic-record",
      "academic-transcript",
      "block-buffer",
      "cfg-if",
      "ciborium",
      "ciborium-io",
      "ciborium-ll",
      "cpufeatures",
      "crunchy",
      "crypto-common",
      "curve25519-dalek",
      "curve25519-dalek-derive",
      "digest",
      "ed25519",
      "ed25519-dalek",
      "fiat-crypto",
      "generic-array",
      "getrandom",
      "half",
      "hex",
      "hmac",
      "libc",
      "proc-macro2",
      "quote",
      "r-efi",
      "serde",
      "serde_core",
      "serde_derive",
      "sha2",
      "signature",
      "subtle",
      "syn",
      "thiserror",
      "thiserror-impl",
      "typenum",
      "unicode-ident",
      "uuid",
      "zerocopy",
      "zerocopy-derive",
      "zeroize",
    ],
    "the GPA engine crate's product closure changed; review the new capability",
  );
  const recordGetrandomOwners = metadata.packages
    .filter((pkg) => recordCrates.has(pkg.name))
    .filter((pkg) => pkg.dependencies.some((dependency) => dependency.name === "getrandom"))
    .map((pkg) => pkg.name)
    .toSorted();
  assert.deepEqual(recordGetrandomOwners, ["uuid"]);

  // The same two halves for the crate that implements `GRADUATION_AUDIT`. Its
  // closure is the union of `academic-record`'s and `academic-ingestion`'s,
  // because a graduation audit reads `P2-U4`'s attempts and `P2-U6`'s conflict
  // dispositions; `academic-policy` arrives with the second carrying the
  // bundled SQLite. **No model crate is in it** -- `academic-model-run` is
  // §27.3's provenance aggregate, which is where a model execution is recorded,
  // and it is absent, as is every HTTP client. That absence is what keeps a
  // graduation verdict off any interpreted-text path, and comparing the closure
  // *whole* is what makes an addition of any kind a review rather than
  // something that had to be predicted.
  const auditRun = spawnSync(
    "cargo",
    [
      "tree",
      "--locked",
      "--offline",
      "--edges",
      "normal",
      "--target",
      "all",
      "-p",
      "academic-audit",
    ],
    { encoding: "utf8", maxBuffer: CARGO_OUTPUT_BYTES },
  );
  assert.equal(auditRun.status, 0, `locked offline cargo tree failed: ${auditRun.stderr}`);
  const auditCrates = new Set(
    auditRun.stdout
      .replaceAll(/\([^)]*\)/gu, "")
      .split("\n")
      .map((line) => line.replace(/^[^A-Za-z]*/u, "").split(" ")[0].trim())
      .filter((name) => name.length > 0),
  );
  assert.deepEqual(
    [...auditCrates].toSorted(),
    [
      "academic-admission",
      "academic-audit",
      "academic-domain",
      "academic-egress-boundary",
      "academic-ingestion",
      "academic-policy",
      "academic-record",
      "academic-requirement",
      "academic-transcript",
      "academic-untrusted-content",
      "bitflags",
      "block-buffer",
      "cfg-if",
      "ciborium",
      "ciborium-io",
      "ciborium-ll",
      "cpufeatures",
      "crunchy",
      "crypto-common",
      "curve25519-dalek",
      "curve25519-dalek-derive",
      "digest",
      "ed25519",
      "ed25519-dalek",
      "fallible-iterator",
      "fallible-streaming-iterator",
      "fiat-crypto",
      "generic-array",
      "getrandom",
      "half",
      "hex",
      "hmac",
      "libc",
      "libsqlite3-sys",
      "proc-macro2",
      "quote",
      "r-efi",
      "rusqlite",
      "serde",
      "serde_core",
      "serde_derive",
      "sha2",
      "signature",
      "smallvec",
      "subtle",
      "syn",
      "thiserror",
      "thiserror-impl",
      "typenum",
      "unicode-ident",
      "uuid",
      "zerocopy",
      "zerocopy-derive",
      "zeroize",
    ],
    "the graduation engine crate's product closure changed; review the new capability",
  );
  for (const absent of ["academic-model-run", "academic-scenario", "academic-store", "reqwest"]) {
    assert.ok(
      !auditCrates.has(absent),
      `${absent} entered the graduation audit's product closure`,
    );
  }
  // Two owners here rather than one, for the reason the coverage engine's
  // closure records: `rusqlite` declares `getrandom` for SQLite's own
  // randomness and arrives through `academic-policy`, which `academic-ingestion`
  // pulls in. That is a fact about the graph, not about this engine -- the
  // "used" half above scans every source file of this crate for an RNG spelling
  // and finds none, and the audit's determinism rests on the frozen-input
  // signature and that scan rather than on a database driver being absent from
  // a transitive closure.
  const auditGetrandomOwners = metadata.packages
    .filter((pkg) => auditCrates.has(pkg.name))
    .filter((pkg) => pkg.dependencies.some((dependency) => dependency.name === "getrandom"))
    .map((pkg) => pkg.name)
    .toSorted();
  assert.deepEqual(auditGetrandomOwners, ["rusqlite", "uuid"]);

  // The same two halves for the crate that implements `TRANSCRIPT_COVERAGE`.
  // Its closure is wider again because the coverage validator reads a `P2-L2`
  // capture journal and a `P2-M1` calibration, and `academic-policy` arrives
  // with them carrying the bundled SQLite. None of it is a clock, a socket, or
  // a model, and `getrandom` still enters through `uuid` alone.
  const lectureRun = spawnSync(
    "cargo",
    [
      "tree",
      "--locked",
      "--offline",
      "--edges",
      "normal",
      "--target",
      "all",
      "-p",
      "academic-lecture-document",
    ],
    { encoding: "utf8", maxBuffer: CARGO_OUTPUT_BYTES },
  );
  assert.equal(lectureRun.status, 0, `locked offline cargo tree failed: ${lectureRun.stderr}`);
  const lectureCrates = new Set(
    lectureRun.stdout
      .replaceAll(/\([^)]*\)/gu, "")
      .split("\n")
      .map((line) => line.replace(/^[^A-Za-z]*/u, "").split(" ")[0].trim())
      .filter((name) => name.length > 0),
  );
  assert.deepEqual(
    [...lectureCrates].toSorted(),
    [
      "academic-capture",
      "academic-consent",
      "academic-domain",
      "academic-egress-boundary",
      "academic-lecture-document",
      "academic-model-run",
      "academic-policy",
      "academic-proposal",
      "academic-transcription",
      "academic-untrusted-content",
      "bitflags",
      "block-buffer",
      "cfg-if",
      "cpufeatures",
      "crypto-common",
      "digest",
      "fallible-iterator",
      "fallible-streaming-iterator",
      "generic-array",
      "getrandom",
      "hex",
      "hmac",
      "libc",
      "libsqlite3-sys",
      "proc-macro2",
      "quote",
      "r-efi",
      "rusqlite",
      "serde",
      "serde_core",
      "serde_derive",
      "sha2",
      "smallvec",
      "subtle",
      "syn",
      "thiserror",
      "thiserror-impl",
      "typenum",
      "unicode-ident",
      "uuid",
    ],
    "the coverage engine crate's product closure changed; review the new capability",
  );
  // Two owners here rather than one, and the second is recorded rather than
  // filtered out. `rusqlite` declares `getrandom` for SQLite's own randomness
  // and arrives through `academic-policy`, which every edge to the transcript
  // pulls in. That is a fact about the graph, not about this engine: the "used"
  // half above scans every source file of this crate for an RNG spelling and
  // finds none, and the engine's determinism rests on the frozen-input
  // signature and that scan rather than on a database driver being absent from
  // a transitive closure.
  const lectureGetrandomOwners = metadata.packages
    .filter((pkg) => lectureCrates.has(pkg.name))
    .filter((pkg) => pkg.dependencies.some((dependency) => dependency.name === "getrandom"))
    .map((pkg) => pkg.name)
    .toSorted();
  assert.deepEqual(lectureGetrandomOwners, ["rusqlite", "uuid"]);
});


test("sqlcipher_feature_is_not_default", () => {
  const storePackage = packagesByName.get("academic-store");
  assert.deepEqual(storePackage.features.default, ["bundled-sqlite"]);
  assert.deepEqual(storePackage.features["bundled-sqlite"], ["rusqlite/bundled"]);
  assert.deepEqual(storePackage.features["sqlcipher-spike"], [
    "rusqlite/bundled-sqlcipher-vendored-openssl",
  ]);
  assert.deepEqual(storePackage.features["sqlcipher-store"], [
    "dep:academic-crypto",
    "rusqlite/bundled-sqlcipher-vendored-openssl",
  ]);
  const storeNode = resolveNodesById.get(storePackage.id);
  assert.equal(storeNode.features.includes("bundled-sqlite"), true);
  assert.equal(storeNode.features.includes("sqlcipher-spike"), false);
  assert.equal(storeNode.features.includes("sqlcipher-store"), false);

  // The default product graph resolves neither the encrypted lane's crypto
  // edge nor the OpenSSL that SQLCipher would drag in with it.
  const defaultPackages = defaultProductPackageNames();
  assert.equal(defaultPackages.has("openssl-src"), false);
  assert.equal(
    (resolveNodesById.get(packagesByName.get("libsqlite3-sys").id)?.features ?? []).some(
      (feature) => feature.startsWith("bundled-sqlcipher"),
    ),
    false,
    "the default graph resolved a SQLCipher libsqlite3-sys",
  );
});

// t068 section 2.3-13. The two lanes are mutually exclusive at compile time, so
// the claim to enforce is not "the encrypted lane is absent" -- it is that
// selecting it swaps the whole lane rather than adding to the default one.
//
// This asks `cargo tree` rather than `cargo metadata`: metadata resolves
// features across the entire workspace, where every other crate still asks for
// the default `academic-store`, so it cannot answer a package-scoped question.
// `cargo tree -p ... --no-default-features` resolves exactly what the encrypted
// lane's own build command resolves.
test("encrypted_store_lane_replaces_the_plaintext_lane", async () => {
  // The encrypted lane pulls the SQLCipher build and the key schedule.
  const cipherTree = featureTree([
    "-p",
    "academic-store",
    "--no-default-features",
    "--features",
    "sqlcipher-store",
  ]);
  assert.ok(
    cipherTree.includes("openssl-sys"),
    "the encrypted lane did not select a SQLCipher build",
  );
  assert.ok(
    cipherTree.includes("academic-crypto"),
    "the encrypted lane did not select the P2-K1 key schedule",
  );

  // The default store lane pulls neither. The `dep:academic-crypto` edge is
  // what makes this a lane swap rather than an addition: it exists only when
  // `sqlcipher-store` is on, so its presence or absence reads the resolved
  // feature set without having to parse one.
  const plaintextTree = featureTree(["-p", "academic-store"]);
  for (const forbidden of ["openssl", "academic-crypto"]) {
    assert.equal(
      plaintextTree.includes(forbidden),
      false,
      `the default store graph selected ${forbidden}`,
    );
  }

  const defaultTree = featureTree(["-p", "academic-daemon"]);
  for (const forbidden of ["sqlcipher", "openssl", "academic-crypto"]) {
    assert.equal(
      defaultTree.includes(forbidden),
      false,
      `the default daemon graph selected ${forbidden}`,
    );
  }

  // Five crates declare a crypto edge, and every one of them is a lane that is
  // off by default: the store's schema-2 database, the vault's
  // AEAD_CHUNKED_V2 objects, the encrypted portability lane, `P2-K4`'s
  // recovery contract, and `P2-K5`'s rotation and retention engine.
  // `academic-recovery` and `academic-retention` are the two non-optional
  // edges, because each of those crates *is* its contract and has no other
  // lane; neither is reached from any product binary, which
  // `encrypted_portability_lane_is_not_default` and
  // `rotation_engine_lane_is_not_default` prove below.
  const storeCryptoDependents = workspacePackages
    .filter((pkg) => productDependencyNames(pkg).includes("academic-crypto"))
    .map((pkg) => pkg.name)
    .toSorted();
  // `academic-repository` is the sixth, and it is here for a different reason
  // than the five above: it needs `DeviceKeystore`, which is the seam between a
  // secret and the operating-system broker, to hold `P2-R1`'s GitHub token. It
  // is not an encrypted-lane crate. What keeps that edge from widening this
  // lane is that `academic-crypto`'s default feature set is empty, so the
  // `os-keystore` FFI leaf is not in this closure, and that nothing depends on
  // `academic-repository` -- the two conditions
  // `encrypted_portability_lane_is_not_default` and
  // `rotation_engine_lane_is_not_default` check for the others.
  assert.deepEqual(storeCryptoDependents, [
    "academic-portability",
    "academic-recovery",
    "academic-repository",
    "academic-retention",
    "academic-store",
    "academic-vault",
  ]);
  // The encrypted restore reaches the key schedule through its own edge and
  // the rotation engine through a second one; both are optional and both are
  // selected by the same non-default lane feature.
  assert.equal(
    productDependencyNames(packagesByName.get("academic-portability")).includes(
      "academic-retention",
    ),
    true,
    "the encrypted restore no longer declares the tombstone edge",
  );

  // The default portability lane pulls neither the key schedule nor the
  // recovery contract, so nothing on the shipping path can reach a backup key.
  const plaintextPortabilityTree = shippingTree(["-p", "academic-portability"]);
  for (const forbidden of [
    "openssl",
    "academic-crypto",
    "academic-recovery",
    "academic-retention",
  ]) {
    assert.equal(
      plaintextPortabilityTree.includes(forbidden),
      false,
      `the default portability graph selected ${forbidden}`,
    );
  }

  // The compile-time guard itself, in the library that declares both features.
  const lib = await readFile("crates/store/src/lib.rs", "utf8");
  assert.match(
    lib,
    /#\[cfg\(all\(feature = "bundled-sqlite", feature = "sqlcipher-store"\)\)\]\s+compile_error!/u,
    "the mutually exclusive lane guard is missing",
  );

  // Migration numbering is allocated once and never reordered.
  const migration = await readFile(
    "migrations/store/0003_phase2_encrypted_identity.sql",
    "utf8",
  );
  assert.match(migration, /CHECK \(schema_version = 2\)/u);
  assert.match(migration, /CHECK \(schema_semver = '2\.0\.0'\)/u);
  assert.match(
    migration,
    /format_uuid = x'67cb6d3ea27e4b53b1e727d46920e4f9'/u,
    "migration 0003 does not pin the frozen schema-2 format UUID",
  );
});

// t068 section 3.4. The encrypted object lane is a non-default feature on the
// vault, exactly like the encrypted store lane is on the store: a default
// product build resolves neither the key schedule nor an AEAD, and the two
// object namespaces (`vault/v1` plaintext, `vault/v2` encrypted) are separate
// physical trees rather than one tree with a flag.
test("encrypted_object_lane_is_not_default", async () => {
  const vaultPackage = packagesByName.get("academic-vault");
  assert.deepEqual(vaultPackage.features.default, []);
  assert.deepEqual(vaultPackage.features["aead-objects"], [
    "dep:academic-crypto",
    "dep:chacha20poly1305",
    "dep:subtle",
  ]);
  assert.deepEqual(vaultPackage.features["phase2-fault-injection"], []);

  const vaultNode = resolveNodesById.get(vaultPackage.id);
  assert.equal(vaultNode.features.includes("aead-objects"), false);
  assert.equal(vaultNode.features.includes("phase2-fault-injection"), false);

  // The default product graph resolves no AEAD through the vault.
  const defaultTree = featureTree(["-p", "academic-daemon"]);
  for (const forbidden of ["chacha20poly1305", "academic-crypto"]) {
    assert.equal(
      defaultTree.includes(forbidden),
      false,
      `the default daemon graph selected ${forbidden}`,
    );
  }

  // Selecting the lane is what pulls them in.
  const encryptedTree = featureTree([
    "-p",
    "academic-vault",
    "--features",
    "aead-objects",
  ]);
  for (const required of ["chacha20poly1305", "academic-crypto"]) {
    assert.ok(
      encryptedTree.includes(required),
      `the encrypted object lane did not select ${required}`,
    );
  }

  // The two namespaces and their extensions are frozen in one place.
  const layout = await readFile("crates/vault/src/layout.rs", "utf8");
  assert.match(layout, /PlaintextSyntheticV1 => "v1"/u);
  assert.match(layout, /AeadChunkedV2 => "v2"/u);
  assert.match(layout, /PlaintextSyntheticV1 => "obj"/u);
  assert.match(layout, /AeadChunkedV2 => "aobj"/u);

  // The frozen header geometry. A silent change to any of these moves every
  // committed object byte.
  const object = await readFile("crates/vault/src/object.rs", "utf8");
  for (const [name, value] of [
    ["OBJECT_FORMAT_VERSION: u16", "2"],
    ["AEAD_ID_XCHACHA20_POLY1305: u8", "1"],
    ["BASE_NONCE_BYTES: usize", "24"],
    ["STREAMING_PREFIX_BYTES: usize", "86"],
    ["WRAP_AAD_BYTES: usize", "128"],
    ["HEADER_LEN_FIELD: u16", "200"],
  ]) {
    assert.ok(
      object.includes(`${name} = ${value};`),
      `the frozen object constant ${name} is no longer ${value}`,
    );
  }
});

// t068 section 5, `P2-K5`. The rotation and retention engine is a workspace
// crate nothing in the shipping graph links, and the half of it that touches
// real `AEAD_CHUNKED_V2` objects sits behind a non-default feature — so a
// default product build resolves neither an AEAD nor the key schedule through
// it, exactly as the encrypted store, object, and portability lanes do.
test("rotation_engine_lane_is_not_default", async () => {
  const retention = packagesByName.get("academic-retention");
  assert.ok(retention, "academic-retention is not a workspace member");
  assert.deepEqual(retention.features.default, []);
  assert.deepEqual(retention.features["rotation-engine"], [
    "dep:academic-vault",
    "academic-vault/aead-objects",
  ]);
  assert.deepEqual(retention.features["phase2-fault-injection"], [
    "academic-vault?/phase2-fault-injection",
  ]);

  const node = resolveNodesById.get(retention.id);
  assert.equal(node.features.includes("rotation-engine"), false);
  assert.equal(node.features.includes("phase2-fault-injection"), false);

  // Selecting the lane is what pulls the encrypted object namespace in.
  const engineTree = featureTree(["-p", "academic-retention", "--features", "rotation-engine"]);
  for (const required of ["academic-vault", "chacha20poly1305", "academic-crypto"]) {
    assert.ok(
      engineTree.includes(required),
      `the rotation engine lane did not select ${required}`,
    );
  }
  // Without the lane the object namespace is not there at all. The key schedule
  // still is, and deliberately: the journal names key *generations* and the
  // revocation contract reads recipient records, both of which are
  // `academic-crypto`'s and neither of which opens an object.
  const defaultRetentionTree = shippingTree(["-p", "academic-retention"]);
  assert.equal(
    defaultRetentionTree.includes("academic-vault"),
    false,
    "the default retention graph selected the object vault",
  );
  const vaultNode = resolveNodesById.get(packagesByName.get("academic-vault").id);
  assert.equal(
    vaultNode.features.includes("aead-objects"),
    false,
    "the retention crate turned the encrypted object lane on for the whole workspace",
  );

  // Two workspace crates declare a product edge to it and no third may.
  //
  // `academic-portability`'s edge is optional: its encrypted restore re-applies
  // the tombstones a backup carries, which is `P2-K5`'s keyless positioned
  // write and cannot be imitated on the portability side without duplicating a
  // deletion mechanism.
  //
  // `academic-deletion` is `P2-P2`, the task this comment used to point
  // forward to: the product deletion flow that supplies the real resolver and
  // the real executor `P2-K5` left seams for. Its edge is **not** optional,
  // because the plan, the four-word vocabulary, the journal and the tombstone
  // are pure Rust in the default retention lane and the flow is built on them
  // on every platform. What stays true is the claim this test was written to
  // make: no crate resolves the object namespace that can destroy a key slot in
  // a default graph. `deletion_lane_is_not_default` holds that for the new
  // edge, and the four shipping graphs below still hold it for the binaries.
  const retentionDependents = workspacePackages
    .filter((pkg) => productDependencyNames(pkg).includes("academic-retention"))
    .map((pkg) => pkg.name)
    .toSorted();
  assert.deepEqual(
    retentionDependents,
    ["academic-deletion", "academic-portability"],
    "a crate other than the deletion flow and the encrypted restore links the rotation engine",
  );
  const portabilityRetentionEdge = packagesByName
    .get("academic-portability")
    .dependencies.find((dependency) => dependency.name === "academic-retention");
  assert.ok(portabilityRetentionEdge, "the encrypted restore lost its tombstone edge");
  assert.equal(
    portabilityRetentionEdge.optional,
    true,
    "the tombstone edge is not behind the encrypted portability lane",
  );
  for (const shipping of [
    shippingTree(["-p", "academic-portability"]),
    shippingTree(["-p", "academic-daemon"]),
    shippingTree(["-p", "academic-cli"]),
  ]) {
    assert.equal(
      shipping.includes("academic-retention"),
      false,
      "a default graph selected the rotation engine",
    );
  }
  // `academic-deletion` does resolve it, and that is the edge above. What it
  // must not resolve is the half that opens an object: the object namespace and
  // the AEAD stay behind `academic-retention`'s own non-default feature, which
  // this crate's non-default `deletion-engine` lane is the only thing that
  // selects.
  const deletionShipping = shippingTree(["-p", "academic-deletion"]);
  assert.ok(
    deletionShipping.includes("academic-retention"),
    "the deletion flow lost its edge to the retention crate",
  );
  assert.equal(
    deletionShipping.includes("academic-vault"),
    false,
    "the default deletion graph selected the object vault",
  );
  // The key schedule is still there and deliberately: `academic-retention`'s
  // own default lane carries it, because the journal names key generations and
  // the revocation contract reads recipient records. Neither opens an object,
  // and this crate names neither.
  assert.ok(
    deletionShipping.includes("academic-crypto"),
    "the retention crate stopped bringing the key schedule with it",
  );

  // Phase 2 has not accepted a *rotation*, only the machinery that would run
  // one. `rotation-orchestration` is what selects the machinery's entry points;
  // without it they refuse on their first line. Nothing in any shipping graph
  // may select it, and the feature must stay empty — a feature that pulled a
  // dependency in would be a second way to notice it, and a feature with an
  // implied edge would be a second way to enable it.
  assert.deepEqual(
    retention.features["rotation-orchestration"],
    [],
    "the rotation orchestration lane grew a dependency edge",
  );
  assert.equal(
    node.features.includes("rotation-orchestration"),
    false,
    "the workspace resolve selects the rotation orchestration lane",
  );
  const portability = packagesByName.get("academic-portability");
  assert.deepEqual(
    portability.features["encrypted-portability-rotation"],
    ["encrypted-portability", "academic-retention/rotation-orchestration"],
    "the encrypted rotation lane no longer selects exactly the two things it names",
  );
  assert.equal(
    portability.features["encrypted-portability"].includes(
      "academic-retention/rotation-orchestration",
    ),
    false,
    "the plain encrypted portability lane turns the rotation gate off",
  );
  assert.equal(
    resolveNodesById
      .get(portability.id)
      .features.includes("encrypted-portability-rotation"),
    false,
    "the workspace resolve selects the encrypted rotation lane",
  );
  for (const [label, tree] of [
    ["academic-portability", shippingTree(["-p", "academic-portability"])],
    ["academic-daemon", shippingTree(["-p", "academic-daemon"])],
    ["academic-cli", shippingTree(["-p", "academic-cli"])],
    ["workspace", shippingTree(["--workspace"])],
  ]) {
    assert.equal(
      tree.includes("rotation-orchestration"),
      false,
      `the ${label} shipping graph selected the rotation orchestration lane`,
    );
  }

  // The refusal is decided once, by the build and by nothing else. A second
  // decision site, an environment variable, or a debug-build branch would each
  // be the "quiet flag" t068 section 3.1 forbids, and each has been injected
  // and observed to fail `rotation_gate.rs`.
  const rotation = await readFile("crates/retention/src/rotation.rs", "utf8");
  assert.equal(
    rotation.match(/cfg!\(feature = "rotation-orchestration"\)/gu)?.length,
    1,
    "the rotation gate is decided in more than one place",
  );
  for (const source of [
    "crates/retention/src/rotation.rs",
    "crates/retention/src/engine.rs",
    "crates/retention/src/recipients.rs",
  ]) {
    const text = await readFile(source, "utf8");
    for (const forbidden of [
      'cfg(feature = "rotation-orchestration")',
      "ACADEMIC_OS_ALLOW_ROTATION",
      "debug_assertions",
    ]) {
      assert.equal(
        text.includes(forbidden),
        false,
        `${source} holds ${forbidden}, which is a second way past the rotation gate`,
      );
    }
  }

  // The frozen journal and shred contracts, in the one place each is defined.
  const journal = await readFile("crates/retention/src/journal.rs", "utf8");
  assert.ok(journal.includes('pub const JOURNAL_VERSION: u8 = 1;'));
  assert.ok(
    journal.includes('pub const ROTATION_JOURNAL_RELATIVE_PATH: &str = "keys/rotation-journal.jsonl";'),
    "the rotation journal is no longer at the t068 section 3.2 path",
  );
  const object = await readFile("crates/vault/src/object.rs", "utf8");
  assert.ok(
    object.includes('pub const KEY_SLOT_OFFSET: usize = WRAP_AAD_BYTES;'),
    "the crypto-shred key slot is no longer defined as the wrapped DEK offset",
  );
  assert.ok(
    object.includes('pub const KEY_SLOT_SHRED_MARKER: &[u8; 24] = b"ACOB-KEYSLOT-SHREDDED-V1";'),
    "the shred marker changed, which is an object-format break",
  );

  // The four-word retention vocabulary is closed and is not a free string.
  const plan = await readFile("crates/retention/src/plan.rs", "utf8");
  assert.ok(
    plan.includes(
      'pub const RETENTION_OUTCOMES: &[&str] = &["PLANNED", "COMPLETE", "PARTIAL", "REPAIR_REQUIRED"];',
    ),
    "the retention result vocabulary is not the four words t068 section 5 fixes",
  );

  // `GATE-38-026` stays a user decision: the mechanism is here and no default
  // policy is, which is the same shape `P2-K4` used for `GATE-38-031`.
  for (const forbidden of [
    "impl Default for OriginalVoiceAuthority",
    "DEFAULT_ORIGINAL_VOICE_AUTHORITY",
    "ORIGINAL_VOICE_DELETION_ALLOWED",
  ]) {
    assert.equal(
      plan.includes(forbidden),
      false,
      `the retention planner decides GATE-38-026 through ${forbidden}`,
    );
  }
});

/**
 * Resolves a feature tree exactly as the corresponding build would.
 *
 * The trailing `(path)` of every workspace entry is stripped: it is the
 * checkout location, and a worktree whose directory name happens to contain
 * `sqlcipher` would otherwise read as a selected dependency.
 */
function featureTree(selector) {
  const run = spawnSync(
    "cargo",
    ["tree", "--locked", "--offline", "--edges", "features", ...selector],
    { encoding: "utf8", maxBuffer: CARGO_OUTPUT_BYTES },
  );
  assert.equal(run.status, 0, `locked offline cargo tree failed: ${run.stderr}`);
  return run.stdout.replaceAll(/\([^)]*\)/gu, "");
}

/// The shipping graph only. `featureTree` includes dev edges, which is right
/// when the question is "what does this feature select" and wrong when the
/// question is "what does a product binary link".
function shippingTree(selector) {
  const run = spawnSync(
    "cargo",
    ["tree", "--locked", "--offline", "--edges", "normal,build", ...selector],
    { encoding: "utf8", maxBuffer: CARGO_OUTPUT_BYTES },
  );
  assert.equal(run.status, 0, `locked offline cargo tree failed: ${run.stderr}`);
  return run.stdout.replaceAll(/\([^)]*\)/gu, "");
}

// t068 section 5, `P2-P2`. The deletion and retention product flow is a
// workspace crate nothing in the shipping graph links, and the half of it that
// reaches real `AEAD_CHUNKED_V2` objects — the shredder and the object-tree
// index — sits behind a non-default feature that selects `academic-retention`'s
// own non-default object lane. So a default product build resolves neither an
// AEAD nor the key schedule through it, exactly as the encrypted store, object,
// portability and rotation lanes do.
test("deletion_lane_is_not_default", () => {
  const deletion = packagesByName.get("academic-deletion");
  assert.ok(deletion, "academic-deletion is not a workspace member");
  assert.deepEqual(deletion.features.default, []);
  assert.deepEqual(deletion.features["deletion-engine"], [
    "dep:academic-vault",
    "academic-vault/aead-objects",
    "academic-retention/rotation-engine",
  ]);
  assert.deepEqual(deletion.features["phase2-fault-injection"], [
    "academic-retention/phase2-fault-injection",
    "academic-vault?/phase2-fault-injection",
  ]);

  const node = resolveNodesById.get(deletion.id);
  assert.equal(node.features.includes("deletion-engine"), false);
  assert.equal(node.features.includes("phase2-fault-injection"), false);

  // Selecting the lane is what pulls the encrypted object namespace in.
  const engineTree = featureTree(["-p", "academic-deletion", "--features", "deletion-engine"]);
  for (const required of ["academic-vault", "chacha20poly1305", "academic-crypto"]) {
    assert.ok(
      engineTree.includes(required),
      `the deletion engine lane did not select ${required}`,
    );
  }

  // Nothing in the shipping graph links it. It is a leaf, like every other view
  // crate in this stage, and `P2-Z1` is what will drive it.
  const deletionProductDependents = workspacePackages
    .filter((pkg) => productDependencyNames(pkg).includes("academic-deletion"))
    .map((pkg) => pkg.name)
    .toSorted();
  assert.deepEqual(
    deletionProductDependents,
    [],
    "a crate links the deletion flow before P2-Z1 drives it",
  );
  // One crate links it while a test target is compiling, and only one.
  // `P2-M4`'s `ai_cannot_confirm_deletion` drives this crate's real preview and
  // real confirmation, because the claim that task makes is that its compiled
  // non-delegable set **agrees with the doors that already exist** rather than
  // checking them a second time. A dev edge is not in the shipping graph, so
  // the leaf property above is unchanged; this assertion is what stops the dev
  // edge from becoming a second, unexamined dependent.
  const deletionDevDependents = workspacePackages
    .filter((pkg) => devDependencyNames(pkg).includes("academic-deletion"))
    .map((pkg) => pkg.name)
    .toSorted();
  assert.deepEqual(
    deletionDevDependents,
    ["academic-non-delegable"],
    "the crates that drive the deletion flow from a test target changed",
  );

  // It persists nothing: no store edge of any kind, so it claims no migration
  // number and adds no canonical table.
  for (const forbidden of ["academic-store", "academic-store-platform"]) {
    assert.equal(
      workspaceDependencyNames(deletion).includes(forbidden),
      false,
      `the deletion flow declared a ${forbidden} edge`,
    );
  }
  // And no product edge to the broker: the provider deletion receipt it links
  // is `P2-G3`'s row, compared through a dev edge, so no product file here can
  // name a type that owns a transmitted byte.
  assert.equal(
    productDependencyNames(deletion).includes("academic-policy"),
    false,
    "the deletion flow declared a product edge to the permission broker",
  );
  assert.ok(
    devDependencyNames(deletion).includes("academic-policy"),
    "the deletion flow lost the dev edge that compares the broker's own receipt row",
  );
});

// t068 section 5, `P2-U7`. The transcript ingestion boundary is a workspace
// crate nothing in the shipping graph links, its encrypted half sits behind a
// non-default feature that selects the vault's own non-default object lane, and
// the one way past its admission gate is compiled only by a test-only feature.
test("transcript_lanes_are_not_default", () => {
  const transcript = packagesByName.get("academic-transcript");
  assert.ok(transcript, "academic-transcript is not a workspace member");
  assert.deepEqual(transcript.features.default, []);
  assert.deepEqual(transcript.features["encrypted-vault"], [
    "dep:academic-vault",
    "academic-vault/aead-objects",
  ]);
  assert.deepEqual(transcript.features["phase2-fault-injection"], []);

  const node = resolveNodesById.get(transcript.id);
  assert.equal(node.features.includes("encrypted-vault"), false);
  assert.equal(node.features.includes("phase2-fault-injection"), false);

  // Nothing that ships links it, so no product binary reaches an import path.
  for (const shipping of [
    shippingTree(["-p", "academic-daemon"]),
    shippingTree(["-p", "academic-cli"]),
  ]) {
    assert.equal(
      shipping.includes("academic-transcript"),
      false,
      "a default product graph selected the transcript ingestion boundary",
    );
  }

  // The default transcript graph resolves no AEAD and no key schedule; the lane
  // is what pulls them in.
  const defaultTree = shippingTree(["-p", "academic-transcript"]);
  for (const forbidden of ["chacha20poly1305", "academic-crypto", "academic-vault"]) {
    assert.equal(
      defaultTree.includes(forbidden),
      false,
      `the default transcript graph selected ${forbidden}`,
    );
  }
  const encryptedTree = featureTree([
    "-p",
    "academic-transcript",
    "--features",
    "encrypted-vault",
  ]);
  for (const required of ["academic-vault", "chacha20poly1305"]) {
    assert.ok(
      encryptedTree.includes(required),
      `the transcript encrypted lane did not select ${required}`,
    );
  }
});

// t068 section 5, `P2-U7`, "redaction is a projection, never a source edit".
// The invariant is carried by the absence of a mutator, and an absence is the
// one thing a Rust test cannot assert about itself: a suite that projected a
// transcript and re-checked its digest would keep passing on the day someone
// added `set_student_number`. This reads the source instead.
test("transcript_redaction_has_no_source_edit_path", async () => {
  const sources = await rustSources("crates/transcript/src");
  const joined = sources.map(([, contents]) => contents).join("\n");

  // No exclusive borrow of a normalized record reaches any signature, so no
  // caller can be handed one to write through.
  for (const owner of ["NormalizedTranscript", "TranscriptIdentity", "TranscriptRow"]) {
    assert.equal(
      new RegExp(`&\\s*mut\\s+${owner}\\b`, "u").test(joined),
      false,
      `a signature in academic-transcript takes &mut ${owner}, so a projection is no longer the only way to change one`,
    );
  }

  // ... and none of the three has a public field, which would be an exclusive
  // borrow by another name.
  for (const [path, contents] of sources) {
    for (const owner of ["NormalizedTranscript", "TranscriptIdentity", "TranscriptRow"]) {
      const declaration = new RegExp(`struct\\s+${owner}\\s*\\{([\\s\\S]*?)\\n\\}`, "u").exec(
        contents,
      );
      if (declaration === null) {
        continue;
      }
      assert.equal(
        /(^|[{,])\s*pub\s+[a-z_]/u.test(declaration[1]),
        false,
        `${path}: ${owner} declares a public field, which is an edit path into a source record`,
      );
    }
  }

  // The export is built from a projection and from nothing else. A second
  // parameter here — the original bytes, the sealed object, the vault — is the
  // change that would make `redacted_export_contains_no_original_bytes_or_metadata`
  // a claim about a function that no longer exists.
  const redaction = await readFile("crates/transcript/src/redaction.rs", "utf8");
  assert.match(
    redaction,
    /pub fn redacted_export\(projection: &RedactedProjection\) -> Vec<u8>/u,
    "redacted_export no longer takes exactly one projection",
  );
  assert.match(
    redaction,
    /pub fn project\(\s*transcript: &NormalizedTranscript,\s*profile: RedactionProfile,?\s*\) -> RedactedProjection/u,
    "project no longer borrows its source",
  );
});

// t068 section 5, `P2-U7`. Two things about the admission gate that a Rust test
// cannot see: that the capability has exactly one product constructor, and that
// the escape hatch the fault lane needs is compiled only by a test-only feature.
test("transcript_admission_gate_has_one_product_constructor", async () => {
  const admission = await readFile("crates/transcript/src/admission.rs", "utf8");
  const constructors = [...admission.matchAll(/\n    pub (?:const )?fn ([a-z_]+)\(/gu)].map(
    (match) => match[1],
  );
  assert.deepEqual(
    constructors.toSorted(),
    ["for_fault_injection_only", "open", "platforms", "receipt_digest"],
    "the AdmittedImport surface changed; a second way to obtain one is a second way past the gate",
  );
  assert.match(
    admission,
    /#\[cfg\(feature = "phase2-fault-injection"\)\]\s*\n\s*#\[must_use\]\s*\n\s*pub fn for_fault_injection_only/u,
    "the fault-lane capability constructor is no longer behind the test-only feature",
  );
  // It fabricates a capability, so it must not also be able to fabricate a
  // plausible receipt digest a caller could log as evidence of admission.
  assert.match(
    admission,
    /receipt_digest: String::from\("fault-injection-lane"\)/u,
    "the fault-lane capability no longer names itself in its receipt digest",
  );

  // Every gated entry point still demands the capability by type.
  const session = await readFile("crates/transcript/src/session.rs", "utf8");
  const vault = await readFile("crates/transcript/src/vault.rs", "utf8");
  for (const [name, contents, expected] of [
    ["session", session, 2],
    ["vault", vault, 1],
  ]) {
    assert.equal(
      [...contents.matchAll(/_admitted: &AdmittedImport/gu)].length,
      expected,
      `${name}.rs no longer takes the admission capability at every gated entry point`,
    );
  }
});

// t068 section 5, `P2-U7`: "reuse AEAD_CHUNKED_V2, do not create a new object
// format". The crate must therefore contain no cipher, no nonce schedule, and
// no second format label — it composes ADR-004's through the vault's public
// ingest and nothing else.
test("transcript_defines_no_second_object_format", async () => {
  const sources = await rustSources("crates/transcript/src");
  // Deliberately unanchored. A word boundary would let `LOCAL_BASE_NONCE`
  // through, which is exactly the spelling a second nonce schedule would
  // arrive under; the injection that found that gap is why this matches a
  // substring.
  const forbidden =
    /(chacha20|poly1305|xchacha|aead|nonce|wrapped_dek|cipher|hkdf|argon2|blake2)/iu;
  for (const [path, contents] of sources) {
    // The doc comments name the format they reuse, which is the point; the ban
    // is on primitives, so comment lines are stripped before the scan.
    const code = contents
      .split(/\r?\n/)
      .filter((line) => !/^\s*(\/\/|\*|\/\*)/u.test(line))
      .join("\n");
    const found = forbidden.exec(code);
    assert.equal(
      found,
      null,
      `${path}: academic-transcript names the cryptographic primitive ${found?.[0]}; it must compose AEAD_CHUNKED_V2 through academic-vault rather than build one`,
    );
  }

  // The one sealing call, and the two policy labels it is fixed to.
  const vault = await readFile("crates/transcript/src/vault.rs", "utf8");
  assert.equal(
    [...vault.matchAll(/vault\.ingest\(/gu)].length,
    1,
    "the transcript crate has more than one sealing call",
  );
  assert.match(vault, /TRANSCRIPT_CONFIDENTIALITY: Confidentiality = Confidentiality::Restricted/u);
  assert.match(vault, /TRANSCRIPT_RETENTION_CLASS: RetentionClass = RetentionClass::UserManaged/u);
});

test("projection_fault_harness_is_explicit_and_absent_from_product_defaults", async () => {
  // Exactly which crates declare the harness feature, and exactly what each
  // forwards. `academic-core` forwards to the three crates that own the named
  // failpoints; `academic-daemon` forwards to `academic-core` so the X1 exit
  // lane is one feature selection rather than five. Anything else appearing
  // here is a new fault surface and has to be declared in the open.
  assert.deepEqual(faultFeatureForwarding(workspacePackages), {
    "academic-core": [
      "academic-portability/phase1-fault-injection",
      "academic-projections/phase1-fault-injection",
      "academic-vault/phase1-fault-injection",
    ],
    "academic-daemon": ["academic-core/phase1-fault-injection"],
    "academic-portability": [],
    "academic-projections": [],
    "academic-test-support": [],
    "academic-vault": [],
  });
  // A *lane* may be a default feature; a *fault harness* may not. The only
  // default any of these crates is allowed to carry is the one that selects
  // which store lane it links, and `academic-portability` carries exactly that.
  const allowedDefaults = new Map([
    ["academic-store", ["bundled-sqlite"]],
    ["academic-portability", ["plaintext-portability"]],
  ]);
  for (const name of [
    "academic-core",
    "academic-daemon",
    "academic-portability",
    "academic-projections",
    "academic-test-support",
    "academic-vault",
  ]) {
    assert.deepEqual(
      packagesByName.get(name).features.default,
      allowedDefaults.get(name) ?? [],
      `${name} must not enable a fault lane by default`,
    );
  }

  // Nothing enables it in a default resolution, which is the property that
  // keeps a product build free of any crash switch.
  assert.deepEqual(
    packagesResolvingFaultFeature(workspacePackages, resolveNodesById),
    [],
    "default product resolution enabled a fault-injection lane",
  );
  for (const pkg of workspacePackages) {
    for (const dependency of pkg.dependencies) {
      assert.equal(
        dependency.features.includes(FAULT_FEATURE),
        false,
        `${pkg.name} enables phase1-fault-injection on ${dependency.name}`,
      );
    }
  }

  const [runnerSource, querySource] = await Promise.all([
    readFile("crates/projections/src/runner.rs", "utf8"),
    readFile("crates/projections/src/query.rs", "utf8"),
  ]);
  const assertFeatureGuarded = (source, declaration) => {
    const index = source.indexOf(declaration);
    assert.notEqual(index, -1, `missing fault-harness declaration: ${declaration}`);
    assert.match(
      source.slice(Math.max(0, index - 240), index),
      /#\[cfg\(feature = "phase1-fault-injection"\)\]/u,
      `${declaration} is not guarded by phase1-fault-injection`,
    );
  };
  for (const declaration of [
    "pub use fault_boundary::{",
    "pub fn rebuild_at_with_faults",
    "pub fn generation(",
    "pub fn audit_active_generation",
    "pub fn audit_generation_state_count",
  ]) {
    assertFeatureGuarded(runnerSource, declaration);
  }
  for (const declaration of [
    "pub use read_boundary::ProjectionReadBarrier",
    "pub fn graph_neighbors_with_barrier",
    "pub fn search_ranked_with_barrier",
    "pub fn exact_symbol_lookup_with_barrier",
  ]) {
    assertFeatureGuarded(querySource, declaration);
  }
});

test("fault_harness_and_dependency_gates_reject_their_violations", () => {
  // The two gates above assert facts about the real graph. On their own that is
  // an invariant held by coincidence: if the predicates were wrong, the real
  // graph would still satisfy them and both tests would still pass. These
  // fixtures put the predicates in front of graphs that violate the invariant
  // and require them to say so, which is what makes the assertions load-bearing.
  const workspacePackage = (name, dependencies = [], features = {}) => ({
    id: `${name}-id`,
    name,
    dependencies: dependencies.map(([dependencyName, kind]) => ({
      name: dependencyName,
      kind,
      source: null,
      features: [],
    })),
    features,
  });

  // (1) A product edge to the harness is caught; a dev edge is not, because a
  //     dev edge is exactly what a test target is allowed to have.
  const shippingHarnessEdge = [
    workspacePackage("academic-daemon", [["academic-test-support", null]]),
    workspacePackage("academic-test-support"),
  ];
  assert.deepEqual(
    packagesWithProductEdgeTo(shippingHarnessEdge, "academic-test-support"),
    ["academic-daemon"],
    "a normal dependency on the harness must be reported",
  );
  const buildHarnessEdge = [
    workspacePackage("academic-store", [["academic-test-support", "build"]]),
    workspacePackage("academic-test-support"),
  ];
  assert.deepEqual(
    packagesWithProductEdgeTo(buildHarnessEdge, "academic-test-support"),
    ["academic-store"],
    "a build-script dependency on the harness must be reported",
  );
  const devHarnessEdge = [
    workspacePackage("academic-daemon", [["academic-test-support", "dev"]]),
    workspacePackage("academic-test-support"),
  ];
  assert.deepEqual(
    packagesWithProductEdgeTo(devHarnessEdge, "academic-test-support"),
    [],
    "a dev dependency is not a product edge",
  );

  // (2) The product and dev graphs are read apart, so a new product edge cannot
  //     hide behind an expected dev one.
  const mixedEdges = workspacePackage("academic-daemon", [
    ["academic-core", null],
    ["academic-vault", "dev"],
  ]);
  assert.deepEqual(productDependencyNames(mixedEdges), ["academic-core"]);
  assert.deepEqual(devDependencyNames(mixedEdges), ["academic-vault"]);

  // (3) A crate that quietly gains the fault feature, or forwards it somewhere
  //     new, appears in the forwarding map rather than passing unnoticed.
  const smuggledFeature = [
    workspacePackage("academic-rpc", [], { default: [], [FAULT_FEATURE]: [] }),
    workspacePackage("academic-core", [], {
      default: [],
      [FAULT_FEATURE]: ["academic-vault/phase1-fault-injection"],
    }),
    workspacePackage("academic-domain", [], { default: [] }),
  ];
  assert.deepEqual(faultFeatureForwarding(smuggledFeature), {
    "academic-core": ["academic-vault/phase1-fault-injection"],
    "academic-rpc": [],
  });

  // (4) A default resolution that enables the fault lane is reported, which is
  //     the property that keeps a product build free of a crash switch.
  const resolved = new Map([
    ["academic-core-id", { features: ["default", FAULT_FEATURE] }],
    ["academic-domain-id", { features: [] }],
  ]);
  assert.deepEqual(
    packagesResolvingFaultFeature(
      [workspacePackage("academic-core"), workspacePackage("academic-domain")],
      resolved,
    ),
    ["academic-core"],
    "a resolution that enables the fault lane must be reported",
  );
  assert.deepEqual(
    packagesResolvingFaultFeature(
      [workspacePackage("academic-domain")],
      new Map([["academic-domain-id", { features: ["default"] }]]),
    ),
    [],
  );
});

test("synthetic_manifest_schema_rejects_non_synthetic_data", async () => {
  const [schemaText, fixtureBytes, storeSource, rpcSource, daemonMainSource] = await Promise.all([
    readFile("schemas/jsonschema/synthetic-ingest-manifest-v1.schema.json", "utf8"),
    readFile("schemas/fixtures/signed-batch-v2.json"),
    readFile("crates/store/src/lib.rs", "utf8"),
    readFile("crates/rpc/src/lib.rs", "utf8"),
    readFile("crates/daemon/src/main.rs", "utf8"),
  ]);
  const schema = JSON.parse(schemaText);
  const validate = new Ajv2020({ allErrors: true, strict: true }).compile(schema);
  const validManifest = structuredClone(schema.examples[0]);
  assert.equal(validate(validManifest), true, JSON.stringify(validate.errors));
  assert.equal(validManifest.fixture_byte_length, fixtureBytes.length);
  assert.equal(
    validManifest.fixture_sha256,
    createHash("sha256").update(fixtureBytes).digest("hex"),
  );
  for (const [field, value] of [
    ["data_class", "PERSONAL"],
    ["network_egress", "HTTPS"],
    ["storage_encryption", "SQLCIPHER"],
    ["production_data_allowed", true],
    ["product_network", "TCP"],
    ["fixture_sha256", "0".repeat(64)],
  ]) {
    const candidate = { ...validManifest, [field]: value };
    assert.equal(validate(candidate), false, `schema accepted ${field}=${String(value)}`);
  }
  for (const field of [
    "data_policy",
    "storage_mode",
    "storage_encryption",
    "product_network",
  ]) {
    const value = validManifest[field];
    assert.equal(storeSource.includes(value), true, `store policy omitted ${value}`);
    assert.equal(rpcSource.includes(value), true, `RPC policy omitted ${value}`);
  }

  const extractPolicy = (source) => {
    const values = {};
    for (const field of [
      "data_policy",
      "storage_mode",
      "storage_encryption",
      "production_data_allowed",
      "product_network",
    ]) {
      const match = source.match(
        new RegExp(`${field}: (?:"(?<string>[^"]+)"|(?<boolean>true|false)),`, "u"),
      );
      assert.ok(match, `missing frozen Rust policy field ${field}`);
      values[field] = match.groups.string ?? match.groups.boolean === "true";
    }
    return values;
  };
  const storePolicyBytes = Buffer.from(JSON.stringify(extractPolicy(storeSource)), "utf8");
  const rpcPolicyBytes = Buffer.from(JSON.stringify(extractPolicy(rpcSource)), "utf8");
  assert.deepEqual(rpcPolicyBytes, storePolicyBytes, "store/RPC policy bytes drifted");

  const extractBanner = (source) =>
    source.match(/pub const PHASE1_POLICY_BANNER: &str =\s*"(?<banner>[^"]+)";/u)?.groups
      ?.banner;
  assert.equal(extractBanner(storeSource), extractBanner(rpcSource), "policy banner bytes drifted");
  assert.match(
    daemonMainSource,
    /academic_rpc::PHASE1_POLICY_BANNER/u,
    "daemon must use the frozen RPC banner instead of duplicating it",
  );
});

function normalizeDependencyUse(dependency, packageName) {
  return {
    package: packageName,
    kind: dependency.kind ?? "normal",
    target: dependency.target,
    default_features: dependency.uses_default_features,
    features: dependency.features.toSorted(),
  };
}

test("dependency_license_and_source_receipt_is_complete", async () => {
  // Every admission receipt in `docs/security` is read off disk and keyed on the
  // task it names. `T177` did this for the duplicate-claim check; what used to be
  // written out here was nine edits at one place per crate -- a destructured name,
  // a read, a parse, two set constructions, a tuple filter, its length assertion,
  // two clauses of the incoming conjunction and one summand -- and six rebase races
  // in this run collided on exactly them. A receipt added later is bound by
  // `receiptFor` without anybody editing a list, and one nobody binds at all is
  // still read, still subtracted from the incoming set and still counted in the sum.
  const RECEIPT_DIRECTORY = "docs/security";
  const PHASE1_RECEIPT = "dependency-admission-phase1.json";
  const receiptFiles = (await readdir(RECEIPT_DIRECTORY))
    .filter((name) => name.startsWith("dependency-admission-") && name.endsWith(".json"))
    .toSorted();
  assert.ok(
    receiptFiles.includes(PHASE1_RECEIPT),
    `${RECEIPT_DIRECTORY} holds no phase 1 admission receipt`,
  );
  // The floor is what fails if the walk stops finding receipts: an empty read
  // would satisfy every assertion made over its result, which is the third of
  // this repository's three empty-scan shapes.
  assert.ok(
    receiptFiles.length >= 36,
    `expected every admission receipt to be read, found ${receiptFiles.length}`,
  );
  const [cargoLock, ...receiptTexts] = await Promise.all([
    readFile("Cargo.lock", "utf8"),
    ...receiptFiles.map((name) => readFile(join(RECEIPT_DIRECTORY, name), "utf8")),
  ]);
  const receipt = JSON.parse(receiptTexts[receiptFiles.indexOf(PHASE1_RECEIPT)]);
  assert.equal(receipt.receipt_version, 1);
  assert.equal(receipt.resolution_budget, 1);
  assert.deepEqual(receipt.lock_delta, {
    base_commit: "4f84bf78e51e04a0347c9475e08292da1c7d4608",
    incoming_package_tuple_count: 173,
    incoming_package_tuple_sha256: "4f370a5dd80938b0b6a00de809985f7ff32378a866ec570e13d9b650e7ce01c7",
    added_workspace_path_packages: [
      {
        name: "academic-store-platform",
        version: "0.1.0",
        source: null,
        checksum: null,
      },
    ],
  });
  const lockTuples = cargoLockPackageTuples(cargoLock);
  const platformTuples = lockTuples.filter(([name]) => name === "academic-store-platform");
  assert.deepEqual(platformTuples, [["academic-store-platform", "0.1.0", null, null]]);

  // Each phase 2 receipt's admitted set, path packages and lock tuples, derived
  // once. The file name has to agree with the task the receipt names, so a
  // receipt cannot be filed under another task's name -- that is the binding the
  // `assert.equal(<receipt>.task, …)` lines used to make for some of the receipts
  // and not others, and `T186` measured every one of those lines as carrying no
  // load: deleting one left its block reading the receipt by destructuring
  // position.
  const phase2Receipts = new Map();
  for (const [index, file] of receiptFiles.entries()) {
    if (file === PHASE1_RECEIPT) {
      continue;
    }
    const parsed = JSON.parse(receiptTexts[index]);
    assert.equal(typeof parsed.task, "string", `${file} names no task`);
    assert.equal(
      file,
      `dependency-admission-${parsed.task.toLowerCase().replace("p2-", "phase2-")}.json`,
      `${file} is filed under a name that is not ${parsed.task}'s`,
    );
    const admitted = new Set(
      parsed.admissions.map((admission) => `${admission.name}@${admission.version}`),
    );
    const pathPackages = new Set(
      parsed.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
    );
    const tuples = lockTuples.filter(
      ([name, version]) =>
        admitted.has(`${name}@${version}`) || pathPackages.has(`${name}@${version}`),
    );
    assert.equal(
      tuples.length,
      admitted.size + pathPackages.size,
      `a ${parsed.task} admitted package is missing from Cargo.lock`,
    );
    assert.equal(
      phase2Receipts.has(parsed.task),
      false,
      `${parsed.task} is named by two admission receipts`,
    );
    phase2Receipts.set(parsed.task, { receipt: parsed, admitted, pathPackages, tuples });
  }
  const bound = new Set();
  const receiptFor = (task) => {
    const found = phase2Receipts.get(task);
    assert.ok(found, `${RECEIPT_DIRECTORY} holds no admission receipt naming ${task}`);
    bound.add(task);
    return found;
  };

  // Every package `P2-K1` added is enumerated in its own receipt. Subtracting
  // exactly that set and re-checking the frozen Phase 1 digest proves two
  // things at once: no Phase 1 dependency moved, and nothing entered the lock
  // that is not covered by a reviewed admission receipt.
  const { receipt: keyReceipt } = receiptFor("P2-K1");

  // `P2-C7` is subtracted the same way and for the same reason. The two
  // receipts must not overlap: a package claimed by both would be subtracted
  // twice and the arithmetic below would hide a third, unreceipted arrival.
  const { receipt: scenarioReceipt } = receiptFor("P2-C7");

  // `P2-K4` admits no external crate at all; its receipt covers exactly one
  // workspace path package. It is subtracted here for the same reason as the
  // other two: a path package with no receipt would otherwise be counted as an
  // unreviewed arrival, and one with a receipt must not be counted twice.
  const { admitted: recoveryAdmitted } = receiptFor("P2-K4");
  assert.equal(recoveryAdmitted.size, 0, "P2-K4 must admit no external crate");

  // `P2-K5` admits no external crate either; its receipt covers exactly the one
  // workspace path package `academic-retention`, subtracted for the same reason.
  const { admitted: retentionAdmitted } = receiptFor("P2-K5");
  assert.equal(retentionAdmitted.size, 0, "P2-K5 must admit no external crate");

  // `P2-K6` likewise admits no external crate and adds only the receipt and
  // posture workspace boundary.
  const { admitted: admissionAdmitted } = receiptFor("P2-K6");
  assert.equal(admissionAdmitted.size, 0, "P2-K6 must admit no external crate");

  // `P2-G1` also reuses only previously admitted crates and adds the
  // socket-free `academic-policy` workspace boundary.
  const { admitted: policyAdmitted } = receiptFor("P2-G1");
  assert.equal(policyAdmitted.size, 0, "P2-G1 must admit no external crate");

  const {
    admitted: processAdmitted,
    pathPackages: processPathPackages,
  } = receiptFor("P2-G7");
  assert.equal(processAdmitted.size, 0, "P2-G7 must admit no external crate");
  assert.deepEqual(
    [...processPathPackages].toSorted(),
    [
      "academic-capture-client@0.1.0",
      "academic-connector@0.1.0",
      "academic-egress@0.1.0",
      "academic-export-job@0.1.0",
      "academic-indexer@0.1.0",
      "academic-repository-analyzer@0.1.0",
    ],
  );

  // `P2-U7` adds the transcript ingestion boundary and admits no external
  // crate. A PDF or OCR library would have been the obvious way to build it and
  // would have arrived here as an unreceipted package; the corpus is written by
  // a deterministic builder inside the crate instead, which is why this receipt
  // subtracts one path package and nothing else.
  const { admitted: transcriptAdmitted } = receiptFor("P2-U7");
  assert.equal(transcriptAdmitted.size, 0, "P2-U7 must admit no external crate");

  // `P2-G2` adds the DLP rulepack, the minimizer, the byte-accurate preview,
  // and the outbound seam as `academic-egress-boundary`, and admits no external
  // crate. It is a separate package from `P2-G7`'s `academic-egress` process
  // entry point, whose whole manifest and whole product source that task pins.
  const {
    receipt: egressReceipt,
    admitted: egressAdmitted,
    pathPackages: egressPathPackages,
  } = receiptFor("P2-G2");
  assert.equal(egressAdmitted.size, 0, "P2-G2 must admit no external crate");
  assert.deepEqual([
    ...egressPathPackages,
  ], ["academic-egress-boundary@0.1.0"]);
  assert.deepEqual(egressReceipt.summary.npm_additions, []);
  assert.equal(egressReceipt.summary.npm_install_scripts_added, false);
  // `P2-G4` adds the worker sandbox as `academic-worker` and admits no
  // external crate: `libc` and `windows-sys` are already in this lock at these
  // versions through earlier receipts, so what is new is two direct edges,
  // which the receipt records with their own owner, licence, feature set,
  // advisory path and trust-boundary justification.
  const {
    receipt: sandboxReceipt,
    admitted: sandboxAdmitted,
    pathPackages: sandboxPathPackages,
  } = receiptFor("P2-G4");
  assert.equal(sandboxAdmitted.size, 0, "P2-G4 must admit no external crate");
  assert.deepEqual([...sandboxPathPackages], ["academic-worker@0.1.0"]);
  assert.deepEqual(sandboxReceipt.summary.npm_additions, []);
  assert.equal(sandboxReceipt.summary.npm_install_scripts_added, false);
  // Each direct edge the task adds names a version that is already in the lock,
  // and names it exactly, so a bump cannot ride in under a receipt that says
  // nothing changed.
  assert.deepEqual(Object.keys(sandboxReceipt.direct_edge_review).toSorted(), [
    "libc",
    "windows-sys",
  ]);
  for (const [name, edge] of Object.entries(sandboxReceipt.direct_edge_review)) {
    assert.equal(edge.already_in_lock, true, `${name} is claimed as new`);
    assert.equal(
      lockTuples.some(([lockName, version]) => lockName === name && version === edge.version),
      true,
      `${name}@${edge.version} is not the version in Cargo.lock`,
    );
    for (const field of [
      "owner",
      "license",
      "features",
      "why_this_dependency_belongs_inside_its_trust_boundary",
      "advisory_path",
    ]) {
      assert.equal(
        typeof edge[field] === "string" && edge[field].length > 0,
        true,
        `${name}'s admission receipt has no ${field}`,
      );
    }
  }

  // `P2-G5` adds the untrusted-content boundary as `academic-untrusted-content`
  // and admits no external crate: its product edges are `academic-egress-boundary`,
  // `sha2` and `thiserror`, and its dev edge is `academic-policy`, all four
  // already in this lock through earlier receipts.
  const {
    receipt: untrustedReceipt,
    admitted: untrustedAdmitted,
    pathPackages: untrustedPathPackages,
  } = receiptFor("P2-G5");
  assert.equal(untrustedAdmitted.size, 0, "P2-G5 must admit no external crate");
  assert.deepEqual([...untrustedPathPackages], ["academic-untrusted-content@0.1.0"]);
  assert.deepEqual(untrustedReceipt.summary.npm_additions, []);
  assert.equal(untrustedReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(untrustedReceipt.direct_workspace_dependencies, {});

  // `P2-M1` adds the model-run provenance and calibration boundary as
  // `academic-model-run` and admits no external crate: its product edges are
  // `academic-domain`, `academic-policy`, `sha2` and `thiserror`, and its dev
  // edge is `trybuild`, all five already in this lock through earlier receipts.
  const {
    receipt: modelRunReceipt,
    admitted: modelRunAdmitted,
    pathPackages: modelRunPathPackages,
  } = receiptFor("P2-M1");
  assert.equal(modelRunAdmitted.size, 0, "P2-M1 must admit no external crate");
  assert.deepEqual([...modelRunPathPackages], ["academic-model-run@0.1.0"]);
  assert.deepEqual(modelRunReceipt.summary.npm_additions, []);
  assert.equal(modelRunReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(modelRunReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-policy",
  ]);

  // `P2-M2` adds the proposal boundary, risk tiers and review queue as
  // `academic-proposal` and admits no external crate: its product edges are
  // `academic-domain`, `sha2` and `thiserror`, and its dev edges are
  // `academic-domain`, `serde_json`, `trybuild` and `uuid`, all already in this
  // lock through earlier receipts.
  const {
    receipt: proposalReceipt,
    admitted: proposalAdmitted,
    pathPackages: proposalPathPackages,
  } = receiptFor("P2-M2");
  assert.equal(proposalAdmitted.size, 0, "P2-M2 must admit no external crate");
  assert.deepEqual([...proposalPathPackages], ["academic-proposal@0.1.0"]);
  assert.deepEqual(proposalReceipt.summary.npm_additions, []);
  assert.equal(proposalReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(proposalReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
  ]);



  // `P2-U4` adds the attempt ledger and the two §28 engines and admits no
  // external crate. A decimal or big-rational library would have been the
  // obvious way to build a grade-point average and would have arrived here as
  // an unreceipted package; the arithmetic is written over the canonical
  // `Decimal` instead, which is why this receipt subtracts one path package and
  // nothing else.
  const {
    admitted: recordAdmitted,
    pathPackages: recordPathPackages,
  } = receiptFor("P2-U4");
  assert.equal(recordAdmitted.size, 0, "P2-U4 must admit no external crate");
  assert.deepEqual([...recordPathPackages], ["academic-record@0.1.0"]);

  // `P2-G6` adds the consent ledger as `academic-consent` and admits no
  // external crate: its product edges are `academic-domain` and `thiserror`,
  // and its dev edges are `academic-domain`, `academic-retention` and
  // `trybuild`, all already in this lock through earlier receipts.
  const {
    receipt: consentReceipt,
    admitted: consentAdmitted,
    pathPackages: consentPathPackages,
  } = receiptFor("P2-G6");
  assert.equal(consentAdmitted.size, 0, "P2-G6 must admit no external crate");
  assert.deepEqual([...consentPathPackages], ["academic-consent@0.1.0"]);
  assert.deepEqual(consentReceipt.summary.npm_additions, []);
  assert.equal(consentReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(consentReceipt.direct_workspace_dependencies, {});

  // `P2-L1` adds the capture device gate as `academic-capture-gate` and admits
  // no external crate: `libc` and `windows-sys` are the pinned versions
  // `academic-worker`'s native lane already admitted, and every other edge is
  // a workspace path crate an earlier receipt covers.
  const {
    receipt: captureReceipt,
    admitted: captureAdmitted,
    pathPackages: capturePathPackages,
  } = receiptFor("P2-L1");
  assert.equal(captureAdmitted.size, 0, "P2-L1 must admit no external crate");
  assert.deepEqual([...capturePathPackages], ["academic-capture-gate@0.1.0"]);
  assert.deepEqual(captureReceipt.summary.npm_additions, []);
  assert.equal(captureReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(captureReceipt.direct_workspace_dependencies, {});

  // `P2-X1` adds one workspace path package, `academic-desktop`, and admits no
  // external crate: its product edges are `academic-rpc` and `thiserror` and
  // its dev edges are `serde_json` and `trybuild`, all four already in this
  // lock through earlier receipts. The Tauri runtime is not linked, and the
  // receipt records the measurement that decided it.
  const {
    receipt: desktopReceipt,
    admitted: desktopAdmitted,
    pathPackages: desktopPathPackages,
  } = receiptFor("P2-X1");
  assert.equal(desktopAdmitted.size, 0, "P2-X1 must admit no external crate");
  assert.deepEqual([...desktopPathPackages], ["academic-desktop@0.1.0"]);
  assert.deepEqual(desktopReceipt.summary.npm_additions, []);
  assert.equal(desktopReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(desktopReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-rpc",
  ]);
  // The two vendored Tauri schemas are data, not dependencies, and the receipt
  // says so with the digests `capability-snapshot.test.ts` pins.
  assert.deepEqual(
    desktopReceipt.vendored_data.map((entry) => [entry.path, entry.is_a_dependency]),
    [
      ["schemas/tauri/config-2.11.5.schema.json", false],
      ["schemas/tauri/capability-2.9.3.schema.json", false],
    ],
  );
  for (const entry of desktopReceipt.vendored_data) {
    assert.equal(
      createHash("sha256").update(await readFile(entry.path)).digest("hex"),
      entry.sha256,
      `${entry.path} does not match the digest its admission receipt records`,
    );
  }

  // `P2-R1` adds one workspace path package, `academic-repository`, and admits
  // no external crate: its product edges are `academic-crypto`,
  // `academic-policy`, `academic-untrusted-content`, `sha2`, `thiserror` and
  // `zeroize`, and its dev edge is `tempfile`, all already in this lock through
  // earlier receipts.
  const {
    receipt: repositoryReceipt,
    admitted: repositoryAdmitted,
    pathPackages: repositoryPathPackages,
  } = receiptFor("P2-R1");
  assert.equal(repositoryAdmitted.size, 0, "P2-R1 must admit no external crate");
  assert.deepEqual([...repositoryPathPackages], ["academic-repository@0.1.0"]);
  assert.deepEqual(repositoryReceipt.summary.npm_additions, []);
  assert.equal(repositoryReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(repositoryReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-crypto",
    "academic-policy",
    "academic-untrusted-content",
  ]);

  // `P2-R2` adds one workspace path package, `academic-repository-analysis`,
  // and admits no external crate: its product edges are `academic-model-run`,
  // `academic-policy`, `academic-repository`, `academic-untrusted-content` and
  // `thiserror`, all already in this lock through earlier receipts, and it
  // declares no dev edge. Section 17.3 names AST indexing, which is where a
  // parser generator would normally enter; none is admitted, and the receipt
  // says why in `no_parser_dependency_note`.
  const {
    receipt: analysisReceipt,
    admitted: analysisAdmitted,
    pathPackages: analysisPathPackages,
  } = receiptFor("P2-R2");
  assert.equal(analysisAdmitted.size, 0, "P2-R2 must admit no external crate");
  assert.deepEqual([...analysisPathPackages], ["academic-repository-analysis@0.1.0"]);
  assert.deepEqual(analysisReceipt.summary.npm_additions, []);
  assert.equal(analysisReceipt.summary.npm_install_scripts_added, false);
  assert.equal(analysisReceipt.summary.linked_into_binary_count, 0);
  assert.equal(analysisReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof analysisReceipt.no_parser_dependency_note, "string");
  assert.deepEqual(Object.keys(analysisReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-model-run",
    "academic-policy",
    "academic-repository",
    "academic-untrusted-content",
  ]);

  // `P2-R3` adds one workspace path package, `academic-repository-correlation`,
  // and admits no external crate: its product edges are `academic-domain`,
  // `academic-ledger`, `academic-repository`, `academic-repository-analysis`
  // and `thiserror`, and its dev edges are `academic-model-run`,
  // `academic-policy` and `academic-untrusted-content`, all already in this
  // lock through earlier receipts. The `academic-ledger` edge is the one that
  // needs a reason and the receipt carries it: section 30.3's six authority
  // rows are already implemented there, and this task adds the qualifier those
  // rows state in words rather than a second rank table.
  const {
    receipt: correlationReceipt,
    admitted: correlationAdmitted,
    pathPackages: correlationPathPackages,
  } = receiptFor("P2-R3");
  assert.equal(correlationAdmitted.size, 0, "P2-R3 must admit no external crate");
  assert.deepEqual([...correlationPathPackages], ["academic-repository-correlation@0.1.0"]);
  assert.deepEqual(correlationReceipt.summary.npm_additions, []);
  assert.equal(correlationReceipt.summary.npm_install_scripts_added, false);
  assert.equal(correlationReceipt.summary.linked_into_binary_count, 0);
  assert.equal(correlationReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof correlationReceipt.no_second_resolver_note, "string");
  assert.deepEqual(Object.keys(correlationReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-ledger",
    "academic-repository",
    "academic-repository-analysis",
  ]);
  assert.deepEqual(Object.keys(correlationReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-model-run",
    "academic-policy",
    "academic-untrusted-content",
  ]);

  // `P2-R4` adds one workspace path package, `academic-repository-classification`,
  // and admits no external crate: its product edges are `academic-domain`,
  // `academic-policy`, `academic-repository-analysis`,
  // `academic-repository-correlation` and `thiserror`, and its dev edges are
  // `academic-model-run`, `academic-repository`, `academic-untrusted-content`
  // and `trybuild`, all already in this lock through earlier receipts. The
  // `academic-policy` edge is the one that needs a reason and the receipt
  // carries it: a materialized requirement's identity is a digest of the four
  // facts section 18.4 binds, because those four joined and truncated to
  // `RequirementId`'s 64 bytes collide.
  const {
    receipt: classificationReceipt,
    admitted: classificationAdmitted,
    pathPackages: classificationPathPackages,
  } = receiptFor("P2-R4");
  assert.equal(classificationAdmitted.size, 0, "P2-R4 must admit no external crate");
  assert.deepEqual(
    [...classificationPathPackages],
    ["academic-repository-classification@0.1.0"],
  );
  assert.deepEqual(classificationReceipt.summary.npm_additions, []);
  assert.equal(classificationReceipt.summary.npm_install_scripts_added, false);
  assert.equal(classificationReceipt.summary.linked_into_binary_count, 0);
  assert.equal(classificationReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof classificationReceipt.no_second_ladder_note, "string");
  assert.deepEqual(Object.keys(classificationReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-policy",
    "academic-repository-analysis",
    "academic-repository-correlation",
  ]);
  assert.deepEqual(Object.keys(classificationReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-model-run",
    "academic-repository",
    "academic-untrusted-content",
    "trybuild",
  ]);

  // `P2-L2` adds one workspace path package, `academic-capture`, and admits no
  // external crate: its product edges are `academic-consent`, `academic-domain`
  // and `thiserror`, and its one dev edge is `tempfile`, all already in this
  // lock through earlier receipts. It declares no binary target, and it has no
  // edge of any kind to `academic-capture-gate`.
  const {
    receipt: captureSubsystemReceipt,
    admitted: captureSubsystemAdmitted,
    pathPackages: captureSubsystemPathPackages,
  } = receiptFor("P2-L2");
  assert.equal(captureSubsystemAdmitted.size, 0, "P2-L2 must admit no external crate");
  assert.deepEqual([...captureSubsystemPathPackages], ["academic-capture@0.1.0"]);
  assert.deepEqual(captureSubsystemReceipt.summary.npm_additions, []);
  assert.equal(captureSubsystemReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(
    Object.keys(captureSubsystemReceipt.direct_workspace_dependencies).toSorted(),
    ["academic-consent", "academic-domain"],
  );
  // `P2-U6` adds one workspace path package, `academic-ingestion`, and admits
  // no external crate: its product edges are `academic-domain`,
  // `academic-untrusted-content` and `thiserror` and its dev edge is
  // `trybuild`, all four already in this lock through earlier receipts. No
  // HTTP client, TLS stack, browser driver or media decoder is linked, and the
  // receipt records why the conditional fetch is a caller-supplied trait.
  const {
    receipt: ingestionReceipt,
    admitted: ingestionAdmitted,
    pathPackages: ingestionPathPackages,
  } = receiptFor("P2-U6");
  assert.equal(ingestionAdmitted.size, 0, "P2-U6 must admit no external crate");
  assert.deepEqual([...ingestionPathPackages], ["academic-ingestion@0.1.0"]);
  assert.deepEqual(ingestionReceipt.summary.npm_additions, []);
  assert.equal(ingestionReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(ingestionReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-untrusted-content",
  ]);
  assert.deepEqual(ingestionReceipt.vendored_data, []);

  // `P2-U1` adds one workspace path package, `academic-curriculum`, and admits
  // no external crate: its product edges are `academic-domain`,
  // `academic-ingestion` and `thiserror` and its dev edges are
  // `academic-domain` and `trybuild`, all already in this lock through earlier
  // receipts. No store, vault, crypto, policy or transport crate is linked at
  // any feature setting, and the receipt records why the aggregate boundary
  // sits above the ingestion pipeline rather than inside it.
  const {
    receipt: curriculumReceipt,
    admitted: curriculumAdmitted,
    pathPackages: curriculumPathPackages,
  } = receiptFor("P2-U1");
  assert.equal(curriculumAdmitted.size, 0, "P2-U1 must admit no external crate");
  assert.deepEqual([...curriculumPathPackages], ["academic-curriculum@0.1.0"]);
  assert.deepEqual(curriculumReceipt.summary.npm_additions, []);
  assert.equal(curriculumReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(curriculumReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-ingestion",
  ]);
  assert.deepEqual(curriculumReceipt.vendored_data, []);

  // `P2-U2` adds `academic-requirement` and no external crate. The rule DSL is
  // a boundary above the ingestion pipeline and beside the curriculum
  // aggregates: it links neither a writer nor a model, which is what
  // `production_audit_no_llm` rests on.
  const {
    receipt: requirementReceipt,
    admitted: requirementAdmitted,
    pathPackages: requirementPathPackages,
  } = receiptFor("P2-U2");
  assert.equal(requirementAdmitted.size, 0, "P2-U2 must admit no external crate");
  assert.deepEqual([...requirementPathPackages], ["academic-requirement@0.1.0"]);
  assert.deepEqual(requirementReceipt.summary.npm_additions, []);
  assert.equal(requirementReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(requirementReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-ingestion",
  ]);
  assert.deepEqual(requirementReceipt.vendored_data, []);

  // `P2-L3` adds one workspace path package, `academic-transcription`, and
  // admits no external crate: its six product edges and its six dev edges are
  // all in this lock through earlier receipts. No speech engine, audio decoder,
  // HTTP client or TLS stack is linked, and the receipt records why a provider
  // is a caller-supplied trait.
  const {
    receipt: transcriptionReceipt,
    admitted: transcriptionAdmitted,
    pathPackages: transcriptionPathPackages,
  } = receiptFor("P2-L3");
  assert.equal(transcriptionAdmitted.size, 0, "P2-L3 must admit no external crate");
  assert.deepEqual([...transcriptionPathPackages], ["academic-transcription@0.1.0"]);
  assert.deepEqual(transcriptionReceipt.summary.npm_additions, []);
  assert.equal(transcriptionReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(transcriptionReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-domain",
    "academic-egress-boundary",
    "academic-model-run",
    "academic-proposal",
    "academic-untrusted-content",
  ]);
  assert.deepEqual(transcriptionReceipt.vendored_data, []);

  // `P2-X7` adds `academic-evidence-center` and no external crate. The evidence
  // and correction centre sits above the proposal queue, the ingestion pipeline
  // and the domain vocabulary and links no writer, so it claims no migration
  // number and persists nothing.
  const {
    receipt: centerReceipt,
    admitted: centerAdmitted,
    pathPackages: centerPathPackages,
  } = receiptFor("P2-X7");
  assert.equal(centerAdmitted.size, 0, "P2-X7 must admit no external crate");
  assert.deepEqual([...centerPathPackages], ["academic-evidence-center@0.1.0"]);
  assert.deepEqual(centerReceipt.summary.npm_additions, []);
  assert.equal(centerReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(centerReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-ingestion",
    "academic-proposal",
  ]);
  assert.deepEqual(centerReceipt.vendored_data, []);

  // `P2-X2` adds one workspace path package, `academic-home`, and admits no
  // external crate: its two product edges and its three dev edges are all in
  // this lock through earlier receipts. No knowledge-state and no freshness
  // edge, which is the mastery claim; no policy and no egress-boundary edge,
  // which is the difference between a permission status and a permission.
  const {
    receipt: homeReceipt,
    admitted: homeAdmitted,
    pathPackages: homePathPackages,
  } = receiptFor("P2-X2");
  assert.equal(homeAdmitted.size, 0, "P2-X2 must admit no external crate");
  assert.deepEqual([...homePathPackages], ["academic-home@0.1.0"]);
  assert.deepEqual(homeReceipt.summary.npm_additions, []);
  assert.equal(homeReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(homeReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-consent",
    "academic-domain",
  ]);
  assert.deepEqual(homeReceipt.vendored_data, []);

  // `P2-N8` adds one workspace path package, `academic-what-if`, and admits no
  // external crate: its eight product edges and its ten dev edges are all in
  // this lock through earlier receipts. It is the composition task of the
  // knowledge slice, so its product edge set is the widest in that slice, and
  // the two absences that carry claims are named rather than left to be
  // inferred: no store or store-platform edge of any kind, which is `INV-C-009`
  // as a graph fact, and no audit edge of any kind, which is what keeps the
  // hypothetical graduation mode away from `P2-U3`'s three-gate rule.
  const {
    receipt: whatIfReceipt,
    admitted: whatIfAdmitted,
    pathPackages: whatIfPathPackages,
  } = receiptFor("P2-N8");
  assert.equal(whatIfAdmitted.size, 0, "P2-N8 must admit no external crate");
  assert.deepEqual([...whatIfPathPackages], ["academic-what-if@0.1.0"]);
  assert.deepEqual(whatIfReceipt.summary.npm_additions, []);
  assert.equal(whatIfReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(whatIfReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-critical-path",
    "academic-curriculum",
    "academic-domain",
    "academic-offering",
    "academic-proposal",
    "academic-record",
    "academic-review",
    "academic-scenario",
  ]);
  for (const forbidden of [
    "academic-store",
    "academic-store-platform",
    "academic-audit",
  ]) {
    assert.equal(
      Object.hasOwn(whatIfReceipt.direct_workspace_dependencies, forbidden),
      false,
      `P2-N8 must not claim ${forbidden} as a product edge`,
    );
    assert.equal(
      Object.hasOwn(whatIfReceipt.dev_workspace_dependencies, forbidden),
      false,
      `P2-N8 must not claim ${forbidden} as a dev edge`,
    );
  }
  assert.deepEqual(whatIfReceipt.vendored_data, []);

  // `P2-L4` adds one workspace path package, `academic-lecture-document`, and
  // admits no external crate: its four product edges and its eight dev edges are
  // all in this lock through earlier receipts. No PDF engine, layout engine,
  // font or image decoder is linked, and the receipt records why a render
  // measurement is a value the caller supplies.
  const {
    receipt: lectureReceipt,
    admitted: lectureAdmitted,
    pathPackages: lecturePathPackages,
  } = receiptFor("P2-L4");
  assert.equal(lectureAdmitted.size, 0, "P2-L4 must admit no external crate");
  assert.deepEqual([...lecturePathPackages], ["academic-lecture-document@0.1.0"]);
  assert.deepEqual(lectureReceipt.summary.npm_additions, []);
  assert.equal(lectureReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(Object.keys(lectureReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-domain",
    "academic-model-run",
    "academic-transcription",
  ]);
  assert.deepEqual(lectureReceipt.vendored_data, []);


  // `P2-N2` adds one workspace path package, `academic-knowledge-state`, and
  // admits no external crate: its product edges are `academic-domain`,
  // `academic-ledger`, `academic-lecture-document`,
  // `academic-repository-classification`, `serde` and `thiserror`, and its dev
  // edges are the two fixture chains plus `serde_json`, `tempfile`, `trybuild`
  // and `uuid`, all already in this lock through earlier receipts. The
  // `academic-ledger` edge is the one that needs a reason and the receipt
  // carries it: section 13.4's conflict is the one section 30.3 already
  // resolved, so the card this crate opens carries `P2-M3`'s
  // `NEW_EVIDENCE_CONFLICT` rather than a second token.
  const {
    receipt: knowledgeStateReceipt,
    admitted: knowledgeStateAdmitted,
    pathPackages: knowledgeStatePathPackages,
  } = receiptFor("P2-N2");
  assert.equal(knowledgeStateAdmitted.size, 0, "P2-N2 must admit no external crate");
  assert.deepEqual([...knowledgeStatePathPackages], ["academic-knowledge-state@0.1.0"]);
  assert.deepEqual(knowledgeStateReceipt.summary.npm_additions, []);
  assert.equal(knowledgeStateReceipt.summary.npm_install_scripts_added, false);
  assert.equal(knowledgeStateReceipt.summary.linked_into_binary_count, 0);
  assert.equal(knowledgeStateReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof knowledgeStateReceipt.no_second_ladder_note, "string");
  assert.deepEqual(Object.keys(knowledgeStateReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-lecture-document",
    "academic-ledger",
    "academic-repository-classification",
  ]);
  assert.deepEqual(Object.keys(knowledgeStateReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-consent",
    "academic-model-run",
    "academic-policy",
    "academic-repository",
    "academic-repository-analysis",
    "academic-repository-correlation",
    "academic-transcription",
    "academic-untrusted-content",
    "serde_json",
    "tempfile",
    "trybuild",
    "uuid",
  ]);
  // This block reads last, so it is the only one that can name every earlier
  // receipt's sets. `3f8859f` measured why the direction matters: a block that
  // names a set declared below it fails with a temporal-dead-zone
  // `ReferenceError` rather than with a duplicate-claim message.
  // `P2-R5` adds one workspace path package, `academic-repository-competency`,
  // and admits no external crate: its product edges are `academic-domain`,
  // `academic-policy`, `academic-repository-analysis`,
  // `academic-repository-classification`, `academic-repository-correlation` and
  // `thiserror`, and its dev edges are `academic-model-run`,
  // `academic-repository`, `academic-untrusted-content` and `trybuild`, all
  // already in this lock through earlier receipts. The `academic-policy` edge is
  // the one that needs a reason and the receipt carries it: each of the two
  // claims carries a domain-separated digest identity, because the facts they
  // bind joined and truncated to `ClaimId`'s 64 bytes collide.
  const {
    receipt: competencyReceipt,
    admitted: competencyAdmitted,
    pathPackages: competencyPathPackages,
  } = receiptFor("P2-R5");
  assert.equal(competencyAdmitted.size, 0, "P2-R5 must admit no external crate");
  assert.deepEqual([...competencyPathPackages], ["academic-repository-competency@0.1.0"]);
  assert.deepEqual(competencyReceipt.summary.npm_additions, []);
  assert.equal(competencyReceipt.summary.npm_install_scripts_added, false);
  assert.equal(competencyReceipt.summary.linked_into_binary_count, 0);
  assert.equal(competencyReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof competencyReceipt.no_second_ladder_note, "string");
  assert.deepEqual(Object.keys(competencyReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-policy",
    "academic-repository-analysis",
    "academic-repository-classification",
    "academic-repository-correlation",
  ]);
  assert.deepEqual(Object.keys(competencyReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-model-run",
    "academic-repository",
    "academic-untrusted-content",
    "trybuild",
  ]);

  // `P2-Y1` adds one workspace path package, `academic-competency`, and admits
  // no external crate: its product edges are `academic-domain`,
  // `academic-knowledge-state`, `academic-repository-competency`, `serde` and
  // `thiserror`, and its dev edges are `P2-R5`'s own corpus chain plus
  // `academic-policy`, `serde_json`, `trybuild` and `uuid`, all already in this
  // lock through earlier receipts. The `academic-knowledge-state` edge is the
  // one that needs a reason and the receipt carries it: section 24.3's
  // `dependency를 사용했다는 이유만으로 competency를 채우지 않는다` is section
  // 13.2's own ceiling read through `EvidenceKind::ceiling`, so the refusal is
  // that table's answer rather than a second rule in this crate.
  const {
    receipt: competencyModelReceipt,
    admitted: competencyModelAdmitted,
    pathPackages: competencyModelPathPackages,
  } = receiptFor("P2-Y1");
  assert.equal(competencyModelAdmitted.size, 0, "P2-Y1 must admit no external crate");
  assert.deepEqual([...competencyModelPathPackages], ["academic-competency@0.1.0"]);
  assert.deepEqual(competencyModelReceipt.summary.npm_additions, []);
  assert.equal(competencyModelReceipt.summary.npm_install_scripts_added, false);
  assert.equal(competencyModelReceipt.summary.linked_into_binary_count, 0);
  assert.equal(competencyModelReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof competencyModelReceipt.no_second_vocabulary_note, "string");
  assert.deepEqual(Object.keys(competencyModelReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-knowledge-state",
    "academic-repository-competency",
  ]);
  assert.deepEqual(Object.keys(competencyModelReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-model-run",
    "academic-policy",
    "academic-repository",
    "academic-repository-analysis",
    "academic-repository-classification",
    "academic-repository-correlation",
    "academic-untrusted-content",
    "serde_json",
    "trybuild",
    "uuid",
  ]);

  // `P2-Y2` adds one workspace path package, `academic-role-profile`, and
  // admits no external crate: its product edges are `academic-competency`,
  // `academic-domain`, `academic-ingestion`, `serde` and `thiserror`, and its
  // dev edges are `academic-competency` again -- for `trybuild` -- plus
  // `serde_json` and `trybuild`, all already in this lock through earlier
  // receipts. The `academic-ingestion` edge is the one that needs a reason and
  // the receipt carries it: section 24.2's `validAt` is the document's own date
  // rather than the clock, and `P2-U6`'s `dating` module already owns that
  // separation with a `Date` that has no constructor taking an instant, so
  // reusing it is what keeps *this crate cannot ask what time it is* true after
  // a dated field arrives.
  const {
    receipt: roleBundleReceipt,
    admitted: roleBundleAdmitted,
    pathPackages: roleBundlePathPackages,
  } = receiptFor("P2-Y2");
  assert.equal(roleBundleAdmitted.size, 0, "P2-Y2 must admit no external crate");
  assert.deepEqual([...roleBundlePathPackages], ["academic-role-profile@0.1.0"]);
  assert.deepEqual(roleBundleReceipt.summary.npm_additions, []);
  assert.equal(roleBundleReceipt.summary.npm_install_scripts_added, false);
  assert.equal(roleBundleReceipt.summary.linked_into_binary_count, 0);
  assert.equal(roleBundleReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof roleBundleReceipt.no_second_vocabulary_note, "string");
  assert.deepEqual(Object.keys(roleBundleReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-competency",
    "academic-domain",
    "academic-ingestion",
  ]);
  assert.deepEqual(Object.keys(roleBundleReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-competency",
    "serde_json",
    "trybuild",
  ]);

  // `P2-X5` adds one workspace path package, `academic-cs-map`, and admits no
  // external crate: its product edges are `academic-domain`, `serde` and
  // `thiserror`, and its dev edges are `academic-domain`, `serde_json`,
  // `trybuild` and `uuid`, all already in this lock through earlier receipts.
  // The edge that needs a reason is the one it does **not** have: no
  // `academic-knowledge-state`, no `academic-critical-path` and no
  // `academic-blind-spot`, so a fill, a halo and a gap glyph are displays of
  // values a caller supplies rather than computations this surface could do
  // differently from the crate that owns them.
  const {
    receipt: csMapReceipt,
    admitted: csMapAdmitted,
    pathPackages: csMapPathPackages,
  } = receiptFor("P2-X5");
  assert.equal(csMapAdmitted.size, 0, "P2-X5 must admit no external crate");
  assert.deepEqual([...csMapPathPackages], ["academic-cs-map@0.1.0"]);
  assert.deepEqual(csMapReceipt.summary.npm_additions, []);
  assert.equal(csMapReceipt.summary.npm_install_scripts_added, false);
  assert.equal(csMapReceipt.summary.linked_into_binary_count, 0);
  assert.equal(csMapReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof csMapReceipt.no_second_vocabulary_note, "string");
  assert.deepEqual(Object.keys(csMapReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
  ]);
  assert.deepEqual(Object.keys(csMapReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-domain",
    "serde_json",
    "trybuild",
    "uuid",
  ]);
  // `P2-Y3` adds one workspace path package, `academic-readiness`, and admits
  // no external crate: its product edges are `academic-competency`,
  // `academic-domain`, `academic-knowledge-state`, `academic-role-profile`,
  // `serde` and `thiserror`, and its dev edges are the `P2-R1`--`P2-R5` chain,
  // `P2-U3`'s fixture module and `P2-P1`'s exporter, all already in this lock
  // through earlier receipts. The `academic-export` edge is the one that needs
  // a reason and the receipt carries it: `non_guarantee_disclaimer_survives_export`
  // is a claim about the bundle a user keeps when this product is gone, so it
  // is measured against that crate's own writer and reader rather than against
  // a copy of them -- and it stays a **dev** edge, because a readiness view
  // whose product closure held the exporter could carry a notice the exporter
  // minted instead of one the document carries.
  const {
    receipt: readinessReceipt,
    admitted: readinessAdmitted,
    pathPackages: readinessPathPackages,
  } = receiptFor("P2-Y3");
  assert.equal(readinessAdmitted.size, 0, "P2-Y3 must admit no external crate");
  assert.deepEqual([...readinessPathPackages], ["academic-readiness@0.1.0"]);
  assert.deepEqual(readinessReceipt.summary.npm_additions, []);
  assert.equal(readinessReceipt.summary.npm_install_scripts_added, false);
  assert.equal(readinessReceipt.summary.linked_into_binary_count, 0);
  assert.equal(readinessReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof readinessReceipt.no_second_vocabulary_note, "string");
  assert.deepEqual(Object.keys(readinessReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-competency",
    "academic-domain",
    "academic-knowledge-state",
    "academic-role-profile",
  ]);
  assert.deepEqual(Object.keys(readinessReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-audit",
    "academic-competency",
    "academic-export",
    "academic-ingestion",
    "academic-model-run",
    "academic-policy",
    "academic-record",
    "academic-repository",
    "academic-repository-analysis",
    "academic-repository-classification",
    "academic-repository-competency",
    "academic-repository-correlation",
    "academic-requirement",
    "academic-role-profile",
    "academic-untrusted-content",
    "serde_json",
    "trybuild",
    "uuid",
  ]);
  // `P2-R6` adds one workspace path package, `academic-build-learn`, and admits
  // no external crate: its product edges are `academic-critical-path`,
  // `academic-curriculum`, `academic-domain`, `academic-gap`,
  // `academic-knowledge-state`, `academic-repository-classification`, `serde`
  // and `thiserror`, and its dev edges are the `P2-N5` fixture chain plus
  // `serde_json`, `tempfile`, `trybuild` and `uuid`, all already in this lock
  // through earlier receipts. The edge that needs a reason is the one it does
  // **not** have: no `academic-store`, so a build-to-learn plan cannot reach the
  // canonical writer and this task adds no migration -- a plan is recomputed
  // from a goal, an overlay and a snapshot rather than stored beside them.
  const {
    receipt: buildLearnReceipt,
    admitted: buildLearnAdmitted,
    pathPackages: buildLearnPathPackages,
  } = receiptFor("P2-R6");
  assert.equal(buildLearnAdmitted.size, 0, "P2-R6 must admit no external crate");
  assert.deepEqual([...buildLearnPathPackages], ["academic-build-learn@0.1.0"]);
  assert.deepEqual(buildLearnReceipt.summary.npm_additions, []);
  assert.equal(buildLearnReceipt.summary.npm_install_scripts_added, false);
  assert.equal(buildLearnReceipt.summary.linked_into_binary_count, 0);
  assert.equal(buildLearnReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof buildLearnReceipt.no_second_vocabulary_note, "string");
  assert.deepEqual(Object.keys(buildLearnReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-critical-path",
    "academic-curriculum",
    "academic-domain",
    "academic-gap",
    "academic-knowledge-state",
    "academic-repository-classification",
  ]);
  assert.deepEqual(Object.keys(buildLearnReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-consent",
    "academic-critical-path",
    "academic-domain",
    "academic-freshness",
    "academic-gap",
    "academic-knowledge-state",
    "academic-lecture-document",
    "academic-model-run",
    "academic-policy",
    "academic-repository",
    "academic-repository-analysis",
    "academic-repository-correlation",
    "academic-transcription",
    "academic-untrusted-content",
    "serde_json",
    "tempfile",
    "trybuild",
    "uuid",
  ]);
  // `P2-P2` adds one workspace path package, `academic-deletion`, and admits no
  // external crate: its product edges are `academic-consent`, `academic-domain`,
  // `academic-evidence-center`, `academic-proposal`, `academic-retention`,
  // `academic-student-voice`, `academic-vault` and `thiserror`, and its dev
  // edges are `academic-crypto`, `academic-domain`, `academic-policy`,
  // `academic-proposal`, `academic-retention`, `serde_json` and `trybuild`, all
  // already in this lock through earlier receipts.
  //
  // Two edges need a reason and the receipt carries both. `academic-retention`
  // is the **second** product edge to that crate, and the first that is not
  // optional: `rotation_engine_lane_is_not_default` used to hold that exactly
  // one crate declared it, with a comment saying `P2-P2` is the task that would
  // wire the real derivative subsystems. This is that task, and the claim the
  // rule protects — that no default graph resolves the object namespace that
  // can destroy a key slot — is now held by `deletion_lane_is_not_default`
  // instead of by the count. `academic-policy` stays a **dev** edge, because a
  // deletion flow whose product closure held the broker could name a type that
  // owns a transmitted byte.
  const {
    receipt: deletionReceipt,
    admitted: deletionAdmitted,
    pathPackages: deletionPathPackages,
  } = receiptFor("P2-P2");
  assert.equal(deletionAdmitted.size, 0, "P2-P2 must admit no external crate");
  assert.deepEqual([...deletionPathPackages], ["academic-deletion@0.1.0"]);
  assert.deepEqual(deletionReceipt.summary.npm_additions, []);
  assert.equal(deletionReceipt.summary.npm_install_scripts_added, false);
  assert.equal(deletionReceipt.summary.linked_into_binary_count, 0);
  assert.equal(deletionReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof deletionReceipt.no_second_vocabulary_note, "string");
  assert.deepEqual(Object.keys(deletionReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-consent",
    "academic-domain",
    "academic-evidence-center",
    "academic-proposal",
    "academic-retention",
    "academic-student-voice",
    "academic-vault",
  ]);
  assert.deepEqual(Object.keys(deletionReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-crypto",
    "academic-domain",
    "academic-policy",
    "academic-proposal",
    "academic-retention",
    "serde_json",
    "trybuild",
  ]);
  // `P2-M4` adds one workspace path package, `academic-non-delegable`, and
  // admits no external crate: its product edges are `academic-domain`,
  // `academic-proposal` and `thiserror`, and its eleven dev edges are the six
  // crates that own the six non-delegable actions plus the two argument types
  // and `trybuild`, all already in this lock through earlier receipts. The
  // `academic-deletion` edge is the one that needs a reason and the receipt
  // carries it: it is a **dev** edge, because `deletion_lane_is_not_default`
  // holds that no crate in the shipping graph links that flow, and the test
  // that says this task's compiled constant agrees with `P2-P2`'s type has to
  // drive that type.
  const {
    receipt: nonDelegableReceipt,
    admitted: nonDelegableAdmitted,
    pathPackages: nonDelegablePathPackages,
  } = receiptFor("P2-M4");
  assert.equal(nonDelegableAdmitted.size, 0, "P2-M4 must admit no external crate");
  assert.deepEqual([...nonDelegablePathPackages], ["academic-non-delegable@0.1.0"]);
  assert.deepEqual(nonDelegableReceipt.summary.npm_additions, []);
  assert.equal(nonDelegableReceipt.summary.npm_install_scripts_added, false);
  assert.equal(nonDelegableReceipt.summary.linked_into_binary_count, 0);
  assert.equal(nonDelegableReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof nonDelegableReceipt.no_second_actor_check_note, "string");
  assert.deepEqual(Object.keys(nonDelegableReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-proposal",
  ]);
  assert.deepEqual(Object.keys(nonDelegableReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-audit",
    "academic-consent",
    "academic-deletion",
    "academic-domain",
    "academic-knowledge-state",
    "academic-policy",
    "academic-proposal",
    "academic-record",
    "academic-retention",
    "academic-student-voice",
    "trybuild",
  ]);

  // `P2-L5` adds one workspace path package, `academic-student-voice`, and
  // admits no external crate: its product edges are `academic-capture`,
  // `academic-consent`, `academic-domain`, `academic-lecture-document`,
  // `academic-transcription` and `thiserror`, and its dev edges are
  // `academic-model-run`, `academic-retention`, `tempfile` and `trybuild`, all
  // already in this lock through earlier receipts. The `academic-retention`
  // edge is the one that needs a reason and the receipt carries it: it is a
  // **dev** edge, because `rotation_engine_lane_is_not_default` holds that
  // exactly one crate declares that product edge, and the scan that refuses an
  // `OriginalVoiceAuthority` still has to name the type it refuses.
  const {
    receipt: studentVoiceReceipt,
    admitted: studentVoiceAdmitted,
    pathPackages: studentVoicePathPackages,
  } = receiptFor("P2-L5");
  assert.equal(studentVoiceAdmitted.size, 0, "P2-L5 must admit no external crate");
  assert.deepEqual([...studentVoicePathPackages], ["academic-student-voice@0.1.0"]);
  assert.deepEqual(studentVoiceReceipt.summary.npm_additions, []);
  assert.equal(studentVoiceReceipt.summary.npm_install_scripts_added, false);
  assert.equal(studentVoiceReceipt.summary.linked_into_binary_count, 0);
  assert.equal(studentVoiceReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof studentVoiceReceipt.no_second_retention_rule_note, "string");
  assert.deepEqual(Object.keys(studentVoiceReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-consent",
    "academic-domain",
    "academic-lecture-document",
    "academic-transcription",
  ]);
  assert.deepEqual(Object.keys(studentVoiceReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-consent",
    "academic-domain",
    "academic-model-run",
    "academic-retention",
    "tempfile",
    "trybuild",
  ]);

  // `P2-L6` adds one workspace path package, `academic-next-lecture`, and
  // admits no external crate. It is section 12.7's next-lecture preparation:
  // five product edges, each a boundary it reads a fact out of rather than a
  // vocabulary it restates, and thirteen dev edges of which eleven are the
  // fixture chain it reaches through `P2-N5`'s own module by `#[path]`.
  // `academic-home` is the dev edge that needs a reason and the receipt carries
  // it: `P2-X2` offers the same morning card, so the `1-3` bound is compared
  // between two crates after being parsed out of two design-document sentences,
  // and a product edge would make one crate's bound the other's by construction
  // rather than by comparison. There is deliberately no `academic-knowledge-state`
  // product edge, which is what `an_extracted_claim_is_never_confirmed` reads out
  // of the public signatures rather than asserts.
  const {
    receipt: nextLectureReceipt,
    admitted: nextLectureAdmitted,
    pathPackages: nextLecturePathPackages,
  } = receiptFor("P2-L6");
  assert.equal(nextLectureAdmitted.size, 0, "P2-L6 must admit no external crate");
  assert.deepEqual([...nextLecturePathPackages], ["academic-next-lecture@0.1.0"]);
  assert.deepEqual(nextLectureReceipt.summary.npm_additions, []);
  assert.equal(nextLectureReceipt.summary.npm_install_scripts_added, false);
  assert.equal(nextLectureReceipt.summary.linked_into_binary_count, 0);
  assert.equal(nextLectureReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof nextLectureReceipt.no_second_ladder_note, "string");
  assert.deepEqual(Object.keys(nextLectureReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-gap",
    "academic-ingestion",
    "academic-lecture-document",
    "academic-untrusted-content",
  ]);
  assert.deepEqual(Object.keys(nextLectureReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-consent",
    "academic-domain",
    "academic-freshness",
    "academic-home",
    "academic-knowledge-state",
    "academic-model-run",
    "academic-policy",
    "academic-repository",
    "academic-repository-analysis",
    "academic-repository-classification",
    "academic-repository-correlation",
    "academic-transcription",
    "serde_json",
    "tempfile",
    "trybuild",
    "uuid",
  ]);
  assert.deepEqual(nextLectureReceipt.vendored_data, []);

  // `P2-N6` adds one workspace path package, `academic-critical-path`, and
  // admits no external crate: its product edges are `academic-curriculum`,
  // `academic-domain`, `academic-freshness`, `academic-gap`, `serde` and
  // `thiserror`, and its dev edges are `P2-N5`'s fixture chain -- reached
  // through that crate's own fixture module by `#[path]`, which reaches
  // `P2-N2`'s the same way -- plus `serde_json`, `tempfile`, `trybuild` and
  // `uuid`, all already in this lock through earlier receipts.
  // The `academic-gap` edge is the one that needs a reason and the receipt
  // carries it: this engine plans around a decided `GapCase` rather than around
  // a concept and a state, so section 15.1's restraint carries forward as a
  // graph fact and a hyperedge member is that crate's admitted
  // `PrerequisiteEdge` rather than a second allowlist here.
  // `academic-knowledge-state` is a **dev** edge and not a product one, which is
  // what makes "taking a course changes no mastery" a compile-time fact. The
  // edge it does **not** carry is the point: no `academic-store`, so no plan
  // reaches the canonical writer and no migration is added, and no
  // `academic-model-run` on the product edge, so nothing in the product path
  // can call a model.
  const {
    receipt: criticalPathReceipt,
    admitted: criticalPathAdmitted,
    pathPackages: criticalPathPathPackages,
  } = receiptFor("P2-N6");
  assert.equal(criticalPathAdmitted.size, 0, "P2-N6 must admit no external crate");
  assert.deepEqual([...criticalPathPathPackages], ["academic-critical-path@0.1.0"]);
  assert.deepEqual(criticalPathReceipt.summary.npm_additions, []);
  assert.equal(criticalPathReceipt.summary.npm_install_scripts_added, false);
  assert.equal(criticalPathReceipt.summary.linked_into_binary_count, 0);
  assert.equal(criticalPathReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof criticalPathReceipt.no_second_ladder_note, "string");
  assert.deepEqual(
    Object.keys(criticalPathReceipt.direct_workspace_dependencies).toSorted(),
    ["academic-curriculum", "academic-domain", "academic-freshness", "academic-gap"],
  );
  assert.deepEqual(Object.keys(criticalPathReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-consent",
    "academic-knowledge-state",
    "academic-lecture-document",
    "academic-model-run",
    "academic-policy",
    "academic-repository",
    "academic-repository-analysis",
    "academic-repository-classification",
    "academic-repository-correlation",
    "academic-transcription",
    "academic-untrusted-content",
    "serde_json",
    "tempfile",
    "trybuild",
    "uuid",
  ]);

  // `P2-U3` adds `academic-audit` and no external crate. The graduation audit
  // is a boundary above `P2-U2`'s rule set and `P2-U4`'s attempt ledger: it
  // links neither a writer nor a model, which is what keeps a graduation
  // verdict off any interpreted-text path, and its one projection edge is a dev
  // edge that exists so a compile-fail case can name what cannot enter.
  const {
    receipt: auditReceipt,
    admitted: auditAdmitted,
    pathPackages: auditPathPackages,
  } = receiptFor("P2-U3");
  assert.equal(auditAdmitted.size, 0, "P2-U3 must admit no external crate");
  assert.deepEqual([...auditPathPackages], ["academic-audit@0.1.0"]);
  assert.deepEqual(auditReceipt.summary.npm_additions, []);
  assert.equal(auditReceipt.summary.npm_install_scripts_added, false);
  assert.equal(auditReceipt.summary.linked_into_binary_count, 0);
  assert.equal(auditReceipt.summary.build_time_only_count, 0);
  assert.deepEqual(Object.keys(auditReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-ingestion",
    "academic-record",
    "academic-requirement",
  ]);
  assert.deepEqual(Object.keys(auditReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-record",
    "academic-requirement",
    "academic-scenario",
  ]);
  assert.deepEqual(auditReceipt.vendored_data, []);


  // `P2-N3` adds one workspace path package, `academic-freshness`, and admits no
  // external crate: its product edges are `academic-domain`,
  // `academic-knowledge-state`, `serde` and `thiserror`, and its dev edges are
  // `P2-N2`'s two fixture chains -- reached through that crate's own fixture
  // module by `#[path]` -- plus `tempfile`, `trybuild` and `uuid`, all already
  // in this lock through earlier receipts. The `academic-knowledge-state` edge
  // is the one that needs a reason and the receipt carries it: section 13.3's
  // first input is the last strong evidence, and the evidence it means is the
  // evidence that already passed section 13.4's four checks. The edge it does
  // **not** carry is the point -- that crate hands out `LADDER`, `rung` and
  // `AutomaticLevel`, and `academic-freshness` imports none of them.
  const {
    receipt: freshnessReceipt,
    admitted: freshnessAdmitted,
    pathPackages: freshnessPathPackages,
  } = receiptFor("P2-N3");
  assert.equal(freshnessAdmitted.size, 0, "P2-N3 must admit no external crate");
  assert.deepEqual([...freshnessPathPackages], ["academic-freshness@0.1.0"]);
  assert.deepEqual(freshnessReceipt.summary.npm_additions, []);
  assert.equal(freshnessReceipt.summary.npm_install_scripts_added, false);
  assert.equal(freshnessReceipt.summary.linked_into_binary_count, 0);
  assert.equal(freshnessReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof freshnessReceipt.no_second_ladder_note, "string");
  assert.deepEqual(Object.keys(freshnessReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-knowledge-state",
  ]);
  assert.deepEqual(Object.keys(freshnessReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-consent",
    "academic-lecture-document",
    "academic-model-run",
    "academic-policy",
    "academic-repository",
    "academic-repository-analysis",
    "academic-repository-classification",
    "academic-repository-correlation",
    "academic-transcription",
    "academic-untrusted-content",
    "tempfile",
    "trybuild",
    "uuid",
  ]);
  // `P2-N5` adds one workspace path package, `academic-gap`, and admits no
  // external crate: its product edges are `academic-domain`,
  // `academic-freshness`, `academic-knowledge-state`, `serde` and `thiserror`,
  // and its dev edges are `P2-N2`'s two fixture chains -- reached through that
  // crate's own fixture module by `#[path]` -- plus `serde_json`, `tempfile`,
  // `trybuild` and `uuid`, all already in this lock through earlier receipts.
  // The `academic-freshness` edge is the one that needs a reason and the receipt
  // carries it: section 13.3 licenses a spillover on `REQUIRES`, `BUILDS_ON`,
  // `RELATED_TO` and `SPECIAL_CASE_OF`, and **two of those four are the edges
  // section 15.2 step 2 descends**, so a band a neighbour on the blocking path
  // raised is the surface concept's evidence deciding its own prerequisite's
  // deficit. The edge it does **not** carry is the point: no `academic-store`,
  // so no gap reaches the canonical writer and no migration is added.
  const {
    receipt: gapReceipt,
    admitted: gapAdmitted,
    pathPackages: gapPathPackages,
  } = receiptFor("P2-N5");
  assert.equal(gapAdmitted.size, 0, "P2-N5 must admit no external crate");
  assert.deepEqual([...gapPathPackages], ["academic-gap@0.1.0"]);
  assert.deepEqual(gapReceipt.summary.npm_additions, []);
  assert.equal(gapReceipt.summary.npm_install_scripts_added, false);
  assert.equal(gapReceipt.summary.linked_into_binary_count, 0);
  assert.equal(gapReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof gapReceipt.no_second_ladder_note, "string");
  assert.deepEqual(Object.keys(gapReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-freshness",
    "academic-knowledge-state",
  ]);
  assert.deepEqual(Object.keys(gapReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-consent",
    "academic-lecture-document",
    "academic-model-run",
    "academic-policy",
    "academic-repository",
    "academic-repository-analysis",
    "academic-repository-classification",
    "academic-repository-correlation",
    "academic-transcription",
    "academic-untrusted-content",
    "serde_json",
    "tempfile",
    "trybuild",
    "uuid",
  ]);

  // `P2-N7` adds one workspace path package, `academic-blind-spot`, and admits
  // no external crate: its product edges are `academic-domain`,
  // `academic-knowledge-state`, `serde` and `thiserror`, and its dev edges are
  // `P2-N2`'s lecture fixture chain -- reached through that crate's own
  // `tests/common/lecture.rs` by `#[path]` -- plus `academic-knowledge-state`
  // again for `trybuild`, `serde_json`, `tempfile`, `trybuild` and `uuid`, all
  // already in this lock through earlier receipts. It takes the `lecture` half
  // of that fixture module only, so the repository chain `P2-N5` and `P2-N6`
  // reach is absent here. The edges it does **not** carry are the point: no
  // `academic-gap`, which is what makes `모든 분야를 균등하게 채우라는 목표를
  // 만들지 않는다` a graph fact rather than a rule in a function; no
  // `academic-freshness`, so a band is carried and never computed; and no
  // `academic-store`, so no finding reaches the canonical writer and no
  // migration is added.
  const {
    receipt: blindSpotReceipt,
    admitted: blindSpotAdmitted,
    pathPackages: blindSpotPathPackages,
  } = receiptFor("P2-N7");
  assert.equal(blindSpotAdmitted.size, 0, "P2-N7 must admit no external crate");
  assert.deepEqual([...blindSpotPathPackages], ["academic-blind-spot@0.1.0"]);
  assert.deepEqual(blindSpotReceipt.summary.npm_additions, []);
  assert.equal(blindSpotReceipt.summary.npm_install_scripts_added, false);
  assert.equal(blindSpotReceipt.summary.linked_into_binary_count, 0);
  assert.equal(blindSpotReceipt.summary.build_time_only_count, 0);
  assert.equal(typeof blindSpotReceipt.no_second_ladder_note, "string");
  assert.deepEqual(Object.keys(blindSpotReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-domain",
    "academic-knowledge-state",
  ]);
  assert.deepEqual(Object.keys(blindSpotReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-capture",
    "academic-consent",
    "academic-knowledge-state",
    "academic-lecture-document",
    "academic-model-run",
    "academic-transcription",
    "serde_json",
    "tempfile",
    "trybuild",
    "uuid",
  ]);

  // `P2-P3` adds `academic-integrations` and no external crate. It is the
  // section 33 boundary: six product edges, each one a boundary it reuses
  // rather than rebuilds, and six dev edges of which `academic-ledger` is the
  // real core `core_graph_opens_with_every_connector_down` opens while every
  // connector is down. There is deliberately no `academic-competency` edge of
  // any kind, which is what `assistant_use_is_not_competency` reads out of the
  // manifests rather than asserts.
  const {
    receipt: integrationsReceipt,
    admitted: integrationsAdmitted,
    pathPackages: integrationsPathPackages,
  } = receiptFor("P2-P3");
  assert.equal(integrationsAdmitted.size, 0, "P2-P3 must admit no external crate");
  assert.deepEqual([...integrationsPathPackages], ["academic-integrations@0.1.0"]);
  assert.deepEqual(integrationsReceipt.summary.npm_additions, []);
  assert.equal(integrationsReceipt.summary.npm_install_scripts_added, false);
  assert.equal(integrationsReceipt.summary.linked_into_binary_count, 0);
  assert.equal(integrationsReceipt.summary.build_time_only_count, 0);
  assert.deepEqual(
    Object.keys(integrationsReceipt.direct_workspace_dependencies).toSorted(),
    [
      "academic-domain",
      "academic-egress-boundary",
      "academic-model-run",
      "academic-policy",
      "academic-repository",
      "academic-untrusted-content",
      "sha2",
      "thiserror",
    ],
  );
  assert.deepEqual(Object.keys(integrationsReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-contracts",
    "academic-crypto",
    "academic-ledger",
    "ed25519-dalek",
    "trybuild",
    "zeroize",
  ]);
  assert.deepEqual(integrationsReceipt.vendored_data, []);

  // `P2-U5` adds `academic-offering` and no external crate. The offering
  // forecast is a boundary above `P2-U1`'s aggregates and `P2-U6`'s source
  // levels: it links no writer, and its one model edge is `P2-M1`'s calibration
  // registry, which is what turns a raw score into a number a reader may see.
  const {
    receipt: offeringReceipt,
    admitted: offeringAdmitted,
    pathPackages: offeringPathPackages,
  } = receiptFor("P2-U5");
  assert.equal(offeringAdmitted.size, 0, "P2-U5 must admit no external crate");
  assert.deepEqual([...offeringPathPackages], ["academic-offering@0.1.0"]);
  assert.deepEqual(offeringReceipt.summary.npm_additions, []);
  assert.equal(offeringReceipt.summary.npm_install_scripts_added, false);
  assert.equal(offeringReceipt.summary.linked_into_binary_count, 0);
  assert.equal(offeringReceipt.summary.build_time_only_count, 0);
  assert.deepEqual(Object.keys(offeringReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-curriculum",
    "academic-domain",
    "academic-ingestion",
    "academic-model-run",
    "academic-record",
  ]);
  assert.deepEqual(Object.keys(offeringReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-curriculum",
    "academic-domain",
    "academic-model-run",
    "academic-record",
    "trybuild",
  ]);
  assert.deepEqual(offeringReceipt.vendored_data, []);

  // `P2-P1` adds `academic-export` and no external crate. It is a separate
  // package from `academic-portability` for the reason `INV-C-015` is about:
  // that crate's three lanes all link `academic-store`, and a bundle written
  // from it would have put the database engine inside the closure of the
  // artefact a user keeps after this product is gone. What this crate links is
  // `P2-U3`'s engine, `P2-U2`'s published rule set and the domain, and nothing
  // that could open a store, unwrap a key or reach a host.
  const {
    receipt: exportReceipt,
    admitted: exportAdmitted,
    pathPackages: exportPathPackages,
  } = receiptFor("P2-P1");
  assert.equal(exportAdmitted.size, 0, "P2-P1 must admit no external crate");
  assert.deepEqual([...exportPathPackages], ["academic-export@0.1.0"]);
  assert.deepEqual(exportReceipt.summary.npm_additions, []);
  assert.equal(exportReceipt.summary.npm_install_scripts_added, false);
  assert.equal(exportReceipt.summary.linked_into_binary_count, 0);
  assert.equal(exportReceipt.summary.build_time_only_count, 0);
  assert.deepEqual(Object.keys(exportReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-audit",
    "academic-domain",
    "academic-requirement",
    "serde",
    "serde_json",
    "sha2",
  ]);
  assert.deepEqual(Object.keys(exportReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-contracts",
    "academic-ingestion",
    "academic-portability",
    "academic-record",
    "academic-store",
    "academic-vault",
    "ed25519-dalek",
  ]);

  // `P2-U8` adds `academic-review` and no external crate. It is a separate
  // package from `academic-ingestion` for the reason section 29.5 is about:
  // that crate is `P2-U6`'s official-source pipeline, and its surface holds the
  // conditional-fetch trait, the declared target and the credential binding a
  // request is composed from. Somebody else's writing does not belong in the
  // same package as those. What this crate links is the curriculum's instructor
  // and term names, the domain identifiers, `P2-U6`'s four fallbacks, `P2-M2`'s
  // `AI_INFERRED` constant and `P2-G5`'s trust label -- and nothing that could
  // open a socket, persist a row, or serialise a review into a bundle.
  const {
    receipt: reviewReceipt,
    admitted: reviewAdmitted,
    pathPackages: reviewPathPackages,
  } = receiptFor("P2-U8");
  assert.equal(reviewAdmitted.size, 0, "P2-U8 must admit no external crate");
  assert.deepEqual([...reviewPathPackages], ["academic-review@0.1.0"]);
  assert.deepEqual(reviewReceipt.summary.npm_additions, []);
  assert.equal(reviewReceipt.summary.npm_install_scripts_added, false);
  assert.equal(reviewReceipt.summary.linked_into_binary_count, 0);
  assert.equal(reviewReceipt.summary.build_time_only_count, 0);
  assert.deepEqual(Object.keys(reviewReceipt.direct_workspace_dependencies).toSorted(), [
    "academic-curriculum",
    "academic-domain",
    "academic-ingestion",
    "academic-proposal",
    "academic-untrusted-content",
  ]);
  assert.deepEqual(Object.keys(reviewReceipt.dev_workspace_dependencies).toSorted(), [
    "academic-untrusted-content",
    "trybuild",
  ]);
  assert.deepEqual(reviewReceipt.vendored_data, []);

  // Everything in the lock that no phase 2 receipt claims. The conjunction this
  // replaces missed `processAdmitted`, which is inert only because `P2-G7` admits
  // no external crate; a whole-set difference cannot miss one.
  const claimedByPhase2 = new Set(
    [...phase2Receipts.values()].flatMap(({ admitted, pathPackages }) => [
      ...admitted,
      ...pathPackages,
    ]),
  );
  const incomingTuples = lockTuples.filter(
    ([name, version]) =>
      name !== "academic-store-platform" && !claimedByPhase2.has(`${name}@${version}`),
  );
  assert.equal(incomingTuples.length, receipt.lock_delta.incoming_package_tuple_count);
  assert.equal(
    createHash("sha256").update(JSON.stringify(incomingTuples)).digest("hex"),
    receipt.lock_delta.incoming_package_tuple_sha256,
    "an incoming Cargo.lock package tuple changed",
  );
  assert.equal(
    lockTuples.length,
    receipt.lock_delta.incoming_package_tuple_count +
      1 +
      [...phase2Receipts.values()].reduce((total, { tuples }) => total + tuples.length, 0),
  );
  // Every receipt the arithmetic above counts also has a block below it saying
  // what that task's edges are. Without this the arithmetic is complete on its
  // own and a dropped block is silent -- the sum used to break because each
  // block held its own tuple filter, and deriving the sum is what takes that
  // coupling away. `T186` measured the block-shaped merge break as caught; it
  // stays caught, and now by something that names the receipt.
  assert.deepEqual(
    [...phase2Receipts.keys()].filter((task) => !bound.has(task)).toSorted(),
    [],
    `an admission receipt in ${RECEIPT_DIRECTORY} is read but named by no block here`,
  );

  assert.deepEqual(receipt.toolchain, {
    rust: "1.98.0",
    node: "24.19.0",
    pnpm: "11.22.0",
  });
  const preservedPhase0Versions = {
    anyhow: "1.0.104",
    ciborium: "0.2.2",
    clap: "4.6.6",
    "ed25519-dalek": "2.2.0",
    hex: "0.4.3",
    hmac: "0.12.1",
    proptest: "1.11.0",
    prost: "0.14.1",
    serde: "1.0.229",
    serde_json: "1.0.151",
    sha2: "0.10.9",
    thiserror: "2.0.20",
    uuid: "1.25.0",
  };
  assert.deepEqual(receipt.preserved_phase0_direct_versions, preservedPhase0Versions);
  assert.deepEqual(receipt.npm_additions, []);
  assert.equal(receipt.npm_install_scripts_added, false);

  const expectedAdmissions = [
    "assert_cmd",
    "getrandom",
    "predicates",
    "prost-build",
    "protoc-bin-vendored",
    "rusqlite",
    "rustix",
    "tempfile",
    "tokio",
    "windows-sys",
  ];
  assert.deepEqual(
    receipt.admissions.map((entry) => entry.name),
    expectedAdmissions,
    "receipt admission inventory/order drifted",
  );

  const admittedVersions = Object.fromEntries(
    receipt.admissions.map((entry) => [entry.name, entry.version]),
  );
  const expectedDirectVersions = {
    ...preservedPhase0Versions,
    ...admittedVersions,
    ...keyReceipt.direct_workspace_dependencies,
    ...scenarioReceipt.direct_workspace_dependencies,
    ...sandboxReceipt.direct_workspace_dependencies,
  };
  const directRegistryDependencies = workspacePackages.flatMap((pkg) =>
    pkg.dependencies.filter((dependency) => dependency.source !== null),
  );
  assert.deepEqual(
    [...new Set(directRegistryDependencies.map((dependency) => dependency.name))].toSorted(),
    Object.keys(expectedDirectVersions).toSorted(),
    "workspace has an unreceipted direct registry dependency",
  );
  for (const dependency of directRegistryDependencies) {
    assert.equal(
      dependency.req,
      `=${expectedDirectVersions[dependency.name]}`,
      `${dependency.name} is not pinned to its accepted exact version`,
    );
  }

  const lockChecksum = (name, version) => {
    const block = cargoLock
      .split(/\n(?=\[\[package\]\]\n)/u)
      .find(
        (candidate) =>
          candidate.includes(`name = "${name}"\n`) &&
          candidate.includes(`version = "${version}"\n`),
      );
    assert.ok(block, `Cargo.lock omitted ${name} ${version}`);
    return block.match(/^checksum = "(?<checksum>[0-9a-f]{64})"$/mu)?.groups?.checksum;
  };

  for (const admission of receipt.admissions) {
    const pkg = metadata.packages.find(
      (candidate) =>
        candidate.name === admission.name && candidate.version === admission.version,
    );
    assert.ok(pkg, `metadata omitted admitted dependency ${admission.name}`);
    assert.equal(pkg.version, admission.version, `${admission.name} version`);
    assert.equal(
      lockChecksum(admission.name, admission.version),
      admission.checksum,
      `${admission.name} checksum`,
    );
    assert.equal(pkg.license, admission.license, `${admission.name} license`);
    assert.equal(pkg.rust_version, admission.rust_version, `${admission.name} rust-version`);
    assert.equal(pkg.source, admission.source, `${admission.name} source`);
    assert.equal(typeof admission.owner, "string");
    assert.ok(admission.owner.length > 0, `${admission.name} owner is empty`);
    assert.equal(admission.install_scripts, false, `${admission.name} install script admitted`);
    for (const feature of admission.admitted_features) {
      assert.ok(Object.hasOwn(pkg.features, feature), `${admission.name} omitted feature ${feature}`);
    }

    const actualUses = workspacePackages
      .flatMap((owner) =>
        owner.dependencies
          .filter((dependency) => dependency.name === admission.name)
          .map((dependency) => normalizeDependencyUse(dependency, owner.name)),
      )
      .toSorted((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
    const j1ProjectionUse =
      admission.name === "rusqlite"
        ? [
            {
              package: "academic-projections",
              kind: "normal",
              target: null,
              default_features: false,
              features: ["backup", "hooks", "limits"],
            },
          ]
        : [];
    const t047FormatTestUse =
      admission.name === "rusqlite"
        ? [
            {
              package: "academic-core",
              kind: "dev",
              target: null,
              default_features: false,
              features: ["backup", "hooks", "limits"],
            },
          ]
        : [];
    // `P2-K4` generates the backup root and every nonce it wraps with, from the
    // same admitted randomness source and the same feature set. It adds no
    // second entropy path.
    const k4RecoveryUse =
      admission.name === "getrandom"
        ? [
            {
              package: "academic-recovery",
              kind: "normal",
              target: null,
              default_features: false,
              features: ["std"],
            },
          ]
        : [];
    const k6AdmissionTestUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-admission",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : [];
    const g1PolicyUse =
      admission.name === "rusqlite"
        ? [
            {
              package: "academic-policy",
              kind: "normal",
              target: null,
              default_features: false,
              features: ["backup", "bundled", "hooks", "limits"],
            },
          ]
        : ["sha2", "thiserror"].includes(admission.name)
          ? [
              {
                package: "academic-policy",
                kind: "normal",
                target: null,
                default_features: true,
                features: [],
              },
            ]
          : admission.name === "proptest"
            ? [
                {
                  package: "academic-policy",
                  kind: "dev",
                  target: null,
                  default_features: true,
                  features: [],
                },
              ]
            : [];
    // `P2-U7` reuses two already-admitted crates at their default features:
    // `sha2` for the canonical and checksum digests, `thiserror` for the error
    // type. It adds no third.
    const u7TranscriptUse = ["sha2", "thiserror"].includes(admission.name)
      ? [
          {
            package: "academic-transcript",
            kind: "normal",
            target: null,
            default_features: true,
            features: [],
          },
        ]
      : [];
    // `P2-G4` reuses three already-admitted crates and admits none: `tempfile`
    // for the job roots its acceptance suite builds, and `windows-sys` for the
    // AppContainer and job-object calls, with two feature groups no other
    // package selects. `sha2` and `thiserror` are not on this list because they
    // are not `admissions` entries. The edge is optional and behind
    // `native-sandbox`; `cargo metadata` reports declared dependencies, so it
    // appears here whether or not the feature is on.
    const g4SandboxUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-worker",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : admission.name === "windows-sys"
          ? [
              {
                package: "academic-worker",
                kind: "normal",
                target: "cfg(windows)",
                default_features: false,
                features: [
                  "Win32_Foundation",
                  "Win32_Security",
                  "Win32_Security_Authorization",
                  "Win32_Security_Isolation",
                  "Win32_Storage_FileSystem",
                  "Win32_System_JobObjects",
                  "Win32_System_Threading",
                ],
              },
            ]
          : [];
    // `P2-L1` reuses the same two and admits none: `tempfile` for the report
    // directories its native suite launches a probe into, and `windows-sys`
    // for the AppContainer and configuration-manager calls. Its feature group
    // is not the sandbox's: it needs no job object and it does need
    // `Win32_Devices_DeviceAndDriverInstallation`, which is how a device
    // interface path is enumerated rather than compiled in. The edge is
    // optional and behind `native-capture`; `cargo metadata` reports declared
    // dependencies, so it appears here whether or not the feature is on.
    const l1CaptureUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-capture-gate",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : admission.name === "windows-sys"
          ? [
              {
                package: "academic-capture-gate",
                kind: "normal",
                target: "cfg(windows)",
                default_features: false,
                features: [
                  "Win32_Devices_DeviceAndDriverInstallation",
                  "Win32_Foundation",
                  "Win32_Security",
                  "Win32_Security_Authorization",
                  "Win32_Security_Isolation",
                  "Win32_Storage_FileSystem",
                  "Win32_System_Threading",
                ],
              },
            ]
          : [];
    // `P2-R1` reuses `tempfile` for the synthetic repository trees its
    // acceptance suite builds in-process, and admits none. Its other edges --
    // `sha2`, `thiserror`, `zeroize` -- are `normal` edges on crates that are
    // not `admissions` entries of this receipt, so they are not on this list.
    const r1RepositoryUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-repository",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : [];
    // `P2-L2`. The capture subsystem's one external edge is `tempfile`, and it
    // is a dev edge: the acceptance suite writes its journals into a temporary
    // directory and the product crate takes the journal path as an argument.
    const l2CaptureUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-capture",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : [];
    // `P2-L3`. The transcription pipeline's one external edge on this receipt
    // is `tempfile`, and it is a dev edge: the acceptance suite drives a real
    // `academic_capture::begin` into a temporary directory so the recorder its
    // input manifests bind to, and the journal headers compared against it, are
    // written by the real capture surface rather than fabricated.
    const l3TranscriptionUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-transcription",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : [];
    // `P2-L4`. The coverage suite drives a real capture, so it writes a real
    // journal into a temporary directory. `trybuild` is on the Phase 1 receipt
    // rather than this one, so only `tempfile` gains an owner here.
    const l4LectureDocumentUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-lecture-document",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : [];
    // `P2-N2`. Its acceptance suite drives the same real capture `P2-L4`'s does,
    // so it writes a real journal into a temporary directory. `serde_json`,
    // `trybuild` and `uuid` are on the Phase 1 receipt rather than this one, so
    // only `tempfile` gains an owner here.
    const n2KnowledgeStateUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-knowledge-state",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : [];
    // `P2-N3`. Its acceptance suite reaches `P2-N2`'s fixture module by
    // `#[path]`, so it drives the same real capture and writes a real journal
    // into a temporary directory. `trybuild` and `uuid` are on the Phase 1
    // receipt rather than this one, so only `tempfile` gains an owner here.
    const n3FreshnessUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-freshness",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : [];
    // `P2-N7`. Its acceptance suite reaches `P2-N2`'s lecture fixture file by
    // `#[path]`, so it drives the same real capture and writes a real journal
    // into a temporary directory, and `serde_json` is the wire round-trip of
    // section 23's schema plus the whole-set key comparison that makes a goal
    // added to a finding an extra key. `trybuild` and `uuid` are on the Phase 1
    // receipt rather than this one.
    const n7BlindSpotUse = ["tempfile", "serde_json"].includes(admission.name)
      ? [
          {
            package: "academic-blind-spot",
            kind: "dev",
            target: null,
            default_features: true,
            features: [],
          },
        ]
      : [];
    // `P2-L5`. Its acceptance suite drives the same real capture `P2-L4`'s
    // does, so it writes a real journal into a temporary directory. `trybuild`
    // is on the Phase 1 receipt rather than this one, so only `tempfile` gains
    // an owner here.
    const l5StudentVoiceUse =
      admission.name === "tempfile"
        ? [
            {
              package: "academic-student-voice",
              kind: "dev",
              target: null,
              default_features: true,
              features: [],
            },
          ]
        : [];
    // `P2-N5`. Its acceptance suite reaches `P2-N2`'s fixture module by
    // `#[path]` for the same reason `P2-N3`'s does, so it drives the same real
    // capture and writes a real journal into a temporary directory; and it
    // drives section 15.1's `GapCase` through a real encoder, which is the only
    // way a round trip can observe that deserialization re-runs the
    // constructor's checks. `trybuild` and `uuid` are on the Phase 1 receipt
    // rather than this one, so only `tempfile` and `serde_json` gain an owner
    // here.
    // `P2-L6`. Its acceptance suite reaches `P2-N5`'s fixture module by
    // `#[path]`, which reaches `P2-N2`'s the same way, so it drives the same
    // real capture and writes a real journal into a temporary directory.
    // `serde_json` is a declared dev edge of the package and gains an owner
    // here for that reason; `trybuild` and `uuid` are on the Phase 1 receipt
    // rather than this one.
    const l6NextLectureUse = ["tempfile", "serde_json"].includes(admission.name)
      ? [
          {
            package: "academic-next-lecture",
            kind: "dev",
            target: null,
            default_features: true,
            features: [],
          },
        ]
      : [];
    const n5GapUse = ["tempfile", "serde_json"].includes(admission.name)
      ? [
          {
            package: "academic-gap",
            kind: "dev",
            target: null,
            default_features: true,
            features: [],
          },
        ]
      : [];
    // `P2-N6` gains the same two owners and for the same two reasons, because
    // its fixture chain is `P2-N5`'s reached by `#[path]`: `tempfile` is the
    // directory a real `P2-L2` journal is written into on the way to the
    // `GapCase` this engine plans around, and `serde_json` drives an
    // `Opportunity` through a real encoder so `course_is_an_acquisition_option`
    // can compare its field set against the three an occasion has rather than
    // against a list this suite wrote. `trybuild` and `uuid` are on the Phase 1
    // receipt rather than this one.
    const n6CriticalPathUse = ["tempfile", "serde_json"].includes(admission.name)
      ? [
          {
            package: "academic-critical-path",
            kind: "dev",
            target: null,
            default_features: true,
            features: [],
          },
        ]
      : [];
    // `P2-R6` gains the same two owners and for the same reasons, because its
    // fixture chain is `P2-N5`'s reached by `#[path]` the way `P2-N6`'s is:
    // `tempfile` is the directory a real `P2-L2` journal is written into on the
    // way to the `ConceptState` overlay the readiness comparison reads, and
    // `serde_json` compares the goal's, the constraint's, the decision's and the
    // motivation display's key sets as whole sets rather than field by field.
    // `trybuild` and `uuid` are on the Phase 1 receipt rather than this one.
    const r6BuildLearnUse = ["tempfile", "serde_json"].includes(admission.name)
      ? [
          {
            package: "academic-build-learn",
            kind: "dev",
            target: null,
            default_features: true,
            features: [],
          },
        ]
      : [];
    const expectedUses = [
      ...admission.uses,
      ...n5GapUse,
      ...l6NextLectureUse,
      ...n6CriticalPathUse,
      ...r6BuildLearnUse,
      ...n3FreshnessUse,
      ...n7BlindSpotUse,
      ...l5StudentVoiceUse,
      ...l3TranscriptionUse,
      ...l4LectureDocumentUse,
      ...n2KnowledgeStateUse,
      ...r1RepositoryUse,
      ...l2CaptureUse,
      ...g4SandboxUse,
      ...l1CaptureUse,
      ...j1ProjectionUse,
      ...t047FormatTestUse,
      ...k4RecoveryUse,
      ...k6AdmissionTestUse,
      ...g1PolicyUse,
      ...u7TranscriptUse,
    ]
      .map((use) => ({ ...use, features: use.features.toSorted() }))
      .toSorted((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
    assert.deepEqual(actualUses, expectedUses, `${admission.name} owner/feature receipt`);
  }

  // Unlike the name/version subtraction above, each `P2-C7` admission is
  // checked against what Cargo actually resolved. A receipt that named the
  // right package under the wrong licence, checksum, or registry would
  // otherwise still subtract cleanly.
  for (const admission of scenarioReceipt.admissions) {
    const pkg = metadata.packages.find(
      (candidate) =>
        candidate.name === admission.name && candidate.version === admission.version,
    );
    assert.ok(pkg, `metadata omitted P2-C7 dependency ${admission.name}`);
    assert.equal(pkg.license, admission.license, `${admission.name} license`);
    assert.equal(pkg.rust_version, admission.rust_version, `${admission.name} rust-version`);
    assert.equal(pkg.source, admission.source, `${admission.name} source`);
    assert.equal(
      lockChecksum(admission.name, admission.version),
      admission.checksum,
      `${admission.name} checksum`,
    );
    assert.equal(
      admission.role,
      "build-time only, never linked into a product binary",
      `${admission.name} was admitted as a shipping dependency`,
    );
    assert.equal(
      defaultProductPackageNames().has(admission.name),
      false,
      `${admission.name} entered the default product graph`,
    );
  }

  const expectedNativeNames = [
    "cc",
    "find-msvc-tools",
    "libsqlite3-sys",
    "openssl-src",
    "openssl-sys",
    "pkg-config",
    "shlex",
    "vcpkg",
  ];
  assert.deepEqual(
    receipt.native_transitives.map((entry) => entry.name),
    expectedNativeNames,
    "native transitive inventory drifted",
  );
  for (const native of receipt.native_transitives) {
    const pkg = spikeMetadata.packages.find(
      (candidate) => candidate.name === native.name && candidate.version === native.version,
    );
    assert.ok(pkg, `SQLCipher metadata omitted native dependency ${native.name}`);
    assert.equal(lockChecksum(native.name, native.version), native.checksum);
    assert.equal(pkg.license, native.license, `${native.name} license`);
    assert.equal(pkg.rust_version, native.rust_version, `${native.name} rust-version`);
    assert.equal(pkg.source, native.source, `${native.name} source`);

    const spikeNode = spikeResolveNodesById.get(pkg.id);
    assert.deepEqual(
      spikeNode.features.toSorted(),
      native.sqlcipher_spike_features.toSorted(),
      `${native.name} SQLCipher-spike features`,
    );
    const defaultPkg = metadata.packages.find(
      (candidate) => candidate.name === native.name && candidate.version === native.version,
    );
    if (native.default_features === null) {
      assert.equal(defaultPkg, undefined, `${native.name} entered the default product graph`);
    } else {
      assert.ok(defaultPkg, `default metadata omitted native dependency ${native.name}`);
      assert.deepEqual(
        resolveNodesById.get(defaultPkg.id).features.toSorted(),
        native.default_features.toSorted(),
        `${native.name} default features`,
      );
    }
  }

  const protocPackages = metadata.packages
    .filter((pkg) => pkg.name.startsWith("protoc-bin-vendored-"))
    .toSorted((left, right) => left.name.localeCompare(right.name));
  assert.deepEqual(
    receipt.vendored_protoc_packages.map((entry) => entry.name),
    protocPackages.map((pkg) => pkg.name),
    "vendored protoc platform inventory drifted",
  );
  for (const platformReceipt of receipt.vendored_protoc_packages) {
    const pkg = protocPackages.find((candidate) => candidate.name === platformReceipt.name);
    assert.ok(pkg, `metadata omitted ${platformReceipt.name}`);
    assert.equal(pkg.version, platformReceipt.version);
    assert.equal(pkg.license, platformReceipt.license);
    assert.equal(pkg.source, "registry+https://github.com/rust-lang/crates.io-index");
    assert.equal(lockChecksum(pkg.name, pkg.version), platformReceipt.checksum);
  }

  const sqlitePackage = spikePackagesByName.get("libsqlite3-sys");
  const opensslSourcePackage = spikePackagesByName.get("openssl-src");
  const sqliteRoot = dirname(sqlitePackage.manifest_path);
  const opensslRoot = dirname(opensslSourcePackage.manifest_path);
  const [plainHeader, sqlcipherHeader, sqlcipherSource, sqlcipherLicense, opensslLicense] =
    await Promise.all([
      readFile(join(sqliteRoot, "sqlite3", "sqlite3.h"), "utf8"),
      readFile(join(sqliteRoot, "sqlcipher", "sqlite3.h"), "utf8"),
      readFile(join(sqliteRoot, "sqlcipher", "sqlite3.c"), "utf8"),
      readFile(join(sqliteRoot, "sqlcipher", "LICENSE")),
      readFile(join(opensslRoot, "openssl", "LICENSE.txt")),
    ]);
  assert.match(
    plainHeader,
    new RegExp(`#define SQLITE_VERSION\\s+"${receipt.bundled_sources.plaintext_sqlite.version}"`, "u"),
  );
  assert.ok(plainHeader.includes(receipt.bundled_sources.plaintext_sqlite.source_id));
  assert.match(
    sqlcipherHeader,
    new RegExp(
      `#define SQLITE_VERSION\\s+"${receipt.bundled_sources.sqlcipher_community.sqlite_version}"`,
      "u",
    ),
  );
  assert.ok(sqlcipherHeader.includes(receipt.bundled_sources.sqlcipher_community.sqlite_source_id));
  assert.ok(
    sqlcipherSource.includes(
      `#define CIPHER_VERSION_NUMBER ${receipt.bundled_sources.sqlcipher_community.version}`,
    ),
  );
  assert.ok(sqlcipherSource.includes("#define CIPHER_VERSION_BUILD community"));
  assert.equal(receipt.bundled_sources.sqlcipher_community.adr_002_accepted, false);
  assert.equal(receipt.bundled_sources.openssl.adr_002_accepted, false);
  assert.equal(
    createHash("sha256").update(sqlcipherLicense).digest("hex"),
    receipt.bundled_sources.sqlcipher_community.license_sha256,
  );
  assert.equal(
    createHash("sha256").update(opensslLicense).digest("hex"),
    receipt.bundled_sources.openssl.license_sha256,
  );

  assert.ok(
    workspacePackages.every((pkg) => workspaceIds.has(pkg.id)),
    "workspace package identity mismatch",
  );
});

test("no two admission receipts claim the same package", async () => {
  // `T174` reported that `P2-R4` and `P2-X7` cross-check each other in neither
  // direction. Enumerating the whole pair set says they are one of **161** pairs
  // no cascading clause above reaches: those clauses cover 217 of the 378
  // ordered pairs twenty-eight receipts make, and a block can only ever name the
  // ones declared before it, so the coverage is a function of declaration order
  // rather than of anything anybody decided. `T173` and `T174` each closed one
  // pair by hand; a hundred and sixty-one is not that shape.
  //
  // So the pairs are not written out here. Every receipt in `docs/security` is
  // read off disk, every package each one claims is collected, and a package
  // two of them claim is reported by name with both tasks. That is the property
  // the clauses approximate, and a receipt added after this line is in it
  // without anybody editing this test -- including one nobody wires into
  // `dependency_license_and_source_receipt_is_complete` at all.
  //
  // The tuple-sum assertion in that test is a backstop and not this: a real
  // double claim leaves the sum short by one and it reports `259 !== 260`,
  // naming neither the package nor either task.
  const directory = "docs/security";
  const files = (await readdir(directory))
    .filter((name) => name.startsWith("dependency-admission-") && name.endsWith(".json"))
    .toSorted();
  assert.ok(
    files.length >= 28,
    `expected every admission receipt to be read, found ${files.length}: ${files.join(", ")}`,
  );
  const claimants = new Map();
  const collisions = [];
  for (const file of files) {
    const receipt = JSON.parse(await readFile(join(directory, file), "utf8"));
    const task = typeof receipt.task === "string" ? receipt.task : file;
    const claimed = [
      ...(receipt.admissions ?? []).map((entry) => `${entry.name}@${entry.version}`),
      ...(receipt.added_workspace_path_packages ?? []).map(
        (entry) => `${entry.name}@${entry.version}`,
      ),
    ];
    for (const one of new Set(claimed)) {
      const first = claimants.get(one);
      if (first === undefined) {
        claimants.set(one, task);
      } else {
        collisions.push(`${one} is claimed by two admission receipts: ${first} and ${task}`);
      }
    }
  }
  assert.deepEqual(collisions.toSorted(), [], collisions.join("\n"));
});
