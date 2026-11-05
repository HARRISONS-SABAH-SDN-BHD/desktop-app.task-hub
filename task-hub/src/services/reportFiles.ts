import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
import {
  reportFilename,
  reportMatches,
  suggestReportPatterns,
  type ReportFolder,
  type ReportRule,
  type RuleCheck,
  type PatternSuggestion,
} from "../domain/reportRules";
const previewFolders = new Map<string, File[]>();
export const reportFiles = {
  async open(folder: ReportFolder): Promise<void> {
    if (bridge.native)
      await invoke("open_report_folder", { folderId: folder.id });
  },
  async pick(): Promise<ReportFolder | null> {
    return bridge.native ? invoke("pick_report_folder") : null;
  },
  previewFolder(files: File[]): ReportFolder {
    const top = files.filter((f) => {
      const path = f.webkitRelativePath.split("/");
      return (
        path.length <= 2 &&
        !f.name.startsWith(".") &&
        !f.name.startsWith("~$") &&
        !f.name.endsWith(".tmp")
      );
    });
    if (files.length > 2000)
      throw new Error("Choose a folder with fewer than 2000 entries.");
    const folder = {
      id: crypto.randomUUID(),
      path: `Preview: ${files[0]?.webkitRelativePath.split("/")[0] ?? "Report folder"}`,
    };
    previewFolders.set(folder.id, top);
    return folder;
  },
  async suggest(folder: ReportFolder): Promise<PatternSuggestion[]> {
    if (bridge.native)
      return invoke("suggest_report_patterns", { folderId: folder.id });
    return suggestReportPatterns(
      (previewFolders.get(folder.id) ?? []).map((f) => f.name),
    );
  },
  async inspect(rule: ReportRule, date: string): Promise<RuleCheck> {
    if (bridge.native) return invoke("inspect_report_rule", { rule, date });
    const files = previewFolders.get(rule.folder.id);
    if (!files)
      throw new Error(
        "Reselect the report folder in browser preview. Real folder access is available in the desktop app.",
      );
    const expected = reportFilename(rule.pattern, date, rule.previous_month);
    const matches = files
      .filter((f) => reportMatches(expected, f.name))
      .map((f) => f.name);
    return {
      expected,
      matches,
      suggestions: suggestReportPatterns(files.map((f) => f.name)),
      status:
        matches.length === 1
          ? "One matching file (preview only)"
          : matches.length > 1
            ? "Ambiguous — more than one file matches; email will not be sent"
            : rule.template
              ? "Missing — desktop execution would create a template copy and stop for completion"
              : "Missing — email will not be sent",
    };
  },
  async previewAttachments(rules: ReportRule[], date: string) {
    const result = [];
    for (const rule of rules) {
      const check = await this.inspect(rule, date);
      if (check.matches.length !== 1) throw new Error(check.status);
      const file = previewFolders
        .get(rule.folder.id)!
        .find((f) => f.name === check.matches[0])!;
      result.push({
        id: crypto.randomUUID(),
        name: file.name,
        size: file.size,
        sha256: "0".repeat(64),
      });
    }
    return result;
  },
};
