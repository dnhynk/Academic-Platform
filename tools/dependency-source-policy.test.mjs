import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";

import { assertCargoLockSourcePolicy } from "./cargo-lock-source-policy.mjs";
import { assertPnpmLockSourcePolicy } from "./dependency-source-policy.mjs";

const corpora = [
  {
    directory: "tools/fixtures/cargo-source-policy",
    extension: ".toml",
    assertPolicy: assertCargoLockSourcePolicy,
  },
  {
    directory: "tools/fixtures/pnpm-source-policy",
    extension: ".yaml",
    assertPolicy: assertPnpmLockSourcePolicy,
  },
];

let fixtureCount = 0;
for (const corpus of corpora) {
  const fixtureNames = (await readdir(corpus.directory))
    .filter((name) => name.endsWith(corpus.extension))
    .sort();
  assert.ok(fixtureNames.some((name) => name.startsWith("allow-")));
  assert.ok(fixtureNames.some((name) => name.startsWith("reject-")));
  fixtureCount += fixtureNames.length;

  for (const fixtureName of fixtureNames) {
    const fixtureText = await readFile(`${corpus.directory}/${fixtureName}`, "utf8");
    if (fixtureName.startsWith("allow-")) {
      assert.doesNotThrow(
        () => corpus.assertPolicy(fixtureText, fixtureName),
        `${fixtureName} must remain allowed`,
      );
    } else if (fixtureName.startsWith("reject-")) {
      assert.throws(
        () => corpus.assertPolicy(fixtureText, fixtureName),
        undefined,
        `${fixtureName} must be rejected`,
      );
    } else {
      assert.fail(`source policy fixture needs allow-/reject- expectation prefix: ${fixtureName}`);
    }
  }
}

console.log(`Cargo/pnpm structural source-policy corpora passed (${fixtureCount} fixtures).`);
