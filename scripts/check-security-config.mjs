import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const config = JSON.parse(readFileSync(resolve(root, "src-tauri/tauri.conf.json"), "utf8"));
const capability = JSON.parse(readFileSync(resolve(root, "src-tauri/capabilities/default.json"), "utf8"));
const errors = [];
const permissions = new Set(capability.permissions.filter((permission) => typeof permission === "string"));
for (const permission of ["core:default", "opener:allow-open-path", "opener:allow-open-url", "dialog:allow-open", "dialog:allow-save", "fs:allow-write-file"]) {
  if (!permissions.has(permission)) errors.push(`capability: missing required permission ${permission}`);
}
for (const forbidden of ["fs:default", "fs:allow-read-file", "fs:allow-read-text-file", "fs:allow-remove", "fs:allow-rename"]) {
  if (permissions.has(forbidden)) errors.push(`capability: forbidden broad permission ${forbidden}`);
}
if (capability.windows?.length !== 1 || capability.windows[0] !== "main") errors.push("capability: must be limited to the main window");
if (capability.permissions.some((permission) => typeof permission === "object" && permission.identifier === "fs:scope")) errors.push("capability: must not define a broad fs:scope");

const csp = config.app?.security?.csp ?? "";
for (const directive of ["default-src 'self'", "connect-src 'self' ipc: http://ipc.localhost", "object-src 'none'", "base-uri 'self'", "frame-ancestors 'none'"]) {
  if (!csp.includes(directive)) errors.push(`CSP: missing ${directive}`);
}
if (/unsafe-eval|\*/.test(csp)) errors.push("CSP: unsafe-eval or wildcard source is forbidden");
if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log("Tauri security configuration check passed.");