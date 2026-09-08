import assert from "node:assert/strict";
import { resolve } from "node:path";
import { collect, validate } from "./macos-keystore-evidence.mjs";
import { assertMacosKeystoreWorkflow } from "./macos-keystore-workflow-policy.mjs";
import { assertRepositorySourcePolicy } from "./source-preflight.mjs";

assertMacosKeystoreWorkflow();
await assertRepositorySourcePolicy();
assert(process.env.RUNNER_TEMP, "hosted temporary directory required");
const bundle = resolve(process.env.RUNNER_TEMP, "macos-keystore-evidence");
const manifest = collect(bundle);
if (manifest.status === "refusal_and_api_checks_passed") {
  console.log(JSON.stringify(validate(bundle, {
    commit: process.env.GITHUB_SHA, runId: process.env.GITHUB_RUN_ID,
    attempt: process.env.GITHUB_RUN_ATTEMPT, repository: process.env.GITHUB_REPOSITORY,
  })));
} else {
  console.log(JSON.stringify({ status: manifest.status, failure: manifest.failure, acceptedH1: false }));
  process.exitCode = 1;
}
