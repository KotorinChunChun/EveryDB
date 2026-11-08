import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { buildGridColumns, gridCellText } from "../src/features/grid/data.ts";
import { buildExportText, encodeExportText } from "../src/features/export/serialization.ts";

const usage = `EveryDB CSV検証CLI
  preview <CSV>
  import <隔離フォルダーの絶対パス> <CSV> <ソース名>
  search <隔離フォルダーの絶対パス> <検索語> <ソースID> [ソースID...]
  export <検索結果JSON> <出力TSV>
`;
const [command, ...args] = process.argv.slice(2);

try {
  if (command === undefined || command === "--help") {
    process.stdout.write(usage);
  } else if (command === "export" && args.length === 2) {
    const [input, output] = args;
    const { rows, truncated } = JSON.parse(readFileSync(input, "utf8").replace(/^\uFEFF/, ""));
    if (!Array.isArray(rows) || rows.length === 0 || truncated !== false) {
      throw new Error("空または件数上限に達した検索結果は出力できません");
    }
    const columns = buildGridColumns(rows, []);
    const tsv = buildExportText("tsv", columns, rows, (row, col, index) =>
      gridCellText(row, columns[col], index),
    );
    const bytes = await encodeExportText(tsv, "utf8bom");
    writeFileSync(output, bytes, { flag: "wx" });
    console.log(JSON.stringify({ rows: rows.length, columns: columns.length }));
  } else if (["preview", "import", "search"].includes(command)) {
    const result = spawnSync("cargo", [
      "run", "--quiet", "--locked", "--manifest-path", fileURLToPath(new URL("../src-tauri/Cargo.toml", import.meta.url)),
      "--example", "csv_workflow", "--", command, ...args,
    ], { stdio: "inherit" });
    if (result.error) throw result.error;
    process.exitCode = result.status ?? 1;
  } else {
    throw new Error(usage);
  }
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
