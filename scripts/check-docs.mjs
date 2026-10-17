import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { resolve, dirname, extname, join, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const docsRoot = join(root, "docs");
const devRoot = join(root, "dev");
const files = [join(root, "README.md")];
const categoryLabels = {
  local: "ローカル",
  microsoft: "Microsoft 365",
  cloud: "クラウド",
  database: "データベース",
  "public-data": "公開データ",
  web: "Web",
};

function collectMarkdown(directory) {
  for (const entry of readdirSync(directory)) {
    const path = join(directory, entry);
    // 版別の生成物・生ログは公開文書の検査対象にしない。
    if (entry === "logs" && path.startsWith(`${join(devRoot, "history")}${sep}`)) continue;
    if (statSync(path).isDirectory()) collectMarkdown(path);
    else if (extname(path) === ".md") files.push(path);
  }
}

function sourceCatalog(manifest) {
  const rows = manifest
    .filter((entry) => entry.frontend)
    .map((entry) => `| ${categoryLabels[entry.category]} | ${entry.label}（\`${entry.id}\`） | [${entry.guide}](guides/${entry.guide}.md) |`);
  return [
    "| 区分 | データソース | 接続手順 |",
    "| --- | --- | --- |",
    ...rows,
  ].join("\n");
}

collectMarkdown(docsRoot);
for (const entry of readdirSync(devRoot)) {
  if (entry === "private" || entry === "temp") continue;
  const path = join(devRoot, entry);
  if (statSync(path).isDirectory()) collectMarkdown(path);
  else if (extname(path) === ".md") files.push(path);
}

const errors = [];
for (const file of files) {
  if (file.endsWith("_TEMPLATE.md")) continue;
  const isHistory = file.startsWith(`${join(devRoot, "history")}${sep}`);
  const text = readFileSync(file, "utf8");
  for (const match of text.matchAll(/!?\[[^\]]*\]\(([^)#]+)(?:#[^)]+)?\)/g)) {
    const target = match[1];
    if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith("#")) continue;
    if (isHistory && extname(target.split(":")[0]).toLowerCase() !== ".md") continue;
    if (!existsSync(resolve(dirname(file), target))) errors.push(`${file}: missing link target ${target}`);
  }
  if (!isHistory && /マスターパスワード|docs\/requirements\.md|(?:start-(?:debug|release)|run_dev|gen_access_sample)\.ps1/.test(text)) {
    errors.push(`${file}: deprecated documentation term or link remains`);
  }
}

const readme = readFileSync(join(root, "README.md"), "utf8");
for (const heading of ["対応環境", "インストール", "アンインストール", "主な機能と最低限の使い方", "対応データソース", "プライバシーとセキュリティ", "利用条件", "作者"]) {
  if (!readme.includes(`## ${heading}`)) errors.push(`README.md: missing required heading ${heading}`);
}
if (!readme.includes("docs/DEVELOPERS_GUIDE.md")) {
  errors.push("README.md: missing link to docs/DEVELOPERS_GUIDE.md");
}

const manifest = JSON.parse(readFileSync(join(root, "shared", "source-types.json"), "utf8"));
for (const entry of manifest.filter((item) => item.frontend)) {
  if (typeof entry.label !== "string" || entry.label.trim().length === 0) {
    errors.push(`shared/source-types.json: missing label for ${entry.id}`);
  }
  if (!existsSync(join(docsRoot, "guides", `${entry.guide}.md`))) {
    errors.push(`shared/source-types.json: missing guide for ${entry.id}: ${entry.guide}`);
  }
}

const dataSources = readFileSync(join(docsRoot, "data-sources.md"), "utf8").replace(/\r\n/g, "\n");
const match = dataSources.match(/<!-- source-catalog:start -->\n([\s\S]*?)\n<!-- source-catalog:end -->/);
if (!match) {
  errors.push("docs/data-sources.md: missing source-catalog markers");
} else if (match[1] !== sourceCatalog(manifest)) {
  errors.push("docs/data-sources.md: source catalog is out of sync with shared/source-types.json");
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log(`Documentation check passed for ${files.length} Markdown files.`);
