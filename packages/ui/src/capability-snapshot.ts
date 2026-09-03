/**
 * The committed Tauri capability and CSP snapshot, and what is refused in it.
 *
 * `P2-A2` and its re-audit left the desktop capability snapshot diff `NOT_RUN`
 * because there was no snapshot to diff. `crates/desktop/tauri.conf.json` and
 * `crates/desktop/capabilities/desktop.json` are that snapshot. They are the
 * formats Tauri itself reads: `schemas/tauri/config-2.11.5.schema.json` is
 * Tauri's own published schema for the version this repository measured, and
 * `schemas/tauri/capability-2.9.3.schema.json` was generated from
 * `tauri_utils::acl::capability::Capability` with `schemars`. Neither crate is
 * a dependency of this repository; the schemas are vendored data.
 *
 * No Tauri runtime is linked, so nothing here opens a window. What the snapshot
 * is evidence for is its own content: an audit can diff these two files and
 * this module's rules decide whether the content grants breadth.
 *
 * ## Three layers, and which one catches what
 *
 * 1. **Whole-file pins.** Each snapshot file and each vendored schema is pinned
 *    by the SHA-256 of its whole bytes, as `packages/web-contracts` pins the
 *    local-core Proto. Any edit to any of the four fails, whether or not it
 *    names anything anyone thought to forbid.
 * 2. **A closed value world.** Every string that appears anywhere in either
 *    snapshot document, at any depth, must be one of the strings in
 *    {@link ALLOWED_SNAPSHOT_STRINGS}. This is the layer that catches a
 *    wildcard written in a form nobody enumerated, because it does not ask what
 *    a string means -- it asks whether that exact string was reviewed.
 * 3. **Authority rules.** The permission list, the asset-protocol scope, the
 *    CSP directives, the capability's window list and its `remote` key are each
 *    compared against an exact expected value. These are what say *why* a
 *    document is refused, and they are what {@link WILDCARD_FORMS} explains.
 *
 * {@link WILDCARD_FORMS} is an enumeration for the failure message and for the
 * reader. It is deliberately not the thing that decides: a deny list of shapes
 * is broken by the shape that is not on it, which is the empty-guard defect
 * `docs/contracts/policy-source-scans.md` is about. Layer 2 decides.
 */

/** One refusal. */
export interface SnapshotViolation {
  /** JSON pointer of the offending value inside its document. */
  readonly pointer: string;
  /** Which rule refused it. */
  readonly rule: string;
  /** What was wrong. */
  readonly detail: string;
}

/** The two documents of the snapshot, parsed. */
export interface SnapshotDocuments {
  readonly config: unknown;
  readonly capabilities: readonly unknown[];
}

/**
 * Named wildcard shapes, for the failure message.
 *
 * Every one of these is a form that grants breadth in a Tauri capability, an
 * asset-protocol scope, or a CSP source list. `FsScope` in Tauri's own schema
 * is documented as "a list of glob patterns" whose entries may start with
 * `$HOME`, `$DATA` and the other base-directory variables, so the variable
 * forms below are Tauri syntax rather than an invention.
 *
 * This table explains. It does not decide -- see the module documentation.
 */
export const WILDCARD_FORMS: readonly {
  readonly name: string;
  readonly matches: (value: string) => boolean;
}[] = [
  { name: "single-star glob", matches: (value) => /(?:^|[^*])\*(?:[^*]|$)/u.test(value) },
  { name: "double-star glob", matches: (value) => value.includes("**") },
  { name: "base-directory variable", matches: (value) => /\$[A-Z]+/u.test(value) },
  { name: "insecure http scheme", matches: (value) => /^http:\/\//iu.test(value) },
  { name: "scheme wildcard", matches: (value) => /^[a-z][a-z0-9+.-]*:\/\/\*/iu.test(value) },
  {
    name: "scheme-less host",
    matches: (value) => /^(?:[a-z0-9-]+\.)+[a-z]{2,}(?::\d+)?(?:\/|$)/iu.test(value),
  },
  { name: "csp wildcard source", matches: (value) => value.trim() === "*" },
  { name: "question-mark glob", matches: (value) => value.includes("?") },
  { name: "brace expansion", matches: (value) => /\{[^}]*,[^}]*\}/u.test(value) },
  { name: "path traversal", matches: (value) => value.split(/[/\\]/u).includes("..") },
];

/** The named wildcard forms `value` carries, if any. */
export function wildcardForms(value: string): readonly string[] {
  return WILDCARD_FORMS.filter((form) => form.matches(value)).map((form) => form.name);
}

/**
 * Every string the snapshot is allowed to contain, anywhere, at any depth.
 *
 * A string that is not here is refused wherever it appears and whatever it
 * means. Adding one is a review of that exact string.
 */
