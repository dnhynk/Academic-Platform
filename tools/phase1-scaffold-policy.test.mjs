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
    "academic-cli": [
      "academic-admission",
      "academic-core",
      "academic-daemon",
      "academic-rpc",
    ],
    "academic-contracts": ["academic-domain"],
    "academic-connector": ["academic-policy"],
    "academic-crypto": ["academic-keystore-platform"],
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
    "academic-export-job": ["academic-policy"],
    "academic-indexer": ["academic-policy"],
    "academic-ledger": ["academic-contracts", "academic-domain"],
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
    "academic-record": ["academic-domain", "academic-transcript"],
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
    "academic-worker": ["academic-domain"],
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
    // `academic-core` owns `tests/scenario_isolation.rs`, which needs the
    // projection engine and the canonical writer in one process to prove that
    // driving the first leaves the second byte-identical. `academic-scenario`
    // links its own domain crate a second time as a dev edge because the
    // `trybuild` cases compile against the crate under test plus that crate's
    // dev-dependencies, and a case has to name the canonical types a projection
    // must never become.
    "academic-core": ["academic-scenario"],
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
    "academic-scenario": ["academic-admission", "academic-domain"],
    // `P2-G5` needs a real `PermissionBroker` to build an `EgressProxy` and a
    // real `ProcessCapability` to enumerate what a privileged action is. Both
    // are test-only: keeping `academic-policy` off the product edge above is
    // what makes "the adjudicator receives no capability" a compile error
    // rather than a source scan.
    "academic-untrusted-content": ["academic-policy"],
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
  // patterns existed. A bare numeric `syscall(41, ...)` still passes, which is
  // recorded as open in `docs/contracts/policy-source-scans.md`; the link half
  // below is what bounds who can reach `libc` at all.
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

