import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { resolve, dirname, extname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const docsRoot = join(root, "docs");
const files = [join(root, "README.md")];

function collectMarkdown(directory) {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    if (statSync(path).isDirectory()) collectMarkdown(path);
    else if (extname(path) === ".md") files.push(path);
  }
}
collectMarkdown(docsRoot);

const errors = [];
for (const file of files) {
  if (file.endsWith("_TEMPLATE.md")) continue;
  const text = readFileSync(file, "utf8");
  for (const match of text.matchAll(/!?\[[^\]]*\]\(([^)#]+)(?:#[^)]+)?\)/g)) {
    const target = match[1];
    if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith("#")) continue;
    if (!existsSync(resolve(dirname(file), target))) errors.push(`${file}: missing link target ${target}`);
  }
  if (/マスターパスワード|docs\/requirements\.md|scripts\/(gen_access_sample|run_dev)\.ps1/.test(text)) {
    errors.push(`${file}: deprecated documentation term or link remains`);
  }
}

const readme = readFileSync(join(root, "README.md"), "utf8");
for (const heading of ["対応環境", "インストール", "アンインストール", "主な機能と最低限の使い方", "対応データソース", "プライバシーとセキュリティ", "利用条件", "作者"]) {
  if (!readme.includes(`## ${heading}`)) errors.push(`README.md: missing required heading ${heading}`);
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log(`Documentation check passed for ${files.length} Markdown files.`);