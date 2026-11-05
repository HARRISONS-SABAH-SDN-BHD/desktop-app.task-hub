import validate from "./task-config.validator.js";
import type { Parameters, Step, Task, TaskConfig } from "./types";

export function parseConfig(value: unknown): TaskConfig {
  if (!validate(value))
    throw new Error(
      validate.errors
        ?.map((error) => `${error.instancePath || "/"} ${error.message}`)
        .join("\n") ?? "Invalid task configuration.",
    );
  const config = value;
  if (new TextEncoder().encode(JSON.stringify(config)).length > 1024 * 1024)
    throw new Error("Configuration exceeds 1 MB.");
  const ids = new Set<string>();
  for (const task of config.tasks) {
    if (ids.has(task.id)) throw new Error(`Duplicate task id: ${task.id}`);
    ids.add(task.id);
    const steps = new Set<string>();
    for (const step of task.steps) {
      if (steps.has(step.id))
        throw new Error(`Duplicate step id in ${task.id}: ${step.id}`);
      steps.add(step.id);
      if (step.default !== undefined) {
        const error = validateStep({ ...step, required: false }, step.default);
        if (error) throw new Error(`${task.id}.${step.id} default: ${error}`);
      }
    }
    if (task.action.type !== "script") {
      const fields = task.action.fields;
      const required =
        task.action.type === "foreground"
          ? task.action.recipe === "web_email"
            ? ["title", "body", "recipient", "provider", "delivery"]
            : ["title", "body"]
          : task.action.type === "smtp" || task.action.format === "eml"
            ? ["title", "body", "recipient"]
            : task.action.format === "ics"
              ? ["title", "start"]
              : ["title", "body"];
      for (const key of required)
        if (!(key in fields))
          throw new Error(`${task.id}: missing action field ${key}`);
      for (const value of Object.values(fields)) {
        for (const match of value.matchAll(/\{([a-z][a-z0-9_]*)\}/g)) {
          if (!steps.has(match[1]))
            throw new Error(`${task.id}: unknown parameter {${match[1]}}`);
        }
      }
    }
  }
  return config;
}
export function defaults(task: Task): Parameters {
  return Object.fromEntries(
    task.steps.map((s) => [
      s.id,
      Array.isArray(s.default)
        ? [...s.default]
        : (s.default ?? (s.type === "multi_select" ? [] : "")),
    ]),
  );
}
export function validateStep(
  step: Step,
  value: string | string[] | undefined,
): string | undefined {
  const multi = step.type === "multi_select";
  if (
    value !== undefined &&
    (multi ? !Array.isArray(value) : typeof value !== "string")
  )
    return "Unexpected value type.";
  const values = Array.isArray(value) ? value : [value ?? ""];
  if (values.length > 100 || values.some((v) => v.length > 20000))
    return "This answer is too long.";
  if (
    values.some((v) => /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(v))
  )
    return "This answer contains unsupported control characters.";
  const filled = values.filter((v) => v.trim());
  if (step.required !== false && !filled.length)
    return "Please complete this step.";
  if (
    Array.isArray(value) &&
    (new Set(value).size !== value.length || value.some((v) => !v.trim()))
  )
    return "Choose distinct, non-empty values.";
  if (
    (step.type === "select" || multi) &&
    !step.allow_other &&
    filled.some((v) => !step.options?.includes(v))
  )
    return "Choose an available option.";
  if (
    step.type === "email" &&
    filled.length &&
    !/^[^\s@<>\r\n]+@[^\s@<>\r\n]+\.[^\s@<>\r\n]+$/.test(filled[0])
  )
    return "Enter one valid email address.";
  if (
    step.type === "datetime" &&
    filled.length &&
    !Number.isFinite(Date.parse(filled[0]))
  )
    return "Choose a valid date and time.";
}
export function validateParameters(task: Task, parameters: Parameters): void {
  if (new TextEncoder().encode(JSON.stringify(parameters)).length > 65536)
    throw new Error("Combined answers exceed 64 KB.");
  const known = new Set(task.steps.map((s) => s.id));
  if (Object.keys(parameters).some((id) => !known.has(id)))
    throw new Error("Unknown parameter.");
  for (const step of task.steps) {
    const error = validateStep(step, parameters[step.id]);
    if (error) throw new Error(`${step.prompt}: ${error}`);
  }
  if (task.action.type === "foreground") {
    const fields = resolveFields(task, parameters);
    if (
      !fields.title.trim() ||
      fields.title.length > 160 ||
      /[\n\r\t]/.test(fields.title)
    )
      throw new Error("Use a single-line title of at most 160 characters.");
    if (!fields.body.trim() || fields.body.length > 12000)
      throw new Error("Foreground content must contain 1–12,000 characters.");
    if (task.action.recipe === "web_email") {
      if (
        fields.browser &&
        !["Microsoft Edge", "Google Chrome", "Mozilla Firefox"].includes(
          fields.browser,
        )
      )
        throw new Error("Choose a supported browser.");
      if (
        !["Gmail", "Outlook personal", "Outlook work"].includes(fields.provider)
      )
        throw new Error("Choose a supported mailbox.");
      if (
        !["Draft", "Send after review", "Send directly"].includes(
          fields.delivery,
        )
      )
        throw new Error("Choose Draft, Send after review, or Send directly.");
      if (
        !/^[^\s@<>\r\n]+@[^\s@<>\r\n]+\.[^\s@<>\r\n]+$/.test(fields.recipient)
      )
        throw new Error("Enter one valid recipient.");
    }
  }
  if (task.action.type === "export" && task.action.format === "ics") {
    const fields = resolveFields(task, parameters);
    const start = Date.parse(fields.start);
    if (!Number.isFinite(start))
      throw new Error("Calendar start time is invalid.");
    if (
      fields.end &&
      (!Number.isFinite(Date.parse(fields.end)) ||
        Date.parse(fields.end) <= start)
    )
      throw new Error("The end time must be after the start time.");
    if (fields.minutes && !/^\d{1,5}$/.test(fields.minutes))
      throw new Error("Reminder minutes must be a positive whole number.");
  }
}
export function resolveTemplate(
  template: string,
  parameters: Parameters,
): string {
  return template.replace(/\{([a-z][a-z0-9_]*)\}/g, (_, key: string) => {
    if (!(key in parameters)) throw new Error(`Missing parameter: ${key}`);
    const value = parameters[key];
    return Array.isArray(value) ? value.join("\n") : value;
  });
}
export function resolveFields(
  task: Task,
  parameters: Parameters,
): Record<string, string> {
  if (task.action.type === "script") return {};
  return Object.fromEntries(
    Object.entries(task.action.fields).map(([k, v]) => [
      k,
      resolveTemplate(v, parameters),
    ]),
  );
}
export function executionParameters(
  task: Task,
  parameters: Parameters,
): Parameters {
  return Object.fromEntries(
    task.steps.map((step) => {
      const value =
        parameters[step.id] ?? (step.type === "multi_select" ? [] : "");
      return [
        step.id,
        step.type === "datetime" && typeof value === "string" && value
          ? new Date(value).toISOString()
          : value,
      ];
    }),
  );
}
