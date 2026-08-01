import { spawnSync } from "node:child_process";

const checks = [
  process.platform === "win32"
    ? ["cmd.exe", ["/d", "/s", "/c", "npm run check"]]
    : ["npm", ["run", "check"]],
  ["cargo", ["fmt", "--manifest-path", "src-tauri/Cargo.toml", "--all", "--", "--check"]],
  ["cargo", ["clippy", "--manifest-path", "src-tauri/Cargo.toml", "--all-targets", "--all-features", "--", "-D", "warnings"]],
  ["cargo", ["test", "--manifest-path", "src-tauri/Cargo.toml", "--lib"]],
  ["cargo", ["build", "--manifest-path", "src-tauri/Cargo.toml", "--bins"]],
];

for (const [command, args] of checks) {
  console.log(`\n> ${command} ${args.join(" ")}`);
  const result = spawnSync(command, args, { stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

console.log("\nAll offline quality checks passed.");