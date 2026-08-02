import { describe, expect, it } from "vitest";

import { mountSimplePickerDialogs, SIMPLE_PICKER_DIALOGS } from "./simple-picker-dialogs";

describe("mountSimplePickerDialogs", () => {
  it("creates one dialog per definition from the template", () => {
    document.body.innerHTML = `
      <template id="simple-picker-dialog-template">
        <dialog class="dialog simple-picker-dialog">
          <h3></h3>
          <div class="picker-list"></div>
          <div class="dialog-actions">
            <button type="button" class="btn">閉じる</button>
          </div>
        </dialog>
      </template>
    `;

    mountSimplePickerDialogs();

    expect(document.querySelectorAll("dialog.simple-picker-dialog").length).toBe(SIMPLE_PICKER_DIALOGS.length);
    for (const def of SIMPLE_PICKER_DIALOGS) {
      const dialog = document.getElementById(def.dialogId);
      expect(dialog?.querySelector(`#${def.listId}`)).not.toBeNull();
      expect(document.getElementById(def.titleId)?.textContent).toBe(def.title);
    }
  });

  it("does not duplicate dialogs when called twice", () => {
    document.body.innerHTML = `
      <template id="simple-picker-dialog-template">
        <dialog class="dialog simple-picker-dialog">
          <h3></h3>
          <div class="picker-list"></div>
          <div class="dialog-actions"><button type="button" class="btn">閉じる</button></div>
        </dialog>
      </template>
    `;

    mountSimplePickerDialogs();
    mountSimplePickerDialogs();

    expect(document.querySelectorAll("dialog.simple-picker-dialog").length).toBe(SIMPLE_PICKER_DIALOGS.length);
  });
});
