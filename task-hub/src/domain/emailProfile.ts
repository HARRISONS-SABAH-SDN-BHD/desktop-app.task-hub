import initial from "../../config/tasks.json";
import { parseConfig } from "./config";
import type { Parameters, TaskConfig } from "./types";

export const EMAIL_TASK_ID = "foreground_email";
export function emailConfig(): TaskConfig {
  return parseConfig(structuredClone(initial));
}
export function validateEmailRequest(taskId: string, values: Parameters) {
  if (taskId !== EMAIL_TASK_ID)
    throw new Error("This prototype supports email automation only.");
  if (!["Draft", "Send after review"].includes(String(values.delivery)))
    throw new Error("Choose Draft or Send after review.");
}
