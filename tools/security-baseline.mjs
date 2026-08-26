import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [cargoLock, pnpmLock, fixture] = await Promise.all([
  readFile("Cargo.lock", "utf8"),
  readFile("pnpm-lock.yaml", "utf8"),
  readFile("schemas/fixtures/signed-batch-v1.json", "utf8"),
]);

assert.doesNotMatch(cargoLock, /source = "git\+/u, "Cargo git dependencies require explicit review");
assert.doesNotMatch(pnpmLock, /tarball: http:\/\//u, "insecure package tarballs are forbidden");
assert.doesNotMatch(pnpmLock, /git\+ssh:/u, "pnpm git dependencies require explicit review");
assert.match(fixture, /"data_class": "SYNTHETIC_ONLY"/u);
assert.match(fixture, /"network_egress": "NONE"/u);

console.log("Offline dependency-source and synthetic-fixture baseline passed.");
