import { invoke } from "../../app/ipc";

/** API キー・トークンの取得ページを既定のブラウザで開くボタン。 */
export function makeApiKeyButton(url: string): HTMLButtonElement {
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "btn browse-btn";
  btn.textContent = "🔑 取得…";
  btn.title = `${url} を既定のブラウザで開きます`;
  btn.onclick = () => void invoke("open_ref", { target: url }).catch((e) => {
    console.error("取得ページを開けませんでした:", e);
  });
  return btn;
}

/** 任意のページを既定のブラウザで開くリンクボタン。 */
export function makeLinkButton(url: string, text: string): HTMLButtonElement {
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "btn browse-btn";
  btn.textContent = text;
  btn.title = `${url} を既定のブラウザで開きます`;
  btn.onclick = () => void invoke("open_ref", { target: url }).catch((e) => {
    console.error("ページを開けませんでした:", e);
  });
  return btn;
}
