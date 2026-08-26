import { spawnSync } from "node:child_process";

import { assertRepositorySourcePolicy } from "./source-preflight.mjs";

await assertRepositorySourcePolicy();

const expected = new Map([
  ["node", "v24.19.0"],
  ["pnpm", "11.22.0"],
  ["rustc", "rustc 1.98.0"],
  ["cargo", "cargo 1.98.0"],
]);

for (const [tool, version] of expected) {
  const result = spawnSync(tool, ["--version"], { encoding: "utf8" });
  const observed = result.stdout.trim();
  if (result.status !== 0 || !observed.startsWith(version)) {
    throw new Error(`${tool}: expected ${version}, observed ${observed || "missing"}`);
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
