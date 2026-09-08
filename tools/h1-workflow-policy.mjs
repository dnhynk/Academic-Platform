import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { parsePnpmLockYaml } from "./restricted-yaml.mjs";
import { PLATFORMS } from "./h1-plan.mjs";

export function validateH1Workflow(text) {
  const workflow = JSON.parse(JSON.stringify(parsePnpmLockYaml(text, "h1-evidence.yml")));
  const steps = [
    { name: "Checkout without persisted credentials", uses: "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683", with: { "persist-credentials": false } },
    { name: "Reject unreviewed sources and workflow before setup", run: "node tools/source-preflight.mjs" },
    { name: "Verify unsigned evidence contract", run: "node --test tools/h1-evidence.test.mjs" },
    { name: "Install pinned Node", uses: "actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020", with: { "node-version-file": ".nvmrc" } },
    { name: "Install pinned Rust toolchain", run: "rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy" },
    { name: "Collect native encrypted component evidence", if: "always()", env: { H1_BUNDLE: "${{ runner.temp }}/h1-evidence", H1_PLATFORM: "${{ matrix.platform }}" }, run: "node tools/h1-run.mjs" },
    { name: "Retain unsigned evidence even on failure", if: "always()", uses: "actions/upload-artifact@bbbca2ddaa5d8feaa63e36b76fdaad77386f024f", with: {
      name: "h1-${{ matrix.platform }}-${{ github.run_id }}-${{ github.run_attempt }}-${{ github.sha }}",
      path: "${{ runner.temp }}/h1-evidence/", "if-no-files-found": "error", "retention-days": 30,
      "compression-level": 6, overwrite: false, "include-hidden-files": false,
    } },
  ];
  assert.deepEqual(workflow, {
    name: "h1-unsigned-evidence",
    on: { workflow_dispatch: null, pull_request: { paths: [".github/workflows/h1-evidence.yml", "tools/h1-*", "docs/contracts/h1-*", "docs/security/h1-*"] } },
    permissions: { contents: "read" },
    concurrency: { group: "h1-evidence-${{ github.ref }}", "cancel-in-progress": false },
    jobs: { encrypted: {
      name: "h1-encrypted-${{ matrix.platform }}", "runs-on": "${{ matrix.os }}", "timeout-minutes": 120,
      strategy: { "fail-fast": false, matrix: { include: Object.entries(PLATFORMS).map(([platform, [os]]) => ({ platform, os })) } },
      env: { CARGO_BUILD_JOBS: "1", CARGO_INCREMENTAL: "0", CARGO_TERM_COLOR: "never" }, steps,
    } },
  }, "H1 workflow differs from reviewed bounded execution policy");
}

export async function assertH1Workflow(root = process.cwd()) {
  validateH1Workflow(await readFile(resolve(root, ".github/workflows/h1-evidence.yml"), "utf8"));
}
