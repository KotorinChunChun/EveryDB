import { describe, expect, it } from "vitest";

import pickerSource from "../pickers/index.ts?raw";
import sourceDialogSource from "../source-editor/dialog.ts?raw";
import ipcSource from "../../app/ipc.ts?raw";
import browseCommandsSource from "../../../src-tauri/src/browse_commands.rs?raw";

describe("credential browse IPC boundary", () => {
  it("keeps saved credential secrets out of picker and source-dialog IPC", () => {
    expect(pickerSource).not.toContain('invoke("credential_secret_value"');
    expect(sourceDialogSource).not.toContain('invoke("credential_secret_value"');
    expect(pickerSource).not.toContain('invoke("resolve_graph_auth"');
    expect(ipcSource).not.toContain("resolve_graph_auth");
    expect(ipcSource).not.toContain("GraphAuthPayload");
  });

  it("accepts credential IDs instead of raw cloud secrets in Rust browse commands", () => {
    expect(browseCommandsSource).not.toMatch(/browse_notion_databases\(token: String\)/);
    expect(browseCommandsSource).not.toMatch(/browse_airtable_(?:bases|tables)\([^)]*token: String/s);
    expect(browseCommandsSource).not.toContain("auth: browse::GraphAuthArgs");
    expect(browseCommandsSource.match(/credential_id: i64/g)?.length).toBeGreaterThanOrEqual(7);
  });
});