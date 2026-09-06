// Bounded native prerequisites for the explicitly selected desktop CI lane.
import { spawnSync } from "node:child_process";
if (process.platform === "linux") {
  for (const args of [["apt-get", "update"], ["apt-get", "install", "--no-install-recommends", "-y", "libwebkit2gtk-4.1-dev"]]) {
    const result = spawnSync("sudo", args, { stdio: "inherit" });
    if (result.status !== 0) process.exit(result.status ?? 1);
  }
} else if (!["win32", "darwin"].includes(process.platform)) {
  throw new Error(`Unsupported desktop platform: ${process.platform}`);
}
