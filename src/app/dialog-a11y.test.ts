import { describe, expect, it, vi } from "vitest";

import { focusableElements, focusInitial, trapTab } from "./dialog-a11y";

describe("dialog accessibility helpers", () => {
  it("wraps tab focus within a dialog", () => {
    document.body.innerHTML = `
      <dialog open>
        <button id="first">A</button>
        <button id="last">B</button>
      </dialog>
    `;
    const dialog = document.querySelector("dialog")!;
    const last = document.getElementById("last") as HTMLButtonElement;
    last.focus();

    const ev = new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true });
    const prevent = vi.spyOn(ev, "preventDefault");
    trapTab(dialog, ev);

    expect(prevent).toHaveBeenCalled();
    expect(document.activeElement?.id).toBe("first");
  });

  it("focuses the first field when a dialog opens", () => {
    document.body.innerHTML = `
      <dialog>
        <h3>Title</h3>
        <button id="close">Close</button>
        <input id="field" />
      </dialog>
    `;
    const dialog = document.querySelector("dialog")!;
    focusInitial(dialog);
    expect(document.activeElement?.id).toBe("field");
  });

  it("skips disabled controls when collecting focusable elements", () => {
    document.body.innerHTML = `
      <dialog>
        <button disabled id="off">Off</button>
        <button id="on">On</button>
      </dialog>
    `;
    const dialog = document.querySelector("dialog")!;
    expect(focusableElements(dialog).map((el) => el.id)).toEqual(["on"]);
  });
});
