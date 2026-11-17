import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const root = process.cwd();
const debug = process.argv.includes("--debug");
const profile = debug ? "debug" : "release";
const host = execFileSync("rustc", ["-vV"], { encoding: "utf8" })
  .split(/\r?\n/)
  .find((line) => line.startsWith("host: "))
  ?.slice("host: ".length)
  .trim();

if (!host) {
  console.error("无法读取 rustc host target");
  process.exit(1);
}

const extension = process.platform === "win32" ? ".exe" : "";
const source = join(root, "src-tauri", "target", profile, `phadmin-cli${extension}`);
if (!existsSync(source) || !debug) {
  execFileSync(
    "cargo",
    [
      "build",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--bin",
      "phadmin-cli",
      ...(debug ? [] : ["--release"]),
    ],
    { cwd: root, stdio: "inherit" },
  );
}
if (!existsSync(source)) {
  console.error(`未找到 ${source}`);
  process.exit(1);
}

const targetDir = join(root, "src-tauri", "binaries");
mkdirSync(targetDir, { recursive: true });
const destination = join(targetDir, `phadmin-cli-${host}${extension}`);
copyFileSync(source, destination);
console.log(`sidecar (${profile}): ${destination}`);
