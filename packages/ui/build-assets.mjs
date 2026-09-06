// Copy only the runtime entry's reachable ESM modules, never tests or Node tools.
import { copyFile, mkdir, readFile, readdir, unlink } from "node:fs/promises";
const output = new URL("./desktop-dist/", import.meta.url);
await mkdir(output, { recursive: true });
// Tauri embeds the entire directory, so stale files must not survive a rebuild.
for (const entry of await readdir(output, { withFileTypes: true })) {
  if (!entry.isFile() && !entry.isSymbolicLink()) throw new Error(`Unexpected generated asset directory: ${entry.name}`);
  await unlink(new URL(encodeURIComponent(entry.name), output));
}
const visited = new Set();
async function copyModule(name) {
  if (visited.has(name)) return;
  if (!/^[a-z][a-z0-9-]*\.js$/u.test(name)) throw new Error(`Non-local runtime module: ${name}`);
  visited.add(name);
  const source = new URL(`./dist/${name}`, import.meta.url);
  const code = await readFile(source, "utf8");
  for (const match of code.matchAll(/from\s+["']([^"']+)["']/gu)) {
    if (!match[1].startsWith("./")) throw new Error(`External runtime import: ${match[1]}`);
    await copyModule(match[1].slice(2));
  }
  await copyFile(source, new URL(name, output));
}
await copyModule("runtime-entry.js");
await copyModule("runtime-smoke.js");
for (const name of ["index.html", "shell.css"]) await copyFile(new URL(`./assets/${name}`, import.meta.url), new URL(name, output));
