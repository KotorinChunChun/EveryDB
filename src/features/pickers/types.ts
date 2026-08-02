import type { SourceEditorDialogController } from "../source-editor/dialog";
import type { ParentSettingsDialogController } from "../source-editor/parent-dialog";

export interface SourcePickerDeps {
  getSourceEditor: () => SourceEditorDialogController;
  getParentSettings: () => ParentSettingsDialogController;
  normalizePath: (value: string | undefined | null) => string;
  loadSources: () => Promise<void>;
}

/** Source editor / field renderer picker callbacks (excluding wireCredentialSelector). */
export interface SourcePickers {
  openBrowserProfilePicker: (target: "history" | "bookmarks", pathInput: HTMLInputElement) => void;
  openSheetPicker: (input: HTMLInputElement) => void;
  openExcelTablePicker: (input: HTMLInputElement) => void;
  openCellPicker: (
    input: HTMLInputElement,
    gridMode: boolean | "top" | "bottom" | "bottom_left" | "bottom_right",
    pickMode: "cell" | "column",
  ) => void;
  openAirtableBasePicker: (input: HTMLInputElement) => void;
  openAirtableTablePicker: (input: HTMLInputElement) => void;
  openAirtableViewPicker: (input: HTMLInputElement) => void;
  openNotionPicker: () => void;
  openTablePicker: (browse: "sqlite" | "accdb" | "excel_table") => void;
  openSharePointPicker: () => void;
}
