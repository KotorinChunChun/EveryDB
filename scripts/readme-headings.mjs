// 従来の操作説明と画像付きの操作説明を、どちらも利用者向け手順として扱う。
export function hasUsageSection(readme) {
  const text = readme.replace(/^```[^\n]*\n[\s\S]*?^```[^\n]*$/gm, "");
  return /^## (?:主な機能と最低限の使い方|画像で見る使い方)[ \t]*\r?$/m.test(text);
}
