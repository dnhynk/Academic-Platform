import { spawnSync } from "node:child_process";

import { assertRepositorySourcePolicy } from "./source-preflight.mjs";
import {
  assertToolVersionConformanceCorpus,
  isSupportedToolVersion,
  loadToolVersionConformanceCorpus,
} from "./tool-version-policy.mjs";

await assertRepositorySourcePolicy();

const versionCorpus = await loadToolVersionConformanceCorpus();
assertToolVersionConformanceCorpus(versionCorpus);

for (const tool of versionCorpus.tools) {
  const result = spawnSync(tool.name, ["--version"], { encoding: "utf8" });
  const observed = result.stdout.trim();
  if (result.status !== 0 || !isSupportedToolVersion(tool, observed)) {
    throw new Error(
      `${tool.name}: expected ${tool.expected}, observed ${observed || "missing"}`,
    );
  }
}

for (const [command, args] of [
  ["pnpm", ["install", "--frozen-lockfile"]],
  ["cargo", ["fetch", "--locked"]],
]) {
  const result = spawnSync(command, args, { stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed`);
  }
}

console.log("Bootstrap complete. Only synthetic fixtures are permitted until ADR-002 is accepted.");
