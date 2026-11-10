import { describe, it, expect } from "vitest";
import { parseExcelFlowSummary } from "../src/domain/excelflow";
const path = "Formatted October 2026/DISC/01 KK DISC 1026.xlsx";
const file = {
  name: "01 KK DISC 1026.xlsx",
  size: 30,
  lastModified: 100,
} as File;
const data = {
  status: "complete",
  period: "2026-10",
  files: [{ file: path, kind: "DISC", branch: "A01", rows: 10 }],
};
describe("ExcelFlow summary adapter", () => {
  it("reads successful legacy ExcelFlow summaries and metadata", () => {
    const run = parseExcelFlowSummary(
      JSON.stringify(data),
      new Map([[path, file]]),
    );
    expect(run.period).toBe("2026-10");
    expect(run.reports[0]).toMatchObject({
      kind: "DISC",
      branch: "A01",
      version: "Normal",
      rows: 10,
    });
  });
  it("rejects incomplete runs, missing files and unsafe paths", () => {
    for (const status of ["running", "failed", "cancelled"])
      expect(() =>
        parseExcelFlowSummary(
          JSON.stringify({ ...data, status }),
          new Map([[path, file]]),
        ),
      ).toThrow();
    expect(() =>
      parseExcelFlowSummary(JSON.stringify(data), new Map()),
    ).toThrow(/missing/);
    for (const file of ["../x.xlsx", "C:\\private.xlsx", "/x.xlsx"])
      expect(() =>
        parseExcelFlowSummary(
          JSON.stringify({ ...data, files: [{ ...data.files[0], file }] }),
          new Map(),
        ),
      ).toThrow(/unsafe/);
  });
  it("identifies Special reports without mixing them with Normal", () => {
    const special =
      "Formatted October 2026 (DISC with IPCF, FTG with IPCD)/DISC/01 KK DISC 1026.xlsx";
    const run = parseExcelFlowSummary(
      JSON.stringify({ ...data, files: [{ ...data.files[0], file: special }] }),
      new Map([[special, file]]),
    );
    expect(run.reports[0].version).toBe("Special");
  });
});
