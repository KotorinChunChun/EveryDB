import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync } from "node:fs";
import path from "node:path";

const { version } = JSON.parse(readFileSync("package.json", "utf8"));
if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("Invalid package version");
const logs = path.resolve(`dev/history/v${version}/logs`);
mkdirSync(logs, { recursive: true });
const sampleDir = path.join(mkdtempSync(path.join(logs, "offline-quality-")), "samples");
const testEnvironment = { ...process.env, EVERYDB_TEST_SAMPLES_DIR: sampleDir };

const checks = [
  ["pwsh", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "dev/scripts/test-development-environment.ps1"]],
  ["pwsh", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "dev/scripts/test-script-naming.ps1"]],
  process.platform === "win32"
    ? ["cmd.exe", ["/d", "/s", "/c", "npm run check"]]
    : ["npm", ["run", "check"]],
  ["cargo", ["fmt", "--manifest-path", "src-tauri/Cargo.toml", "--all", "--", "--check"]],
  ["cargo", ["clippy", "--manifest-path", "src-tauri/Cargo.toml", "--all-targets", "--all-features", "--", "-D", "warnings"]],
  ["cargo", ["run", "--manifest-path", "src-tauri/Cargo.toml", "--example", "gen_samples", "--", "--output-dir", sampleDir]],
  ["cargo", ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib"]],
  ["cargo", ["build", "--manifest-path", "src-tauri/Cargo.toml", "--bins"]],
];

for (const [command, args] of checks) {
  console.log(`\n> ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, { stdio: "inherit", env: testEnvironment });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

console.log("\nAll offline quality checks passed.");
