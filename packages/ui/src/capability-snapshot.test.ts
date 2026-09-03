/**
 * `capability_snapshot_has_no_wildcard`.
 *
 * Three things are checked, and they fail for different reasons on purpose.
 *
 * 1. The four files are pinned by the SHA-256 of their whole bytes, the way
 *    `packages/web-contracts` pins the local-core Proto. Any edit fails.
 * 2. Both snapshot documents validate against the schemas Tauri itself uses, so
 *    the committed format is the format the runtime will read when `P2-X1`'s
 *    successor links it. A negative control shows the schema is doing work --
 *    and a second one shows the schema is *not* the wildcard guard, because it
 *    accepts `$HOME/**` in an asset-protocol scope quite happily.
 * 3. `scanSnapshot` refuses breadth. The injections below are the point: three
 *    of them use wildcard forms that `WILDCARD_FORMS` does not name, and they
 *    are refused anyway, because the rule that decides is a closed world over
 *    reviewed strings rather than a list of bad shapes.
 */

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { Ajv } from "ajv";

import {
  ALLOWED_PERMISSIONS,
  scanSnapshot,
  wildcardForms,
  WILDCARD_FORMS,
  type SnapshotDocuments,
} from "./capability-snapshot.js";

const repositoryRoot = new URL("../../../", import.meta.url);

/** The four pinned files. Editing any of them means editing this table. */
const PINNED_FILES: readonly (readonly [string, string])[] = [
  [
    "crates/desktop/tauri.conf.json",
    "89df304a70c7854f72ee3c6ded20b70feef83e1f92f09123b829297e28a7bb75",
  ],
  [
    "crates/desktop/capabilities/desktop.json",
    "54efe90b8f50836a6fa316521198f490c6497aca7063e1ec39622fad65865167",
  ],
  [
    "schemas/tauri/config-2.11.5.schema.json",
    "6928b54f49574a13d8c597effa0f429853d19ddf3f6da5329751cff3848510de",
  ],
  [
    "schemas/tauri/capability-2.9.3.schema.json",
    "594f80302568a21ac0e54ee11716e1f68911bf114df9e9f70c9d444b415489b6",
  ],
];

async function readSnapshotFile(relative: string): Promise<string> {
  return readFile(new URL(relative, repositoryRoot), "utf8");
}

async function readSnapshot(): Promise<SnapshotDocuments> {
  return {
    config: JSON.parse(await readSnapshotFile("crates/desktop/tauri.conf.json")) as unknown,
    capabilities: [
      JSON.parse(await readSnapshotFile("crates/desktop/capabilities/desktop.json")) as unknown,
    ],
  };
}

/**
 * Ajv compiled for Tauri's own draft-07 schemas.
 *
 * Two accommodations, both about the schema rather than about the snapshot.
 * Tauri's schema carries a pattern with a `\:` escape, which is invalid under
 * the unicode flag Ajv adds by default, so patterns are compiled without it.
 * And it annotates numbers with `format: "double"` and URLs with
 * `format: "uri"`, neither of which Ajv defines; format assertions are
 * annotations here and are switched off rather than stubbed with definitions
 * this repository would then own.
 */
function tauriAjv(): Ajv {
  function withoutUnicodeFlag(pattern: string, flags: string): RegExp {
    return new RegExp(pattern, flags.replace("u", ""));
  }
  withoutUnicodeFlag.code = "new RegExp";
  return new Ajv({
    allErrors: true,
    strict: true,
    validateFormats: false,
    code: { regExp: withoutUnicodeFlag },
  });
}

void test("capability_snapshot_has_no_wildcard: the snapshot and its schemas are pinned", async () => {
  for (const [relative, expected] of PINNED_FILES) {
    const bytes = await readFile(new URL(relative, repositoryRoot));
    assert.equal(
      createHash("sha256").update(bytes).digest("hex"),
      expected,
      `${relative} changed; review the whole file and update this pin`,
    );
  }
});

