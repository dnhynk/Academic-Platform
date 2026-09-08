import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { parsePnpmLockYaml } from "./restricted-yaml.mjs";

export function validateMacosKeystoreWorkflow(text) {
  const workflow = JSON.parse(JSON.stringify(parsePnpmLockYaml(text, "macos-keystore.yml")));
  assert.deepEqual(workflow, {
    name: "macos-keystore-refusal-evidence",
    on: { workflow_dispatch: null, pull_request: { paths: [".github/workflows/macos-keystore.yml", "Cargo.toml", "Cargo.lock", ".nvmrc", "rust-toolchain.toml", "tools/macos-keystore-*", "tools/h1-*.mjs", "tools/source-preflight.mjs", "tools/restricted-yaml.mjs", "crates/keystore-platform/**", "crates/crypto/**", "docs/contracts/macos-device-keystore.md", "docs/security/dependency-admission-phase2-h1.json"] } },
    permissions: { contents: "read" },
    concurrency: { group: "macos-keystore-${{ github.ref }}", "cancel-in-progress": false },
    jobs: { refusal: {
      "runs-on": "macos-latest", "timeout-minutes": 40,
      steps: [
        { name: "Checkout without persisted credentials", uses: "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683", with: { "persist-credentials": false } },
        { name: "Check source and bounded workflow before setup", run: "node tools/source-preflight.mjs && node --test tools/macos-keystore-evidence.test.mjs" },
        { name: "Install pinned Node", uses: "actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020", with: { "node-version-file": ".nvmrc" } },
        { name: "Install pinned Rust toolchain", run: "rustup toolchain install 1.98.0 --profile minimal --component clippy" },
        { name: "Collect unsigned refusal and API evidence", run: "node tools/macos-keystore-run.mjs" },
        { name: "Retain unsigned evidence even on failure", if: "always()", uses: "actions/upload-artifact@bbbca2ddaa5d8feaa63e36b76fdaad77386f024f", with: {
          name: "macos-keystore-${{ github.run_id }}-${{ github.run_attempt }}-${{ github.sha }}", path: "${{ runner.temp }}/macos-keystore-evidence/",
          "if-no-files-found": "error", "retention-days": 30, "compression-level": 6, overwrite: false, "include-hidden-files": false,
        } },
      ],
    } },
  }, "macOS workflow differs from bounded unprovisioned refusal lane");
}

export function assertMacosKeystoreWorkflow() {
  validateMacosKeystoreWorkflow(readFileSync(".github/workflows/macos-keystore.yml", "utf8"));
}
