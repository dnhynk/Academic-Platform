import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";

import { assertPnpmLockSourcePolicy } from "./dependency-source-policy.mjs";

const fixtureDirectory = "tools/fixtures/pnpm-source-policy";
const fixtureNames = (await readdir(fixtureDirectory))
  .filter((name) => name.endsWith(".yaml"))
  .sort();
assert.ok(fixtureNames.some((name) => name.startsWith("allow-")));
assert.ok(fixtureNames.some((name) => name.startsWith("reject-")));

for (const fixtureName of fixtureNames) {
  const fixtureText = await readFile(`${fixtureDirectory}/${fixtureName}`, "utf8");
  if (fixtureName.startsWith("allow-")) {
    assert.doesNotThrow(
      () => assertPnpmLockSourcePolicy(fixtureText, fixtureName),
      `${fixtureName} must remain allowed`,
    );
  } else if (fixtureName.startsWith("reject-")) {
    assert.throws(
      () => assertPnpmLockSourcePolicy(fixtureText, fixtureName),
      undefined,
      `${fixtureName} must be rejected`,
    );
  } else {
    assert.fail(`source policy fixture needs allow-/reject- expectation prefix: ${fixtureName}`);
  }
}

console.log(`pnpm structural source-policy corpus passed (${fixtureNames.length} fixtures).`);