void test("capability_snapshot_has_no_wildcard: the snapshot is the format Tauri reads", async () => {
  const ajv = tauriAjv();
  const configSchema = JSON.parse(
    await readSnapshotFile("schemas/tauri/config-2.11.5.schema.json"),
  ) as object;
  const capabilitySchema = JSON.parse(
    await readSnapshotFile("schemas/tauri/capability-2.9.3.schema.json"),
  ) as object;
  const validateConfig = ajv.compile(configSchema);
  const validateCapability = ajv.compile(capabilitySchema);
  const snapshot = await readSnapshot();

  assert.ok(
    validateConfig(snapshot.config),
    `tauri.conf.json does not validate: ${ajv.errorsText(validateConfig.errors)}`,
  );
  for (const capability of snapshot.capabilities) {
    assert.ok(
      validateCapability(capability),
      `a capability does not validate: ${ajv.errorsText(validateCapability.errors)}`,
    );
  }

  // The schema is doing work.
  const clone = (value: unknown): Record<string, unknown> =>
    JSON.parse(JSON.stringify(value)) as Record<string, unknown>;
  const unknownKey = clone(snapshot.config);
  unknownKey["unreviewedTopLevelKey"] = true;
  assert.equal(validateConfig(unknownKey), false, "the schema accepted an unknown top-level key");
  const withoutIdentifier = clone(snapshot.config);
  delete withoutIdentifier["identifier"];
  assert.equal(validateConfig(withoutIdentifier), false, "the schema accepted a missing identifier");
  const withoutPermissions = clone(snapshot.capabilities[0]);
  delete withoutPermissions["permissions"];
  assert.equal(
    validateCapability(withoutPermissions),
    false,
    "the schema accepted a capability with no permissions",
  );

  // And the schema is not the wildcard guard: it accepts a glob scope, which is
  // exactly why `scanSnapshot` exists beside it.
  const globbedScope = clone(snapshot.config);
  const app = globbedScope["app"] as Record<string, unknown>;
  const security = app["security"] as Record<string, unknown>;
  security["assetProtocol"] = { enable: true, scope: ["$HOME/**"] };
  assert.equal(
    validateConfig(globbedScope),
    true,
    "Tauri's schema has started refusing glob scopes; the note beside this assertion is stale",
  );
  assert.notDeepEqual(scanSnapshot({ ...(await readSnapshot()), config: globbedScope }), []);
});

void test("capability_snapshot_has_no_wildcard: the committed snapshot grants no breadth", async () => {
  assert.deepEqual(scanSnapshot(await readSnapshot()), []);
});

