import { describe, it, expect } from "vitest";
import initial from "../config/examples/library.tasks.json";
import {
  defaults,
  executionParameters,
  parseConfig,
  resolveTemplate,
  validateParameters,
  validateStep,
} from "../src/domain/config";
const config = parseConfig(initial);
describe("workflow configuration", () => {
  it("loads all starter workflows", () => {
    expect(config.tasks).toHaveLength(9);
  });
  it("rejects duplicate task and step ids", () => {
    const duplicate = structuredClone(initial);
    duplicate.tasks[1].id = duplicate.tasks[0].id;
    expect(() => parseConfig(duplicate)).toThrow("Duplicate task");
    const steps = structuredClone(initial);
    steps.tasks[0].steps[1].id = steps.tasks[0].steps[0].id;
    expect(() => parseConfig(steps)).toThrow("Duplicate step");
  });
  it("rejects executable command strings and unknown scripts", () => {
    expect(() =>
      parseConfig({
        ...initial,
        tasks: [{ ...initial.tasks[0], command: "sh script {subject}" }],
      }),
    ).toThrow();
    expect(() =>
      parseConfig({
        ...initial,
        tasks: [
          {
            ...initial.tasks[0],
            action: { type: "script", script_id: "../../evil" },
          },
        ],
      }),
    ).toThrow();
  });
  it("rejects unknown template references and missing export fields", () => {
    const bad = structuredClone(initial);
    bad.tasks[0].action.fields!.title = "{missing}";
    expect(() => parseConfig(bad)).toThrow("unknown parameter");
    expect(() =>
      parseConfig({
        ...initial,
        tasks: [
          {
            ...initial.tasks[0],
            action: {
              type: "export",
              format: "eml",
              fields: { title: "{subject}" },
            },
          },
        ],
      }),
    ).toThrow("missing action field");
  });
  it("validates default selections", () => {
    const task = config.tasks.find((t) => t.id === "word_report")!;
    expect(() =>
      parseConfig({
        schema_version: 1,
        tasks: [
          {
            ...task,
            steps: task.steps.map((s) =>
              s.id === "section"
                ? { ...s, allow_other: false, default: "Invalid" }
                : s,
            ),
          },
        ],
      }),
    ).toThrow();
  });
});
describe("answers and templates", () => {
  it("preserves multi-select defaults without sharing mutable state", () => {
    const task = config.tasks.find((t) => t.id === "system_snapshot")!;
    const values = defaults(task);
    expect(values.sections).toEqual(["Operating system", "Runtime"]);
    validateParameters(task, values);
  });
  it("requires answers and rejects header injection and invalid selections", () => {
    expect(validateStep(config.tasks[0].steps[0], "")).toBeDefined();
    expect(
      validateStep(config.tasks[0].steps[0], "a@b.com\r\nBcc: victim@b.com"),
    ).toBeDefined();
    const step = config.tasks.find((t) => t.id === "reminder")!.steps[2];
    expect(validateStep(step, "999")).toBeDefined();
    expect(validateStep(step, "15")).toBeUndefined();
  });
  it("allows custom options only on configured steps", () => {
    const step = config.tasks.find((t) => t.id === "word_report")!.steps[1];
    expect(validateStep(step, "Quarterly custom review")).toBeUndefined();
    expect(
      validateStep({ ...step, allow_other: false }, "Quarterly custom review"),
    ).toBeDefined();
  });
  it("never recursively interpolates or evaluates answer text", () => {
    const payload = "$(whoami) '; rm -rf /; {secret}";
    expect(resolveTemplate("Answer: {message}", { message: payload })).toBe(
      `Answer: ${payload}`,
    );
    expect(resolveTemplate("{actions}", { actions: ["A", "B"] })).toBe("A\nB");
    expect(() => resolveTemplate("{unknown}", {})).toThrow("Missing parameter");
  });
  it("rejects invalid date ranges and converts local timestamps for native IPC", () => {
    const task = config.tasks.find((t) => t.id === "calendar_event")!;
    const values = {
      title: "Meeting",
      start: "2026-11-01T10:00",
      end: "2026-11-01T09:00",
      location: "",
      notes: "",
    };
    expect(() => validateParameters(task, values)).toThrow("end time");
    expect(executionParameters(task, values).start).toBe(
      new Date(values.start).toISOString(),
    );
  });
  it("rejects extra keys and incorrect answer types", () => {
    const task = config.tasks[0];
    expect(() =>
      validateParameters(task, {
        recipient: "a@b.com",
        subject: "Hi",
        message: "Hello",
        injected: "yes",
      }),
    ).toThrow("Unknown parameter");
    expect(validateStep(task.steps[1], ["wrong type"])).toBeDefined();
  });
});
