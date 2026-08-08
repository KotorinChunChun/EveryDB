import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";
import type { SourceInfo } from "../shared/types";
import { invoke } from "./ipc";

const invokeMock = vi.mocked(tauriInvoke);

describe("IPC wrapper", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("forwards an approved command name and its arguments to Tauri", async () => {
    invokeMock.mockResolvedValue(undefined);

    await expect(invoke("open_ref", { target: "https://example.test" })).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("open_ref", { target: "https://example.test" });
  });
  it("infers command results and rejects mismatched arguments", () => {
    const listSources = () => invoke("list_sources");
    expectTypeOf(listSources).returns.toEqualTypeOf<Promise<SourceInfo[]>>();

    if (false) {
      // @ts-expect-error Rust の parent_key はフロントでは parentKey として渡す。
      void invoke("set_parent_label", { parent_key: "key", label: "label" });
      // @ts-expect-error 引数必須 command は引数を省略できない。
      void invoke("delete_source");
    }
  });
});