void test("capability_snapshot_has_no_wildcard rejects its violations", async () => {
  const base = await readSnapshot();
  const mutate = (
    change: (documents: { config: Record<string, unknown>; capability: Record<string, unknown> }) => void,
  ): SnapshotDocuments => {
    const config = JSON.parse(JSON.stringify(base.config)) as Record<string, unknown>;
    const capability = JSON.parse(JSON.stringify(base.capabilities[0])) as Record<string, unknown>;
    change({ config, capability });
    return { config, capabilities: [capability] };
  };
  const security = (config: Record<string, unknown>): Record<string, unknown> =>
    (config["app"] as Record<string, unknown>)["security"] as Record<string, unknown>;

  /**
   * Each injection, the wildcard form it uses, and whether `WILDCARD_FORMS`
   * names that form.
   *
   * The three marked `named: false` are the ones that matter. They are wildcard
   * grants written in shapes the enumeration does not describe, and they are
   * refused all the same.
   */
  const injections: readonly {
    readonly what: string;
    readonly named: boolean;
    readonly apply: (documents: {
      config: Record<string, unknown>;
      capability: Record<string, unknown>;
    }) => void;
  }[] = [
    {
      what: "a double-star filesystem scope",
      named: true,
      apply: ({ config }) => {
        security(config)["assetProtocol"] = { enable: true, scope: ["$HOME/**"] };
      },
    },
    {
      what: "a wildcard CSP source",
      named: true,
      apply: ({ config }) => {
        (security(config)["csp"] as Record<string, unknown>)["connect-src"] = ["*"];
      },
    },
    {
      what: "an insecure http scheme in a CSP source",
      named: true,
      apply: ({ config }) => {
        (security(config)["csp"] as Record<string, unknown>)["img-src"] = ["http://**"];
      },
    },
    {
      what: "a scheme-less host in a remote capability origin",
      named: true,
      apply: ({ capability }) => {
        capability["remote"] = { urls: ["example.com"] };
      },
    },
    {
      what: "a filesystem plugin permission",
      named: false,
      apply: ({ capability }) => {
        capability["permissions"] = [...ALLOWED_PERMISSIONS, "fs:allow-read-text-file"];
      },
    },
    {
      what: "a shell plugin declared with an empty configuration",
      named: false,
      apply: ({ config }) => {
        config["plugins"] = { shell: {} };
      },
    },
    {
      what: "a filesystem scope written as a fullwidth asterisk",
      named: false,
      apply: ({ config }) => {
        security(config)["assetProtocol"] = { enable: true, scope: ["\uFF0A"] };
      },
    },
    {
      what: "a filesystem scope written as an explicit drive root with no metacharacter",
      named: false,
      apply: ({ config }) => {
        security(config)["assetProtocol"] = { enable: true, scope: ["C:\\"] };
      },
    },
    {
      what: "a protocol-relative CSP source",
      named: false,
      apply: ({ config }) => {
        (security(config)["csp"] as Record<string, unknown>)["script-src"] = [
          "'self'",
          "//cdn.example.net",
        ];
      },
    },
    {
      what: "a data-scheme CSP source, which names no host at all",
      named: false,
      apply: ({ config }) => {
        (security(config)["csp"] as Record<string, unknown>)["img-src"] = ["'self'", "data:"];
      },
    },
    {
      what: "a CSP that drops a directive instead of widening one",
      named: false,
      apply: ({ config }) => {
        delete (security(config)["csp"] as Record<string, unknown>)["object-src"];
      },
    },
  ];

  for (const injection of injections) {
    const injected = mutate(injection.apply);
    const violations = scanSnapshot(injected);
    assert.ok(violations.length > 0, `${injection.what} was accepted`);
  }

  // At least three of the injections use a shape the enumeration does not name,
  // and all three are refused. That is the claim: the enumeration explains, the
  // closed world decides.
  const unnamed = injections.filter((injection) => !injection.named);
  assert.ok(unnamed.length >= 3, "fewer than three injections use an unenumerated shape");
  for (const injection of unnamed) {
    assert.ok(scanSnapshot(mutate(injection.apply)).length > 0, `${injection.what} was accepted`);
  }

  // And reverting yields a clean scan, so the refusals above are the injections
  // rather than a scanner that refuses everything.
  assert.deepEqual(scanSnapshot(mutate(() => undefined)), []);
});

void test("the wildcard enumeration is not vacuous and does not decide", () => {
  // Each named form matches at least one string, so a form that stopped
  // matching anything is a broken row rather than a silent pass.
  const samples: Record<string, string> = {
    "single-star glob": "/etc/*",
    "double-star glob": "$DATA/**",
    "base-directory variable": "$HOME/notes",
    "insecure http scheme": "http://example.com",
    "scheme wildcard": "https://*.example.com",
    "scheme-less host": "example.com/path",
    "csp wildcard source": "*",
    "question-mark glob": "file?.txt",
    "brace expansion": "{a,b}",
    "path traversal": "../../etc/passwd",
  };
  for (const form of WILDCARD_FORMS) {
    const sample = samples[form.name];
    assert.ok(sample !== undefined, `${form.name} has no sample`);
    assert.ok(form.matches(sample), `${form.name} does not match its own sample`);
  }
  assert.deepEqual(
    WILDCARD_FORMS.map((form) => form.name).toSorted(),
    Object.keys(samples).toSorted(),
    "the sample table and the form table have drifted apart",
  );

  // A reviewed snapshot string carries no named form, so the enumeration is not
  // simply matching everything.
  assert.deepEqual(wildcardForms("'self'"), []);
  assert.deepEqual(wildcardForms("core:default"), []);
  assert.deepEqual(wildcardForms("main"), []);

  // And a shape no row names carries no form at all, which is precisely why the
  // enumeration cannot be what decides.
  assert.deepEqual(wildcardForms("\uFF0A"), []);
  assert.deepEqual(wildcardForms("fs:allow-read-text-file"), []);
});
