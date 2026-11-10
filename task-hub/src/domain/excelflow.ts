export interface ExcelFlowReport {
  key: string;
  name: string;
  relative_path: string;
  kind: "DISC" | "FTG";
  branch: string;
  version: "Normal" | "Special";
  rows: number;
  size: number;
  stamp: string;
}
export interface ExcelFlowRun {
  folder: { id: string; path: string };
  period: string;
  summary_hash: string;
  reports: ExcelFlowReport[];
}
export function parseExcelFlowSummary(
  text: string,
  files: Map<string, File>,
): { period: string; reports: ExcelFlowReport[] } {
  let data: { status?: unknown; period?: unknown; files?: unknown };
  try {
    data = JSON.parse(text);
  } catch {
    throw new Error("This is not a readable ExcelFlow run summary.");
  }
  if (!data || data.status !== "complete")
    throw new Error(
      "ExcelFlow has not completed this run successfully. Running, failed and cancelled reports cannot be imported.",
    );
  if (
    typeof data.period !== "string" ||
    !/^(19\d{2}|20\d{2}|21\d{2}|2200)-(0[1-9]|1[0-2])$/.test(data.period)
  )
    throw new Error("ExcelFlow summary has an invalid reporting month.");
  if (
    !Array.isArray(data.files) ||
    !data.files.length ||
    data.files.length > 2000
  )
    throw new Error(
      "Choose a completed ExcelFlow run with reports (at most 2000).",
    );
  const used = new Set<string>();
  const reports = data.files.map((entry: unknown): ExcelFlowReport => {
    if (!entry || typeof entry !== "object")
      throw new Error("Invalid ExcelFlow report entry.");
    const e = entry as Record<string, unknown>;
    if (
      typeof e.file !== "string" ||
      !["DISC", "FTG"].includes(String(e.kind)) ||
      typeof e.branch !== "string" ||
      !/^[A-Z]\d{2}$/.test(e.branch) ||
      !Number.isSafeInteger(e.rows) ||
      (e.rows as number) < 0
    )
      throw new Error("Invalid ExcelFlow report type, branch or row count.");
    const relative = e.file.replaceAll("\\", "/");
    if (
      relative.length > 2048 ||
      relative
        .split("/")
        .some(
          (part) =>
            !part ||
            part === "." ||
            part === ".." ||
            /[\x00-\x1f:*?"<>|]/.test(part),
        ) ||
      !relative.toLowerCase().endsWith(".xlsx")
    )
      throw new Error("ExcelFlow summary contains an unsafe report path.");
    if (used.has(relative.toLowerCase()))
      throw new Error("ExcelFlow summary lists a report twice.");
    used.add(relative.toLowerCase());
    const file = files.get(relative);
    if (!file)
      throw new Error(
        "A report listed in the summary is missing. Keep the completed run folder intact.",
      );
    return {
      key: relative,
      name: file.name,
      relative_path: relative,
      kind: e.kind as "DISC" | "FTG",
      branch: e.branch,
      version: relative.split("/")[0].includes("DISC with IPCF, FTG with IPCD")
        ? "Special"
        : "Normal",
      rows: e.rows as number,
      size: file.size,
      stamp: `${file.size}:${file.lastModified}`,
    };
  });
  return { period: data.period, reports };
}
