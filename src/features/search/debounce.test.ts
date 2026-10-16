import { afterEach, describe, expect, it, vi } from "vitest";
import { createSearchScheduler } from "./debounce";

describe("search scheduler", () => {
  afterEach(() => vi.useRealTimers());

  it("runs once after the final input within the debounce window", () => {
    vi.useFakeTimers();
    const run = vi.fn();
    const scheduler = createSearchScheduler(run, 150);
    scheduler.schedule();
    vi.advanceTimersByTime(100);
    scheduler.schedule();
    vi.advanceTimersByTime(149);
    expect(run).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(run).toHaveBeenCalledTimes(1);
  });

  it("can discard a pending search", () => {
    vi.useFakeTimers();
    const run = vi.fn();
    const scheduler = createSearchScheduler(run);
    scheduler.schedule();
    scheduler.dispose();
    vi.runAllTimers();
    expect(run).not.toHaveBeenCalled();
  });
});