import { test } from "node:test";
import assert from "node:assert/strict";
import { hasUsageSection } from "./readme-headings.mjs";

test("従来と画像付きの利用手順を認める", () => {
  assert.equal(hasUsageSection("## 主な機能と最低限の使い方\n説明"), true);
  assert.equal(hasUsageSection("## 画像で見る使い方\r\n説明"), true);
});

test("本文・コード内だけの言及や別階層の見出しでは代用できない", () => {
  for (const text of ["", "画像で見る使い方", "### 画像で見る使い方", "## 画像で見る使い方ではない", "```md\n## 画像で見る使い方\n```\n"]) {
    assert.equal(hasUsageSection(text), false);
  }
});
