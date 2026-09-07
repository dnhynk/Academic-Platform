import assert from "node:assert/strict";
import { collect, validate } from "./h1-evidence.mjs";
import { assertRepositorySourcePolicy } from "./source-preflight.mjs";
import { assertH1Workflow } from "./h1-workflow-policy.mjs";

await assertRepositorySourcePolicy();
await assertH1Workflow();
assert(process.env.H1_BUNDLE && process.env.H1_PLATFORM, "hosted bundle and platform required");
const manifest = collect(process.env.H1_BUNDLE, process.env.H1_PLATFORM);
console.log(JSON.stringify(validate(process.env.H1_BUNDLE, {
  platform: process.env.H1_PLATFORM, commit: process.env.GITHUB_SHA,
  runId: process.env.GITHUB_RUN_ID, attempt: process.env.GITHUB_RUN_ATTEMPT,
  repository: process.env.GITHUB_REPOSITORY,
})));
if (manifest.status !== "component_checks_passed") process.exitCode = 1;
