import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const corpusUrl = new URL("./fixtures/tool-version-conformance-v1.json", import.meta.url);

function hasOrdinaryStableMetadata(value) {
  if (!value.startsWith("(") || !value.endsWith(")")) {
    return false;
  }
  const [commit, date, ...extra] = value.slice(1, -1).split(" ");
  return (
    extra.length === 0 &&
    commit !== undefined &&
    /^[0-9a-f]{9,40}$/u.test(commit) &&
    date !== undefined &&
    /^\d{4}-\d{2}-\d{2}$/u.test(date)
  );
}

export function isSupportedToolVersion(specification, output) {
  const observed = output.trim();
  if (specification.policy === "exact") {
    return observed === specification.expected;
  }
  if (specification.policy === "stable-rust-tool") {
    if (observed === specification.expected) {
      return true;
    }
    const prefix = `${specification.expected} `;
    return observed.startsWith(prefix) && hasOrdinaryStableMetadata(observed.slice(prefix.length));
  }
  throw new Error(`unsupported tool-version policy ${JSON.stringify(specification.policy)}`);
}

export async function loadToolVersionConformanceCorpus() {
  const parsed = JSON.parse(await readFile(corpusUrl, "utf8"));
  assert.equal(parsed.schema_version, 1, "tool-version corpus schema version");
  assert.ok(Array.isArray(parsed.tools) && parsed.tools.length > 0, "tool-version corpus tools");
  return parsed;
}

export function assertToolVersionConformanceCorpus(corpus) {
  assert.deepEqual(
    corpus.tools.map((tool) => tool.name),
    ["rustc", "cargo", "node", "pnpm"],
    "tool-version corpus must cover every exact repository pin in doctor order",
  );
  for (const tool of corpus.tools) {
    assert.ok(tool.expected.length > 0, `${tool.name} expected output must be nonempty`);
    assert.ok(tool.remediation.length > 0, `${tool.name} remediation must be nonempty`);
    assert.ok(tool.cases.some((testCase) => testCase.supported), `${tool.name} positive cases`);
    assert.ok(tool.cases.some((testCase) => !testCase.supported), `${tool.name} negative cases`);
    for (const testCase of tool.cases) {
      assert.equal(
        isSupportedToolVersion(tool, testCase.output),
        testCase.supported,
        `${tool.name}: ${testCase.name}`,
      );
    }
  }
}
