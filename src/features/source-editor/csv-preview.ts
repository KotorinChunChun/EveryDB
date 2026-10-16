import { invoke, type CsvPreview } from "../../app/ipc";

interface CsvPreviewControllerOptions {
  getConfig: () => Record<string, unknown>;
  request?: (configJson: string) => Promise<CsvPreview>;
  delayMs?: number;
}

export interface CsvPreviewController {
  mount: (host: HTMLElement) => void;
  schedule: () => void;
  clear: () => void;
}

function appendRows(
  tbody: HTMLTableSectionElement,
  rows: CsvPreview["top_rows"],
  columnCount: number,
) {
  for (const row of rows) {
    const tr = document.createElement("tr");
    const number = document.createElement("th");
    number.scope = "row";
    number.textContent = String(row.row_number);
    tr.appendChild(number);
    for (let i = 0; i < columnCount; i += 1) {
      const td = document.createElement("td");
      td.textContent = row.values[i] ?? "";
      tr.appendChild(td);
    }
    tbody.appendChild(tr);
  }
}

export function renderCsvPreview(host: HTMLElement, preview: CsvPreview) {
  const status = host.querySelector<HTMLElement>("[data-csv-preview-status]")!;
  const body = host.querySelector<HTMLElement>("[data-csv-preview-body]")!;
  const delimiter = preview.delimiter === "\\t" ? "タブ" : preview.delimiter;
  status.dataset.state = "success";
  status.textContent = `${preview.total_rows}行 / ${preview.encoding} / ${preview.line_ending} / 区切り: ${delimiter}`;
  body.innerHTML = "";

  if (preview.columns.length === 0) {
    body.textContent = "表示できる列がありません。読み飛ばし行数や解析条件を確認してください。";
    return;
  }
  const table = document.createElement("table");
  table.className = "csv-preview-table";
  const thead = document.createElement("thead");
  const headRow = document.createElement("tr");
  const rowNumberHead = document.createElement("th");
  rowNumberHead.textContent = "行";
  headRow.appendChild(rowNumberHead);
  for (const column of preview.columns) {
    const th = document.createElement("th");
    th.textContent = column;
    headRow.appendChild(th);
  }
  thead.appendChild(headRow);
  table.appendChild(thead);

  const tbody = document.createElement("tbody");
  appendRows(tbody, preview.top_rows, preview.columns.length);
  const lastTop = preview.top_rows.length > 0
    ? preview.top_rows[preview.top_rows.length - 1].row_number
    : undefined;
  const firstBottom = preview.bottom_rows[0]?.row_number;
  if (lastTop !== undefined && firstBottom !== undefined && firstBottom > lastTop + 1) {
    const gap = document.createElement("tr");
    gap.className = "csv-preview-gap";
    const cell = document.createElement("td");
    cell.colSpan = preview.columns.length + 1;
    cell.textContent = `… ${firstBottom - lastTop - 1}行省略 …`;
    gap.appendChild(cell);
    tbody.appendChild(gap);
  }
  appendRows(tbody, preview.bottom_rows, preview.columns.length);
  table.appendChild(tbody);
  body.appendChild(table);
}

export function createCsvPreviewController(options: CsvPreviewControllerOptions): CsvPreviewController {
  const request = options.request ?? ((configJson: string) => invoke("preview_csv", { configJson }));
  const delayMs = options.delayMs ?? 250;
  let host: HTMLElement | null = null;
  let timer: number | null = null;
  let sequence = 0;

  function setMessage(text: string, state: "idle" | "loading" | "error") {
    if (!host) return;
    const status = host.querySelector<HTMLElement>("[data-csv-preview-status]")!;
    const body = host.querySelector<HTMLElement>("[data-csv-preview-body]")!;
    status.dataset.state = state;
    status.textContent = text;
    body.innerHTML = "";
  }

  function clear() {
    sequence += 1;
    if (timer !== null) window.clearTimeout(timer);
    timer = null;
    host = null;
  }

  function schedule() {
    if (!host) return;
    sequence += 1;
    const current = sequence;
    if (timer !== null) window.clearTimeout(timer);
    const config = options.getConfig();
    if (!String(config.path ?? "").trim()) {
      setMessage("CSVファイルを選択すると先頭・末尾各10行を表示します。", "idle");
      return;
    }
    setMessage("解析条件を反映中…", "loading");
    timer = window.setTimeout(async () => {
      timer = null;
      try {
        const preview = await request(JSON.stringify(config));
        if (current !== sequence || !host) return;
        renderCsvPreview(host, preview);
      } catch (error) {
        if (current !== sequence) return;
        setMessage(`プレビュー失敗: ${String(error)}`, "error");
      }
    }, delayMs);
  }

  function mount(nextHost: HTMLElement) {
    clear();
    host = nextHost;
    host.innerHTML = `
      <div class="csv-preview-heading">処理結果プレビュー（先頭10行・末尾10行）</div>
      <div class="csv-preview-status" data-csv-preview-status data-state="idle"></div>
      <div class="csv-preview-table-wrap" data-csv-preview-body></div>
    `;
    schedule();
  }

  return { mount, schedule, clear };
}
