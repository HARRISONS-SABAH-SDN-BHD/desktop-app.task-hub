import { describe, expect, it } from "vitest";
import {
  reportFilename,
  reportMatches,
  suggestReportPatterns,
  validateReportRules,
} from "../src/domain/reportRules";
describe("current report filename rules", () => {
  it("suggests date/month patterns with filename evidence", () => {
    const suggestions = suggestReportPatterns([
      "4-11-2026.xlsx",
      "5-11-2026.xlsx",
      "OCT report 2026.pdf",
      "NOV report 2026.pdf",
    ]);
    expect(suggestions).toContainEqual({
      pattern: "{{day}}-{{month_number}}-{{year}}.xlsx",
      count: 2,
      samples: ["4-11-2026.xlsx", "5-11-2026.xlsx"],
    });
    expect(
      suggestions.find(
        (s) => s.pattern === "{{MONTH_SHORT}} report {{year}}.pdf",
      )?.count,
    ).toBe(2);
  });
  it("renders the occurrence date, leading zeros, and previous-year month", () => {
    expect(
      reportFilename("{{day}}-{{month_number}}-{{year}}.xlsx", "2026-11-05"),
    ).toBe("5-11-2026.xlsx");
    expect(reportFilename("{{year}}-{{mm}}-{{dd}}.xlsx", "2026-02-05")).toBe(
      "2026-02-05.xlsx",
    );
    expect(
      reportFilename("{{MONTH_SHORT}} {{year}}.pdf", "2027-01-05", true),
    ).toBe("DEC 2026.pdf");
  });
  it("requires whole filename matches and rejects folder escapes or unknown fields", () => {
    expect(reportMatches("NOV*.xlsx", "nov report.xlsx")).toBe(true);
    expect(reportMatches("NOV*.xlsx", "OLD NOV report.xlsx")).toBe(false);
    for (const value of [
      "../report.xlsx",
      "C:\\report.xlsx",
      "{{unknown}}.xlsx",
      "NUL.txt",
    ])
      expect(() => reportFilename(value, "2026-11-05")).toThrow();
  });
  it("counts current reports and fixed copies together", () => {
    const rule = {
      id: "r",
      folder: { id: "f", path: "Preview" },
      pattern: "{{year}}.xlsx",
      previous_month: false,
      template: null,
      require_changed: true,
    };
    expect(() => validateReportRules([rule], 10)).toThrow("at most 10");
    expect(() => validateReportRules([rule], 9)).not.toThrow();
  });
});
