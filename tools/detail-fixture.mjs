// Emit only from the deterministic builder; --check compares without rewriting.
import { readFile, writeFile, mkdir } from "node:fs/promises";
import { buildDetailFixture } from "../packages/ui/dist/detail-fixture.js";
const directory = new URL("../testdata/detail-surfaces/", import.meta.url);
const path = new URL("corpus.json", directory);
const bytes = `${JSON.stringify(buildDetailFixture(), null, 2)}\n`;
// Explicit seeder input only; never embedded in the desktop runtime assets.
const rate = 8000;
const samples = rate * 12;
const wav = Buffer.alloc(44 + samples * 2);
wav.write("RIFF", 0); wav.writeUInt32LE(wav.length - 8, 4); wav.write("WAVEfmt ", 8);
wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
wav.writeUInt32LE(rate, 24); wav.writeUInt32LE(rate * 2, 28); wav.writeUInt16LE(2, 32); wav.writeUInt16LE(16, 34);
wav.write("data", 36); wav.writeUInt32LE(samples * 2, 40);
// Synthetic tone first, then silent disposition intervals; no real recording.
for (let i = 0; i < rate * 6; i++) wav.writeInt16LE(i % 40 < 20 ? 1200 : -1200, 44 + i * 2);
const audioPath = new URL("synthetic-lecture.wav", directory);
if (process.argv.includes("--check")) {
  if (await readFile(path, "utf8") !== bytes) throw new Error("Detail fixture differs from deterministic builder");
  if (!(await readFile(audioPath)).equals(wav)) throw new Error("Synthetic audio differs from deterministic builder");
} else {
  await mkdir(directory, { recursive: true });
  await writeFile(path, bytes);
  await writeFile(audioPath, wav);
}
console.log("Synthetic detail corpus and audio are byte-equal to their deterministic builder.");