export const ALLOWED_SNAPSHOT_STRINGS: ReadonlySet<string> = new Set([
  // tauri.conf.json
  "../../schemas/tauri/config-2.11.5.schema.json",
  "Academic OS",
  "dev.academic-os.desktop",
  "0.1.0",
  "../../packages/ui/dist",
  "desktop",
  "'self'",
  "'none'",
  "ipc:",
  "main",
  // capabilities/desktop.json
  "The single main-window capability. It carries core permissions only; filesystem, HTTP and shell authority in Tauri v2 arrive through the tauri-plugin-fs, tauri-plugin-http and tauri-plugin-shell crates, and crates/desktop declares no plugin dependency of any kind.",
  "core:default",
]);

/** The permission identifiers the capability may carry. */
export const ALLOWED_PERMISSIONS: readonly string[] = ["core:default"];

/** The window labels the capability and the configuration may name. */
export const ALLOWED_WINDOW_LABELS: readonly string[] = ["main"];

/** The CSP, directive by directive, exactly. */
export const EXPECTED_CSP: ReadonlyMap<string, readonly string[]> = new Map([
  ["default-src", ["'self'"]],
  ["script-src", ["'self'"]],
  ["style-src", ["'self'"]],
  ["img-src", ["'self'"]],
  ["font-src", ["'self'"]],
  ["connect-src", ["'self'", "ipc:"]],
  ["media-src", ["'none'"]],
  ["object-src", ["'none'"]],
  ["worker-src", ["'none'"]],
  ["child-src", ["'none'"]],
  ["frame-src", ["'none'"]],
  ["frame-ancestors", ["'none'"]],
  ["form-action", ["'none'"]],
  ["base-uri", ["'none'"]],
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function pointerSegment(key: string): string {
  return key.replaceAll("~", "~0").replaceAll("/", "~1");
}

/** One string found inside a document, and whether it was a key or a value. */
export interface StringLeaf {
  readonly pointer: string;
  readonly text: string;
  readonly position: "key" | "value";
}

/** Every string in `document`, keys included, with its JSON pointer. */
export function stringLeaves(document: unknown, prefix = ""): readonly StringLeaf[] {
  if (typeof document === "string") {
    return [{ pointer: prefix, text: document, position: "value" }];
  }
  if (Array.isArray(document)) {
    return document.flatMap((item, index) => stringLeaves(item, `${prefix}/${String(index)}`));
  }
  if (isRecord(document)) {
    return Object.entries(document).flatMap(([key, value]) => {
      const pointer = `${prefix}/${pointerSegment(key)}`;
      return [
        { pointer, text: key, position: "key" as const },
        ...stringLeaves(value, pointer),
      ];
    });
  }
  return [];
}

/**
 * Layer 2: every string in the documents was reviewed.
 *
 * Keys and values are separate closed sets. A key is a string a Tauri
 * configuration reads -- a CSP directive name and a plugin name are both keys
 * -- so keys are checked too; and they are checked against their own set, so
 * that a reviewed key name cannot become a reviewed value somewhere else.
 */
function closedValueWorld(documents: SnapshotDocuments): readonly SnapshotViolation[] {
  const violations: SnapshotViolation[] = [];
  const allowedKeys = new Set([...CONFIG_STRUCTURAL_KEYS, ...EXPECTED_CSP.keys()]);
  const sources: readonly { readonly label: string; readonly document: unknown }[] = [
    { label: "config", document: documents.config },
    ...documents.capabilities.map((document, index) => ({
      label: `capability[${String(index)}]`,
      document,
    })),
  ];
  for (const { label, document } of sources) {
    for (const leaf of stringLeaves(document, "")) {
      const allowed = leaf.position === "key" ? allowedKeys : ALLOWED_SNAPSHOT_STRINGS;
      if (allowed.has(leaf.text)) {
        continue;
      }
      const forms = wildcardForms(leaf.text);
      violations.push({
        pointer: `${label}${leaf.pointer}`,
        rule: `closed-value-world/${leaf.position}`,
        detail:
          forms.length > 0
            ? `${JSON.stringify(leaf.text)} is not a reviewed snapshot ${leaf.position} and carries ${forms.join(", ")}`
            : `${JSON.stringify(leaf.text)} is not a reviewed snapshot ${leaf.position}`,
      });
    }
  }
  return violations;
}

/**
 * The structural key names of the two documents.
 *
 * Keys are strings, and holding them in the reviewed value set would let a
 * reviewed key name become a reviewed *value* somewhere else. They are a
 * separate closed set for that reason, and the set is exact: a key that is not
 * here fails, which is how a `scope`, an `http` plugin entry or a `remote`
 * block is refused without anyone naming it as forbidden.
 */
export const CONFIG_STRUCTURAL_KEYS: ReadonlySet<string> = new Set([
  "$schema",
  "productName",
  "identifier",
  "version",
  "build",
  "frontendDist",
  "app",
  "withGlobalTauri",
  "security",
  "freezePrototype",
  "dangerousDisableAssetCspModification",
  "assetProtocol",
  "enable",
  "scope",
  "capabilities",
  "csp",
  "windows",
  "label",
  "title",
  "width",
  "height",
  "plugins",
  "description",
  "local",
  "permissions",
]);

function expectExact(
  pointer: string,
  rule: string,
  actual: unknown,
  expected: unknown,
): readonly SnapshotViolation[] {
  if (JSON.stringify(actual) === JSON.stringify(expected)) {
    return [];
  }
  return [
    {
      pointer,
      rule,
      detail: `expected ${JSON.stringify(expected)}, found ${JSON.stringify(actual)}`,
    },
  ];
}

/** Layer 3: the fields that carry authority, each compared exactly. */
function authorityRules(documents: SnapshotDocuments): readonly SnapshotViolation[] {
  const violations: SnapshotViolation[] = [];
  const config = documents.config;
  if (!isRecord(config)) {
    return [{ pointer: "config", rule: "shape", detail: "the configuration is not an object" }];
  }
  const app = isRecord(config["app"]) ? config["app"] : {};
  const security = isRecord(app["security"]) ? app["security"] : {};
  const assetProtocol = isRecord(security["assetProtocol"]) ? security["assetProtocol"] : {};

  violations.push(
    ...expectExact("config/plugins", "no-plugin-declares-authority", config["plugins"], {}),
    ...expectExact("config/app/withGlobalTauri", "no-global-tauri", app["withGlobalTauri"], false),
    ...expectExact(
      "config/app/security/freezePrototype",
      "prototype-frozen",
      security["freezePrototype"],
      true,
    ),
    ...expectExact(
      "config/app/security/dangerousDisableAssetCspModification",
      "csp-modification-not-disabled",
      security["dangerousDisableAssetCspModification"],
      false,
    ),
    ...expectExact(
      "config/app/security/assetProtocol/enable",
      "asset-protocol-disabled",
      assetProtocol["enable"],
      false,
    ),
    ...expectExact(
      "config/app/security/assetProtocol/scope",
      "asset-protocol-scope-empty",
      assetProtocol["scope"],
      [],
    ),
  );

  const csp = security["csp"];
  if (!isRecord(csp)) {
    violations.push({
      pointer: "config/app/security/csp",
      rule: "csp-is-a-directive-map",
      detail: "the CSP must be a directive map, so each directive can be compared on its own",
    });
  } else {
    const seen = new Set(Object.keys(csp));
    for (const [directive, sources] of EXPECTED_CSP) {
      violations.push(
        ...expectExact(`config/app/security/csp/${directive}`, "csp-directive", csp[directive], sources),
      );
      seen.delete(directive);
    }
    for (const extra of seen) {
      violations.push({
        pointer: `config/app/security/csp/${extra}`,
        rule: "csp-directive",
        detail: "the CSP carries a directive the snapshot does not pin",
      });
    }
  }

  const configWindows = Array.isArray(app["windows"]) ? app["windows"] : [];
  configWindows.forEach((window, index) => {
    const label = isRecord(window) ? window["label"] : undefined;
    if (typeof label !== "string" || !ALLOWED_WINDOW_LABELS.includes(label)) {
      violations.push({
        pointer: `config/app/windows/${String(index)}/label`,
        rule: "window-label",
        detail: `window label ${JSON.stringify(label)} is not one of ${JSON.stringify(ALLOWED_WINDOW_LABELS)}`,
      });
    }
  });

  documents.capabilities.forEach((capability, index) => {
    const at = `capability[${String(index)}]`;
    if (!isRecord(capability)) {
      violations.push({ pointer: at, rule: "shape", detail: "the capability is not an object" });
      return;
    }
    if ("remote" in capability) {
      violations.push({
        pointer: `${at}/remote`,
        rule: "no-remote-origin",
        detail: "a capability that names remote origins extends its permissions to content this app did not build",
      });
    }
    violations.push(
      ...expectExact(`${at}/permissions`, "permission-allowlist", capability["permissions"], ALLOWED_PERMISSIONS),
      ...expectExact(`${at}/windows`, "window-allowlist", capability["windows"], ALLOWED_WINDOW_LABELS),
      ...expectExact(`${at}/local`, "local-only", capability["local"], true),
    );
  });

  return violations;
}

/**
 * Scans the parsed snapshot.
 *
 * Both layers always run and both sets of findings are returned, so a document
 * that a single rule would have caught still shows every reason it was refused.
 */
export function scanSnapshot(documents: SnapshotDocuments): readonly SnapshotViolation[] {
  return [...closedValueWorld(documents), ...authorityRules(documents)];
}
