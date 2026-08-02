import { invoke } from "../../app/ipc";

/** 折り返された行をつなぐ。日本語はそのまま連結し、英数字同士の境目だけ空白を入れる。 */
function joinWrapped(a: string, b: string): string {
  if (!a) return b;
  const needSpace = /[A-Za-z0-9)\]]$/.test(a) && /^[A-Za-z0-9([]/.test(b);
  return a + (needSpace ? " " : "") + b;
}

/** インライン記法（**強調** / `コード` / [表示](URL)）を DOM として組み立てる。 */
function appendInlineMarkdown(parent: HTMLElement, text: string) {
  const re = /\*\*([^*]+)\*\*|`([^`]+)`|\[([^\]]+)\]\(([^)]+)\)/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) parent.appendChild(document.createTextNode(text.slice(last, m.index)));
    if (m[1] !== undefined) {
      const b = document.createElement("strong");
      b.textContent = m[1];
      parent.appendChild(b);
    } else if (m[2] !== undefined) {
      const c = document.createElement("code");
      c.textContent = m[2];
      parent.appendChild(c);
    } else {
      const url = m[4];
      const a = document.createElement("a");
      a.textContent = m[3];
      a.href = "#";
      a.title = url;
      a.onclick = (ev) => {
        ev.preventDefault();
        void invoke("open_ref", { target: url }).catch(() => {});
      };
      parent.appendChild(a);
    }
    last = re.lastIndex;
  }
  if (last < text.length) parent.appendChild(document.createTextNode(text.slice(last)));
}

/** 手順書用の最小限の Markdown レンダラ（見出し / 箇条書き / 番号付き / 表 / コード / 段落）。 */
export function renderMarkdownInto(container: HTMLElement, md: string) {
  container.innerHTML = "";
  const lines = md.replace(/<!--[\s\S]*?-->/g, "").split(/\r?\n/);
  let i = 0;
  const isTableRow = (s: string) => s.trim().startsWith("|");
  const cellsOf = (s: string) => s.trim().replace(/^\|/, "").replace(/\|$/, "").split("|").map((c) => c.trim());

  while (i < lines.length) {
    const line = lines[i];
    if (line.trim() === "") { i++; continue; }

    if (line.trim().startsWith("```")) {
      i++;
      const buf: string[] = [];
      while (i < lines.length && !lines[i].trim().startsWith("```")) buf.push(lines[i++]);
      i++;
      const pre = document.createElement("pre");
      const code = document.createElement("code");
      code.textContent = buf.join("\n");
      pre.appendChild(code);
      container.appendChild(pre);
      continue;
    }

    const h = /^(#{1,4})\s+(.*)$/.exec(line);
    if (h) {
      const el = document.createElement(`h${h[1].length}`);
      appendInlineMarkdown(el, h[2]);
      container.appendChild(el);
      i++;
      continue;
    }

    if (isTableRow(line) && i + 1 < lines.length && /^\s*\|[\s:|-]+\|\s*$/.test(lines[i + 1])) {
      const table = document.createElement("table");
      const thead = document.createElement("thead");
      const htr = document.createElement("tr");
      for (const c of cellsOf(line)) {
        const th = document.createElement("th");
        appendInlineMarkdown(th, c);
        htr.appendChild(th);
      }
      thead.appendChild(htr);
      table.appendChild(thead);
      i += 2;
      const tbody = document.createElement("tbody");
      while (i < lines.length && isTableRow(lines[i])) {
        const tr = document.createElement("tr");
        for (const c of cellsOf(lines[i])) {
          const td = document.createElement("td");
          appendInlineMarkdown(td, c);
          tr.appendChild(td);
        }
        tbody.appendChild(tr);
        i++;
      }
      table.appendChild(tbody);
      container.appendChild(table);
      continue;
    }

    const bullet = /^(\s*)([-*]|\d+\.)\s+(.*)$/.exec(line);
    if (bullet) {
      const ordered = /\d/.test(bullet[2]);
      const list = document.createElement(ordered ? "ol" : "ul");
      while (i < lines.length) {
        const m2 = /^(\s*)([-*]|\d+\.)\s+(.*)$/.exec(lines[i]);
        if (!m2 || /\d/.test(m2[2]) !== ordered) break;
        let text = m2[3];
        i++;
        while (i < lines.length && /^\s+\S/.test(lines[i]) && !/^(\s*)([-*]|\d+\.)\s+/.test(lines[i]) && !lines[i].trim().startsWith("```")) {
          text = joinWrapped(text, lines[i].trim());
          i++;
        }
        const li = document.createElement("li");
        appendInlineMarkdown(li, text);
        list.appendChild(li);
      }
      container.appendChild(list);
      continue;
    }

    let text = "";
    while (i < lines.length && lines[i].trim() !== "" && !/^(#{1,4})\s/.test(lines[i])
      && !isTableRow(lines[i]) && !/^(\s*)([-*]|\d+\.)\s+/.test(lines[i]) && !lines[i].trim().startsWith("```")) {
      text = joinWrapped(text, lines[i].trim());
      i++;
    }
    const p = document.createElement("p");
    appendInlineMarkdown(p, text);
    container.appendChild(p);
  }
}
