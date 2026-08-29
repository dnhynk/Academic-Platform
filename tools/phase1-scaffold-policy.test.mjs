import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import test from "node:test";

import Ajv2020 from "ajv/dist/2020.js";

const metadataRun = spawnSync(
  "cargo",
  ["metadata", "--locked", "--offline", "--format-version", "1"],
  { encoding: "utf8" },
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
  { encoding: "utf8" },
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
  { encoding: "utf8" },
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
    "academic-cli": ["academic-core", "academic-daemon", "academic-rpc"],
    "academic-contracts": ["academic-domain"],
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
    "academic-daemon": ["academic-core", "academic-rpc", "academic-store"],
    "academic-domain": [],
    "academic-ledger": ["academic-contracts", "academic-domain"],
    "academic-portability": [
      "academic-contracts",
      "academic-domain",
      "academic-projections",
      "academic-store",
      "academic-vault",
    ],
    "academic-projections": ["academic-domain", "academic-store"],
    "academic-rpc": ["academic-contracts", "academic-domain"],
    "academic-scenario": ["academic-domain"],
    "academic-store": [
      "academic-contracts",
      "academic-domain",
      "academic-ledger",
      "academic-store-platform",
      "academic-vault",
    ],
    "academic-keystore-platform": [],
    "academic-store-platform": [],
    "academic-test-support": [],
    "academic-vault": ["academic-domain"],
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
    "academic-scenario": ["academic-domain"],
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
  const scanned = workspacePackages
    .filter(
      (pkg) =>
        pkg.name !== "academic-test-support" && pkg.name !== "academic-keystore-platform",
    )
    .map((pkg) => [pkg.name, join(dirname(pkg.manifest_path), "src")]);
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
        `${relative} spawns a subprocess and is not one of the two reviewed sites`,
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

test("sqlcipher_feature_is_not_default", () => {
  const storePackage = packagesByName.get("academic-store");
  assert.deepEqual(storePackage.features.default, ["bundled-sqlite"]);
  assert.deepEqual(storePackage.features["bundled-sqlite"], ["rusqlite/bundled"]);
  assert.deepEqual(storePackage.features["sqlcipher-spike"], [
    "rusqlite/bundled-sqlcipher-vendored-openssl",
  ]);
  const storeNode = resolveNodesById.get(storePackage.id);
  assert.equal(storeNode.features.includes("bundled-sqlite"), true);
  assert.equal(storeNode.features.includes("sqlcipher-spike"), false);
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
      name === "academic-store" ? ["bundled-sqlite"] : [],
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
  const [receiptText, keyReceiptText, cargoLock] = await Promise.all([
    readFile("docs/security/dependency-admission-phase1.json", "utf8"),
    readFile("docs/security/dependency-admission-phase2-k1.json", "utf8"),
    readFile("Cargo.lock", "utf8"),
  ]);
  const receipt = JSON.parse(receiptText);
  const keyReceipt = JSON.parse(keyReceiptText);
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

  const incomingTuples = lockTuples.filter(
    ([name, version]) =>
      name !== "academic-store-platform" &&
      !keyAdmitted.has(`${name}@${version}`) &&
      !keyPathPackages.has(`${name}@${version}`),
  );
  assert.equal(incomingTuples.length, receipt.lock_delta.incoming_package_tuple_count);
  assert.equal(
    createHash("sha256").update(JSON.stringify(incomingTuples)).digest("hex"),
    receipt.lock_delta.incoming_package_tuple_sha256,
    "an incoming Cargo.lock package tuple changed",
  );
  assert.equal(
    lockTuples.length,
    receipt.lock_delta.incoming_package_tuple_count + 1 + keyTuples.length,
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
    const expectedUses = [...admission.uses, ...j1ProjectionUse, ...t047FormatTestUse]
      .map((use) => ({ ...use, features: use.features.toSorted() }))
      .toSorted((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
    assert.deepEqual(actualUses, expectedUses, `${admission.name} owner/feature receipt`);
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