/** Path segments that lead to a socket; renaming one hides everything under it. */
const SOCKET_MODULE_SEGMENTS = new Set(["net", "socket", "sys", "WinSock", "named_pipe"]);

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
  "academic-cli": ["libc", "mio", "rustix", "socket2", "tokio", "windows-sys"],
  "academic-connector": ["libc"],
  "academic-contracts": ["libc"],
  "academic-core": ["libc", "mio", "rustix", "socket2", "tokio", "windows-sys"],
  "academic-crypto": ["libc"],
  "academic-daemon": ["libc", "mio", "rustix", "socket2", "tokio", "windows-sys"],
  "academic-domain": ["libc"],
  "academic-egress": ["libc"],
  "academic-egress-boundary": ["libc"],
  "academic-export-job": ["libc"],
  "academic-indexer": ["libc"],
  "academic-keystore-platform": ["windows-sys"],
  "academic-ledger": ["libc"],
  "academic-policy": ["libc"],
  "academic-portability": ["libc", "rustix", "windows-sys"],
  "academic-projections": ["libc", "rustix", "windows-sys"],
  "academic-record": ["libc"],
  "academic-recovery": ["libc"],
  "academic-repository-analyzer": ["libc"],
  "academic-retention": ["libc"],
  "academic-rpc": ["libc", "mio", "rustix", "socket2", "tokio", "windows-sys"],
  "academic-scenario": ["libc"],
  "academic-store": ["libc", "rustix", "windows-sys"],
  "academic-store-platform": ["libc", "rustix", "windows-sys"],
  "academic-test-support": [],
  "academic-transcript": ["libc"],
  "academic-vault": ["libc", "rustix", "windows-sys"],
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
  "academic-untrusted-content": ["libc"],
};
async function rustSourcesIfPresent(root) {
  try {
    return await rustSources(root);
  } catch {
    return [];
  }
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

  const observed = new Map();
  const aliases = [];
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

      // An alias hides every later mention of what it renames, so the two that
      // could hide a socket may only ever be renamed to `_` -- the trait-import
      // spelling, which cannot be written as a path.
      //
      // The first is a crate root: `use tokio as t;` leaves `t::net::TcpStream`
      // spelling neither `tokio::net` nor anything else on the list. The second
      // is a socket module inside a braced group: `use tokio::{net as n};`
      // spells the module in a shape the `tokio::net` anchor does not match,
      // which is why the whole statement is read and not one path.
      //
      // A rename of anything else -- `process::Command as ProcessCommand`,
      // `Ordering as AtomicOrdering` -- is not on a socket path and is left
      // alone; forbidding those would be a rule about imports, not about
      // sockets, and this repository already has several.
      for (const match of code.matchAll(/\buse\s+([A-Za-z0-9_]+)\b[^;]*;/gu)) {
        if (!ALIASABLE_ROOTS.has(match[1])) {
          continue;
        }
        const renames = [...match[0].matchAll(/\b([A-Za-z0-9_]+)\s+as\s+([A-Za-z0-9_]+)/gu)];
        for (const [, renamed, alias] of renames) {
          const hidesASocketPath =
            renamed === match[1] || SOCKET_MODULE_SEGMENTS.has(renamed);
          if (hidesASocketPath && alias !== "_") {
            aliases.push(`${relative}: ${match[0]}`);
          }
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
  // Two engines are implemented. `P2-U4` built `GPA` and `CREDIT_ACCOUNTING`
  // in `academic-record`, so that crate's sources are now engine sources and
  // are scanned. The map is enumerated rather than counted: a third engine
  // flipping while one of these flipped back would keep any count intact.
  const IMPLEMENTED_ENGINES = new Map([
    ["GPA", "academic-record"],
    ["CREDIT_ACCOUNTING", "academic-record"],
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
  const scanned = [
    join("crates", "domain", "src", "engines.rs"),
    join("crates", "domain", "src", "engines", "generated.rs"),
    join("crates", "domain", "tests", "engine_harness.rs"),
    ...recordSources,
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
    ["model", /\bModelRun\b/u],
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
  assert.deepEqual(storeCryptoDependents, [
    "academic-portability",
    "academic-recovery",
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

  // One workspace crate declares a product edge to it, and it is optional:
  // `academic-portability`'s encrypted restore re-applies the tombstones a
  // backup carries, which is `P2-K5`'s keyless positioned write and cannot be
  // imitated on the portability side without duplicating a deletion mechanism.
  // Every other crate must not declare the edge at all, and no default graph
  // may resolve it: `P2-P2` is the task that wires the real derivative
  // subsystems, and until then no product binary links a crate that can
  // destroy a key slot.
  const retentionDependents = workspacePackages
    .filter((pkg) => productDependencyNames(pkg).includes("academic-retention"))
    .map((pkg) => pkg.name)
    .toSorted();
  assert.deepEqual(
    retentionDependents,
    ["academic-portability"],
    "a crate other than the encrypted restore links the rotation engine",
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
  const [
    receiptText,
    keyReceiptText,
    scenarioReceiptText,
    recoveryReceiptText,
    retentionReceiptText,
    admissionReceiptText,
    policyReceiptText,
    processReceiptText,
    transcriptReceiptText,
    egressReceiptText,
    recordReceiptText,
    sandboxReceiptText,
    untrustedReceiptText,
    cargoLock,
  ] = await Promise.all([
    readFile("docs/security/dependency-admission-phase1.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-k1.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-c7.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-k4.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-k5.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-k6.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-g1.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-g7.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-u7.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-g2.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-u4.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-g4.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-g5.json", "utf8"),
    readFile("Cargo.lock", "utf8"),
  ]);
  const receipt = JSON.parse(receiptText);
  const keyReceipt = JSON.parse(keyReceiptText);
  const scenarioReceipt = JSON.parse(scenarioReceiptText);
  const recoveryReceipt = JSON.parse(recoveryReceiptText);
  const retentionReceipt = JSON.parse(retentionReceiptText);
  const admissionReceipt = JSON.parse(admissionReceiptText);
  const policyReceipt = JSON.parse(policyReceiptText);
  const processReceipt = JSON.parse(processReceiptText);
  const transcriptReceipt = JSON.parse(transcriptReceiptText);
  const egressReceipt = JSON.parse(egressReceiptText);
  const recordReceipt = JSON.parse(recordReceiptText);
  const sandboxReceipt = JSON.parse(sandboxReceiptText);
  const untrustedReceipt = JSON.parse(untrustedReceiptText);
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

  // Every package `P2-K1` added is enumerated in its own receipt. Subtracting
  // exactly that set and re-checking the frozen Phase 1 digest proves two
  // things at once: no Phase 1 dependency moved, and nothing entered the lock
  // that is not covered by a reviewed admission receipt.
  const keyAdmitted = new Set(
    keyReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const keyPathPackages = new Set(
    keyReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  const keyTuples = lockTuples.filter(([name, version]) =>
    keyAdmitted.has(`${name}@${version}`) || keyPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    keyTuples.length,
    keyAdmitted.size + keyPathPackages.size,
    "a P2-K1 admitted package is missing from Cargo.lock",
  );

  // `P2-C7` is subtracted the same way and for the same reason. The two
  // receipts must not overlap: a package claimed by both would be subtracted
  // twice and the arithmetic below would hide a third, unreceipted arrival.
  const scenarioAdmitted = new Set(
    scenarioReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const scenarioPathPackages = new Set(
    scenarioReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  for (const claimed of [...scenarioAdmitted, ...scenarioPathPackages]) {
    assert.equal(
      keyAdmitted.has(claimed) || keyPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  const scenarioTuples = lockTuples.filter(
    ([name, version]) =>
      scenarioAdmitted.has(`${name}@${version}`) ||
      scenarioPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    scenarioTuples.length,
    scenarioAdmitted.size + scenarioPathPackages.size,
    "a P2-C7 admitted package is missing from Cargo.lock",
  );

  // `P2-K4` admits no external crate at all; its receipt covers exactly one
  // workspace path package. It is subtracted here for the same reason as the
  // other two: a path package with no receipt would otherwise be counted as an
  // unreviewed arrival, and one with a receipt must not be counted twice.
  const recoveryAdmitted = new Set(
    recoveryReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const recoveryPathPackages = new Set(
    recoveryReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  assert.equal(recoveryAdmitted.size, 0, "P2-K4 must admit no external crate");
  for (const claimed of [...recoveryAdmitted, ...recoveryPathPackages]) {
    assert.equal(
      keyAdmitted.has(claimed) ||
        keyPathPackages.has(claimed) ||
        scenarioAdmitted.has(claimed) ||
        scenarioPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  const recoveryTuples = lockTuples.filter(
    ([name, version]) =>
      recoveryAdmitted.has(`${name}@${version}`) ||
      recoveryPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    recoveryTuples.length,
    recoveryAdmitted.size + recoveryPathPackages.size,
    "a P2-K4 admitted package is missing from Cargo.lock",
  );

  // `P2-K5` admits no external crate either; its receipt covers exactly the one
  // workspace path package `academic-retention`, subtracted for the same reason.
  const retentionAdmitted = new Set(
    retentionReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const retentionPathPackages = new Set(
    retentionReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  assert.equal(retentionAdmitted.size, 0, "P2-K5 must admit no external crate");
  for (const claimed of [...retentionAdmitted, ...retentionPathPackages]) {
    assert.equal(
      keyAdmitted.has(claimed) ||
        keyPathPackages.has(claimed) ||
        scenarioAdmitted.has(claimed) ||
        scenarioPathPackages.has(claimed) ||
        recoveryAdmitted.has(claimed) ||
        recoveryPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  const retentionTuples = lockTuples.filter(
    ([name, version]) =>
      retentionAdmitted.has(`${name}@${version}`) ||
      retentionPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    retentionTuples.length,
    retentionAdmitted.size + retentionPathPackages.size,
    "a P2-K5 admitted package is missing from Cargo.lock",
  );

  // `P2-K6` likewise admits no external crate and adds only the receipt and
  // posture workspace boundary.
  const admissionAdmitted = new Set(
    admissionReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const admissionPathPackages = new Set(
    admissionReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  assert.equal(admissionAdmitted.size, 0, "P2-K6 must admit no external crate");
  for (const claimed of [...admissionAdmitted, ...admissionPathPackages]) {
    assert.equal(
      keyAdmitted.has(claimed) ||
        keyPathPackages.has(claimed) ||
        scenarioAdmitted.has(claimed) ||
        scenarioPathPackages.has(claimed) ||
        recoveryAdmitted.has(claimed) ||
        recoveryPathPackages.has(claimed) ||
        retentionAdmitted.has(claimed) ||
        retentionPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  const admissionTuples = lockTuples.filter(
    ([name, version]) =>
      admissionAdmitted.has(`${name}@${version}`) ||
      admissionPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    admissionTuples.length,
    admissionAdmitted.size + admissionPathPackages.size,
    "a P2-K6 admitted package is missing from Cargo.lock",
  );

  // `P2-G1` also reuses only previously admitted crates and adds the
  // socket-free `academic-policy` workspace boundary.
  const policyAdmitted = new Set(
    policyReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const policyPathPackages = new Set(
    policyReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  assert.equal(policyAdmitted.size, 0, "P2-G1 must admit no external crate");
  for (const claimed of [...policyAdmitted, ...policyPathPackages]) {
    assert.equal(
      keyAdmitted.has(claimed) ||
        keyPathPackages.has(claimed) ||
        scenarioAdmitted.has(claimed) ||
        scenarioPathPackages.has(claimed) ||
        recoveryAdmitted.has(claimed) ||
        recoveryPathPackages.has(claimed) ||
        retentionAdmitted.has(claimed) ||
        retentionPathPackages.has(claimed) ||
        admissionAdmitted.has(claimed) ||
        admissionPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  const policyTuples = lockTuples.filter(
    ([name, version]) =>
      policyAdmitted.has(`${name}@${version}`) ||
      policyPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    policyTuples.length,
    policyAdmitted.size + policyPathPackages.size,
    "a P2-G1 admitted package is missing from Cargo.lock",
  );

  const processAdmitted = new Set(
    processReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const processPathPackages = new Set(
    processReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
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
  for (const claimed of processPathPackages) {
    assert.equal(
      keyAdmitted.has(claimed) ||
        keyPathPackages.has(claimed) ||
        scenarioAdmitted.has(claimed) ||
        scenarioPathPackages.has(claimed) ||
        recoveryAdmitted.has(claimed) ||
        recoveryPathPackages.has(claimed) ||
        retentionAdmitted.has(claimed) ||
        retentionPathPackages.has(claimed) ||
        admissionAdmitted.has(claimed) ||
        admissionPathPackages.has(claimed) ||
        policyAdmitted.has(claimed) ||
        policyPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }

  // `P2-U7` adds the transcript ingestion boundary and admits no external
  // crate. A PDF or OCR library would have been the obvious way to build it and
  // would have arrived here as an unreceipted package; the corpus is written by
  // a deterministic builder inside the crate instead, which is why this receipt
  // subtracts one path package and nothing else.
  const transcriptAdmitted = new Set(
    transcriptReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const transcriptPathPackages = new Set(
    transcriptReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  assert.equal(transcriptAdmitted.size, 0, "P2-U7 must admit no external crate");
  for (const claimed of [...transcriptAdmitted, ...transcriptPathPackages]) {
    assert.equal(
      keyAdmitted.has(claimed) ||
        keyPathPackages.has(claimed) ||
        scenarioAdmitted.has(claimed) ||
        scenarioPathPackages.has(claimed) ||
        recoveryAdmitted.has(claimed) ||
        recoveryPathPackages.has(claimed) ||
        retentionAdmitted.has(claimed) ||
        retentionPathPackages.has(claimed) ||
        admissionAdmitted.has(claimed) ||
        admissionPathPackages.has(claimed) ||
        policyAdmitted.has(claimed) ||
        policyPathPackages.has(claimed) ||
        processAdmitted.has(claimed) ||
        processPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  const processTuples = lockTuples.filter(
    ([name, version]) => processPathPackages.has(`${name}@${version}`),
  );

  // `P2-G2` adds the DLP rulepack, the minimizer, the byte-accurate preview,
  // and the outbound seam as `academic-egress-boundary`, and admits no external
  // crate. It is a separate package from `P2-G7`'s `academic-egress` process
  // entry point, whose whole manifest and whole product source that task pins.
  assert.equal(egressReceipt.task, "P2-G2");
  const egressAdmitted = new Set(
    egressReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const egressPathPackages = new Set(
    egressReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  assert.equal(egressAdmitted.size, 0, "P2-G2 must admit no external crate");
  assert.deepEqual([
    ...egressPathPackages,
  ], ["academic-egress-boundary@0.1.0"]);
  assert.deepEqual(egressReceipt.summary.npm_additions, []);
  assert.equal(egressReceipt.summary.npm_install_scripts_added, false);
  for (const claimed of [...egressAdmitted, ...egressPathPackages]) {
    assert.equal(
      keyAdmitted.has(claimed) ||
        keyPathPackages.has(claimed) ||
        scenarioAdmitted.has(claimed) ||
        scenarioPathPackages.has(claimed) ||
        recoveryAdmitted.has(claimed) ||
        recoveryPathPackages.has(claimed) ||
        retentionAdmitted.has(claimed) ||
        retentionPathPackages.has(claimed) ||
        admissionAdmitted.has(claimed) ||
        admissionPathPackages.has(claimed) ||
        policyAdmitted.has(claimed) ||
        policyPathPackages.has(claimed) ||
        processPathPackages.has(claimed) ||
        transcriptAdmitted.has(claimed) ||
        transcriptPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  // `P2-G4` adds the worker sandbox as `academic-worker` and admits no
  // external crate: `libc` and `windows-sys` are already in this lock at these
  // versions through earlier receipts, so what is new is two direct edges,
  // which the receipt records with their own owner, licence, feature set,
  // advisory path and trust-boundary justification.
  assert.equal(sandboxReceipt.task, "P2-G4");
  const sandboxAdmitted = new Set(
    sandboxReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const sandboxPathPackages = new Set(
    sandboxReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
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
  for (const claimed of [...sandboxAdmitted, ...sandboxPathPackages]) {
    assert.equal(
      egressAdmitted.has(claimed) ||
        egressPathPackages.has(claimed) ||
        processPathPackages.has(claimed) ||
        transcriptAdmitted.has(claimed) ||
        transcriptPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  const sandboxTuples = lockTuples.filter(
    ([name, version]) =>
      sandboxAdmitted.has(`${name}@${version}`) ||
      sandboxPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    sandboxTuples.length,
    sandboxAdmitted.size + sandboxPathPackages.size,
    "a P2-G4 admitted package is missing from Cargo.lock",
  );

  // `P2-G5` adds the untrusted-content boundary as `academic-untrusted-content`
  // and admits no external crate: its product edges are `academic-egress-boundary`,
  // `sha2` and `thiserror`, and its dev edge is `academic-policy`, all four
  // already in this lock through earlier receipts.
  assert.equal(untrustedReceipt.task, "P2-G5");
  const untrustedAdmitted = new Set(
    untrustedReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const untrustedPathPackages = new Set(
    untrustedReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  assert.equal(untrustedAdmitted.size, 0, "P2-G5 must admit no external crate");
  assert.deepEqual([...untrustedPathPackages], ["academic-untrusted-content@0.1.0"]);
  assert.deepEqual(untrustedReceipt.summary.npm_additions, []);
  assert.equal(untrustedReceipt.summary.npm_install_scripts_added, false);
  assert.deepEqual(untrustedReceipt.direct_workspace_dependencies, {});
  for (const claimed of [...untrustedAdmitted, ...untrustedPathPackages]) {
    assert.equal(
      keyAdmitted.has(claimed) ||
        keyPathPackages.has(claimed) ||
        scenarioAdmitted.has(claimed) ||
        scenarioPathPackages.has(claimed) ||
        recoveryAdmitted.has(claimed) ||
        recoveryPathPackages.has(claimed) ||
        retentionAdmitted.has(claimed) ||
        retentionPathPackages.has(claimed) ||
        admissionAdmitted.has(claimed) ||
        admissionPathPackages.has(claimed) ||
        policyAdmitted.has(claimed) ||
        policyPathPackages.has(claimed) ||
        processPathPackages.has(claimed) ||
        transcriptAdmitted.has(claimed) ||
        transcriptPathPackages.has(claimed) ||
        egressAdmitted.has(claimed) ||
        egressPathPackages.has(claimed) ||
        sandboxAdmitted.has(claimed) ||
        sandboxPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  const untrustedTuples = lockTuples.filter(
    ([name, version]) =>
      untrustedAdmitted.has(`${name}@${version}`) ||
      untrustedPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    untrustedTuples.length,
    untrustedAdmitted.size + untrustedPathPackages.size,
    "a P2-G5 admitted package is missing from Cargo.lock",
  );

  const egressTuples = lockTuples.filter(
    ([name, version]) =>
      egressAdmitted.has(`${name}@${version}`) ||
      egressPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    egressTuples.length,
    egressAdmitted.size + egressPathPackages.size,
    "a P2-G2 admitted package is missing from Cargo.lock",
  );
  assert.equal(
    processTuples.length,
    processPathPackages.size,
    "a P2-G7 process package is missing from Cargo.lock",
  );
  const transcriptTuples = lockTuples.filter(
    ([name, version]) =>
      transcriptAdmitted.has(`${name}@${version}`) ||
      transcriptPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    transcriptTuples.length,
    transcriptAdmitted.size + transcriptPathPackages.size,
    "a P2-U7 admitted package is missing from Cargo.lock",
  );

  // `P2-U4` adds the attempt ledger and the two §28 engines and admits no
  // external crate. A decimal or big-rational library would have been the
  // obvious way to build a grade-point average and would have arrived here as
  // an unreceipted package; the arithmetic is written over the canonical
  // `Decimal` instead, which is why this receipt subtracts one path package and
  // nothing else.
  const recordAdmitted = new Set(
    recordReceipt.admissions.map((admission) => `${admission.name}@${admission.version}`),
  );
  const recordPathPackages = new Set(
    recordReceipt.added_workspace_path_packages.map((pkg) => `${pkg.name}@${pkg.version}`),
  );
  assert.equal(recordAdmitted.size, 0, "P2-U4 must admit no external crate");
  assert.deepEqual([...recordPathPackages], ["academic-record@0.1.0"]);
  for (const claimed of [...recordAdmitted, ...recordPathPackages]) {
    assert.equal(
      keyAdmitted.has(claimed) ||
        keyPathPackages.has(claimed) ||
        scenarioAdmitted.has(claimed) ||
        scenarioPathPackages.has(claimed) ||
        recoveryAdmitted.has(claimed) ||
        recoveryPathPackages.has(claimed) ||
        retentionAdmitted.has(claimed) ||
        retentionPathPackages.has(claimed) ||
        admissionAdmitted.has(claimed) ||
        admissionPathPackages.has(claimed) ||
        policyAdmitted.has(claimed) ||
        policyPathPackages.has(claimed) ||
        processAdmitted.has(claimed) ||
        processPathPackages.has(claimed) ||
        transcriptAdmitted.has(claimed) ||
        transcriptPathPackages.has(claimed) ||
        egressAdmitted.has(claimed) ||
        egressPathPackages.has(claimed),
      false,
      `${claimed} is claimed by two admission receipts`,
    );
  }
  const recordTuples = lockTuples.filter(
    ([name, version]) =>
      recordAdmitted.has(`${name}@${version}`) ||
      recordPathPackages.has(`${name}@${version}`),
  );
  assert.equal(
    recordTuples.length,
    recordAdmitted.size + recordPathPackages.size,
    "a P2-U4 admitted package is missing from Cargo.lock",
  );

  const incomingTuples = lockTuples.filter(
    ([name, version]) =>
      name !== "academic-store-platform" &&
      !keyAdmitted.has(`${name}@${version}`) &&
      !keyPathPackages.has(`${name}@${version}`) &&
      !scenarioAdmitted.has(`${name}@${version}`) &&
      !scenarioPathPackages.has(`${name}@${version}`) &&
      !recoveryAdmitted.has(`${name}@${version}`) &&
      !recoveryPathPackages.has(`${name}@${version}`) &&
      !retentionAdmitted.has(`${name}@${version}`) &&
      !retentionPathPackages.has(`${name}@${version}`) &&
      !admissionAdmitted.has(`${name}@${version}`) &&
      !admissionPathPackages.has(`${name}@${version}`) &&
      !policyAdmitted.has(`${name}@${version}`) &&
      !policyPathPackages.has(`${name}@${version}`) &&
      !processPathPackages.has(`${name}@${version}`) &&
      !transcriptAdmitted.has(`${name}@${version}`) &&
      !transcriptPathPackages.has(`${name}@${version}`) &&
      !egressAdmitted.has(`${name}@${version}`) &&
      !egressPathPackages.has(`${name}@${version}`) &&
      !recordAdmitted.has(`${name}@${version}`) &&
      !recordPathPackages.has(`${name}@${version}`) &&
      !sandboxAdmitted.has(`${name}@${version}`) &&
      !sandboxPathPackages.has(`${name}@${version}`) &&
      !untrustedAdmitted.has(`${name}@${version}`) &&
      !untrustedPathPackages.has(`${name}@${version}`),
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
      keyTuples.length +
      scenarioTuples.length +
      recoveryTuples.length +
      retentionTuples.length +
      admissionTuples.length +
      policyTuples.length +
      processTuples.length +
      transcriptTuples.length +
      egressTuples.length +
      recordTuples.length +
      sandboxTuples.length +
      untrustedTuples.length,
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
    const expectedUses = [
      ...admission.uses,
      ...g4SandboxUse,
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
