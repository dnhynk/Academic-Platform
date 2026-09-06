import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../", import.meta.url);
const read = (path) => readFile(new URL(path, root), "utf8");
const receipt = JSON.parse(await read("docs/security/dependency-admission-phase2-x1b.json"));
const result = spawnSync("cargo", ["metadata", "--format-version", "1", "--locked", "--offline", "--features", "academic-desktop/desktop-runtime"], { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
assert.equal(result.status, 0, result.stderr);
const metadata = JSON.parse(result.stdout);
const packages = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
const desktop = metadata.packages.find((pkg) => pkg.name === "academic-desktop");
const seen = new Set();
function walk(id) { if (seen.has(id)) return; seen.add(id); for (const edge of nodes.get(id).deps) walk(edge.pkg); }
walk(desktop.id);

test("optional runtime has an exact admitted closure and no store or key owner", () => {
  assert.deepEqual([...seen].map((id) => `${packages.get(id).name}@${packages.get(id).version}`).sort(), receipt.runtime_closure);
  const forbidden = new Set(["academic-core", "academic-store", "academic-store-platform", "academic-vault", "academic-crypto", "academic-keystore-platform", "academic-daemon", "academic-cli", "rusqlite", "libsqlite3-sys", "sqlx", "sqlite", "keyring", "argon2", "chacha20poly1305", "hkdf", "openssl", "ring", "rustls", "security-framework", "secret-service"]);
  for (const id of seen) {
    const pkg = packages.get(id);
    assert.equal(forbidden.has(pkg.name), false, pkg.name);
    assert.equal(pkg.name.startsWith("tauri-plugin-"), false, pkg.name);
  }
  for (const admission of receipt.admissions) {
    const pkg = metadata.packages.find((p) => p.name === admission.name && p.version === admission.version);
    assert.ok(pkg, admission.name);
    assert.equal(pkg.license, admission.license, `${pkg.name} license`);
    assert.equal(pkg.source, admission.source, `${pkg.name} source`);
    assert.deepEqual(nodes.get(pkg.id).features, admission.admitted_features, `${pkg.name} feature review`);
  }
  assert.deepEqual(desktop.features.default, []);
  assert.deepEqual(desktop.targets.find((target) => target.kind.includes("bin"))["required-features"], ["desktop-runtime"]);
  const tauri = desktop.dependencies.find((dep) => dep.name === "tauri");
  assert.equal(tauri.optional, true);
  assert.equal(tauri.uses_default_features, false);
  assert.deepEqual(tauri.features, ["wry", "custom-protocol", "x11"]);
  assert.equal(desktop.dependencies.find((dep) => dep.name === "tauri-build").optional, true);
});

test("runtime invoke manifest and capability describe exactly the same single command", async () => {
  const build = await read("crates/desktop/build.rs");
  const runtime = await read("crates/desktop/src/runtime.rs");
  const cap = JSON.parse(await read("crates/desktop/capabilities/desktop.json"));
  assert.match(build, /commands\(&\["desktop_request_v1"\]\)/u);
  assert.match(runtime, /generate_handler!\[desktop_request_v1\]/u);
  assert.deepEqual([...runtime.matchAll(/#\[tauri::command\]\s+async fn (\w+)/gu)].map((match) => match[1]), ["desktop_request_v1"]);
  assert.deepEqual(cap.permissions, ["allow-desktop-request-v1"]);
  assert.deepEqual(cap.windows, ["main"]);
  assert.equal(cap.local, true);
  assert.equal(Object.hasOwn(cap, "remote"), false);
});

test("desktop filesystem use is the bounded host-selected session read only", async () => {
  const source = await read("crates/desktop/src/local_client.rs");
  assert.deepEqual([...source.matchAll(/File::(\w+)/gu)].map((m) => m[1]), ["open"]);
  assert.match(source, /file_name\(\)\.is_none_or\(\|name\| name != "session.meta"\)/u);
  assert.match(source, /File::open\(path\)\?\.take\(4097\)\.read_to_string/u);
  assert.match(source, /timeout\(Duration::from_secs\(5\), self.exchange\(command\)\)/u);
  assert.equal([...source.matchAll(/Payload::MutableRequest\(request\)/gu)].length, 2, "one send site and one synthetic test receive pattern");
  assert.doesNotMatch(await read("crates/desktop/src/runtime.rs"), /plugin\(|allow\(unsafe_code\)/u);
});
