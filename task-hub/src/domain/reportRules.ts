import type { AttachmentRef } from "./attachments";
export interface ReportFolder {
  id: string;
  path: string;
}
export interface ReportRule {
  id: string;
  folder: ReportFolder;
  pattern: string;
  previous_month: boolean;
  template: AttachmentRef | null;
  require_changed: boolean;
}
export interface PatternSuggestion {
  pattern: string;
  count: number;
  samples: string[];
}
export interface RuleCheck {
  expected: string;
  matches: string[];
  status: string;
  suggestions: PatternSuggestion[];
}
export const REPORT_ISSUE_PREFIX =
  "Report attachment preparation stopped before sending: ";
const months = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];
export function reportFilename(
  pattern: string,
  date: string,
  previous = false,
): string {
  if (!pattern || pattern.length > 600 || /[\x00-\x1f/\\:<>"|]/.test(pattern))
    throw new Error(
      "Use a filename pattern only, without folders or Windows-invalid characters.",
    );
  let day = new Date(`${date}T12:00:00`);
  if (Number.isNaN(day.getTime()))
    throw new Error("Choose a valid report date.");
  if (previous) day = new Date(day.getFullYear(), day.getMonth(), 0);
  const month = months[day.getMonth()],
    short = month.slice(0, 3);
  const values: Record<string, string> = {
    date: `${day.getDate()}-${day.getMonth() + 1}-${day.getFullYear()}`,
    iso_date: `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, "0")}-${String(day.getDate()).padStart(2, "0")}`,
    year: String(day.getFullYear()),
    yy: String(day.getFullYear()).slice(-2),
    month,
    MONTH: month.toUpperCase(),
    month_lower: month.toLowerCase(),
    month_short: short,
    MONTH_SHORT: short.toUpperCase(),
    month_short_lower: short.toLowerCase(),
    mm: String(day.getMonth() + 1).padStart(2, "0"),
    month_number: String(day.getMonth() + 1),
    dd: String(day.getDate()).padStart(2, "0"),
    day: String(day.getDate()),
  };
  const name = pattern.replace(/\{\{\s*([^{}]+?)\s*\}\}/g, (_, key: string) => {
    if (!Object.hasOwn(values, key.trim()))
      throw new Error(`Unknown filename field: ${key}`);
    return values[key.trim()];
  });
  if (
    /[{}]/.test(name) ||
    name.startsWith(".") ||
    name.startsWith("~$") ||
    /[. ]$/.test(name) ||
    /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(name)
  )
    throw new Error("Use a regular report filename with supported fields.");
  return name;
}
export function reportMatches(pattern: string, name: string): boolean {
  return new RegExp(
    `^${pattern
      .split("")
      .map((c) =>
        c === "*"
          ? ".*"
          : c === "?"
            ? "."
            : c.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"),
      )
      .join("")}$`,
    "i",
  ).test(name);
}
export function suggestReportPatterns(names: string[]): PatternSuggestion[] {
  const groups = new Map<string, PatternSuggestion>();
  for (const name of names) {
    let pattern = name;
    const iso =
      /(19\d{2}|20\d{2})([-_.])(0?[1-9]|1[0-2])([-_.])(3[01]|[12]\d|0?[1-9])/;
    const dmy =
      /(3[01]|[12]\d|0?[1-9])([-_.])(0?[1-9]|1[0-2])([-_.])((?:19|20)\d{2})/;
    const a = iso.exec(name),
      b = dmy.exec(name);
    if (a && a[2] === a[4])
      pattern = name.replace(iso, `{{year}}${a[2]}{{mm}}${a[4]}{{dd}}`);
    else if (b && b[2] === b[4])
      pattern = name.replace(
        dmy,
        `${b[1].startsWith("0") ? "{{dd}}" : "{{day}}"}${b[2]}${b[3].startsWith("0") ? "{{mm}}" : "{{month_number}}"}${b[4]}{{year}}`,
      );
    pattern = pattern
      .replace(/(?:19|20)\d{2}/g, "{{year}}")
      .replace(
        /(^|[^a-z])(january|february|march|april|may|june|july|august|september|october|november|december|jan|feb|mar|apr|jun|jul|aug|sep|oct|nov|dec)([^a-z]|$)/gi,
        (_, prefix: string, value: string, suffix: string) => {
          const token = value.length === 3 ? "month_short" : "month";
          return `${prefix}{{${value === value.toUpperCase() ? token.toUpperCase() : value === value.toLowerCase() ? `${token}_lower` : token}}}${suffix}`;
        },
      );
    if (pattern === name || !pattern.includes("{{")) continue;
    const item = groups.get(pattern) ?? { pattern, count: 0, samples: [] };
    item.count++;
    if (item.samples.length < 3) item.samples.push(name);
    groups.set(pattern, item);
  }
  return [...groups.values()]
    .sort((a, b) => b.count - a.count || a.pattern.localeCompare(b.pattern))
    .slice(0, 12);
}
export function validateReportRules(
  rules: ReportRule[],
  fixedCount: number,
): void {
  if (rules.length + fixedCount > 10)
    throw new Error(
      "Use at most 10 fixed attachments and report rules combined.",
    );
  if (new Set(rules.map((r) => r.id)).size !== rules.length)
    throw new Error("Duplicate report rule.");
  for (const rule of rules) {
    if (!rule.folder.id)
      throw new Error("Choose a folder for each report rule.");
    reportFilename(rule.pattern, "2026-11-05", rule.previous_month);
  }
}
