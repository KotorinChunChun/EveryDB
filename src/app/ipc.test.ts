import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "./ipc";

const invokeMock = vi.mocked(tauriInvoke);

describe("IPC wrapper", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("forwards an approved command name and its arguments to Tauri", async () => {
    invokeMock.mockResolvedValue(undefined);

    await expect(invoke<void>("open_ref", { target: "https://example.test" })).resolves.toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith("open_ref", { target: "https://example.test" });
  });
});