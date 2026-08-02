export type ClipboardWriter = (text: string) => Promise<void>;

export type ClipboardWriteResult =
  | { ok: true }
  | { ok: false; error: unknown };

function defaultClipboardWriter(): ClipboardWriter {
  if (typeof navigator === "undefined" || !navigator.clipboard?.writeText) {
    throw new Error("この環境ではクリップボードを利用できません");
  }
  return navigator.clipboard.writeText.bind(navigator.clipboard);
}

/** テキストをクリップボードへ書き込む。呼び出し元が画面種別に応じた通知を選べるよう、失敗を結果として返す。 */
export async function writeClipboardText(
  text: string,
  writer: ClipboardWriter = defaultClipboardWriter(),
): Promise<ClipboardWriteResult> {
  try {
    await writer(text);
    return { ok: true };
  } catch (error) {
    return { ok: false, error };
  }
}