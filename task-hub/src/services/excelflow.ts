import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
import { attachmentService } from "./attachments";
import { parseExcelFlowSummary, type ExcelFlowRun } from "../domain/excelflow";
import { validateAttachments, type AttachmentRef } from "../domain/attachments";
const previews = new Map<string, Map<string, File>>();
async function digest(text: string) {
  return [
    ...new Uint8Array(
      await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text)),
    ),
  ]
    .map((v) => v.toString(16).padStart(2, "0"))
    .join("");
}
export const excelFlow = {
  async inspect(folderId: string): Promise<ExcelFlowRun> {
    return invoke("inspect_excelflow_run", { folderId });
  },
  async preview(files: File[]): Promise<ExcelFlowRun> {
    const map = new Map<string, File>();
    for (const file of files) {
      const parts = file.webkitRelativePath.split("/");
      const relative = parts.slice(1).join("/");
      if (map.has(relative))
        throw new Error("Duplicate files in the selected folder.");
      map.set(relative, file);
    }
    const summary = map.get("run_summary.json");
    if (!summary)
      throw new Error("Choose the run folder containing run_summary.json.");
    if (summary.size > 2 * 1024 * 1024)
      throw new Error("ExcelFlow summary is too large.");
    const text = await summary.text();
    const result = parseExcelFlowSummary(text, map);
    const id = crypto.randomUUID();
    previews.set(id, map);
    return {
      folder: {
        id,
        path: files[0]?.webkitRelativePath.split("/")[0] ?? "Preview folder",
      },
      summary_hash: await digest(text),
      ...result,
    };
  },
  async import(
    run: ExcelFlowRun,
    keys: string[],
    existing: AttachmentRef[],
  ): Promise<AttachmentRef[]> {
    if (bridge.native)
      return invoke("import_excelflow_reports", {
        folderId: run.folder.id,
        summaryHash: run.summary_hash,
        selected: run.reports
          .filter((r) => keys.includes(r.key))
          .map((r) => ({ key: r.key, stamp: r.stamp })),
        existing,
      });
    const files = previews.get(run.folder.id);
    if (!files) throw new Error("Choose the ExcelFlow run again.");
    const selected = run.reports.filter((r) => keys.includes(r.key));
    const names = new Set(existing.map((f) => f.name.toLowerCase()));
    if (!selected.length) throw new Error("Choose at least one report.");
    for (const report of selected) {
      if (names.has(report.name.toLowerCase()))
        throw new Error(
          "Normal and Special reports may share filenames. Choose one version per email, or use separate routines.",
        );
      names.add(report.name.toLowerCase());
    }
    const items = await attachmentService.preview(
      selected.map((r) => files.get(r.relative_path)!),
    );
    validateAttachments([...existing, ...items]);
    return items;
  },
};
