import { invoke, isTauri } from "@tauri-apps/api/core";
import { emailConfig, validateEmailRequest } from "../domain/emailProfile";
import { parseConfig, validateParameters } from "../domain/config";
import type { Bridge, Run, Snapshot, TaskConfig } from "../domain/types";

const key = "taskhub.preview.v1";
interface PreviewStore {
  config: TaskConfig;
  snapshot: Snapshot;
  foreground_migrated?: boolean;
}
function read(): PreviewStore {
  const value = localStorage.getItem(key);
  if (value) {
    try {
      const stored = JSON.parse(value) as PreviewStore;
      parseConfig(stored.config);
      // Keep history, but never restore the old multi-task catalog or schedules.
      stored.config = emailConfig();
      stored.snapshot.schedules.forEach((schedule) => {
        schedule.enabled = false;
      });
      return stored;
    } catch {
      /* Recover a corrupted preview store. Native storage reports errors. */
    }
  }
  return {
    config: emailConfig(),
    foreground_migrated: true,
    snapshot: {
      runs: [],
      schedules: [],
      output_dir: "Available in the desktop app",
    },
  };
}
function save(value: PreviewStore) {
  localStorage.setItem(key, JSON.stringify(value));
}
export const bridge: Bridge = isTauri()
  ? {
      native: true,
      loadConfig: async () => parseConfig(await invoke("load_config")),
      saveConfig: (config) =>
        invoke("save_config", { config: parseConfig(config) }),
      snapshot: () => invoke("get_snapshot"),
      run: (taskId, parameters, options) => {
        validateEmailRequest(taskId, parameters);
        return invoke("run_task", {
          taskId,
          parameters,
          allowDirectSend: options?.allowDirectSend ?? false,
          expectedSender: options?.senderAccount ?? null,
          attachments: options?.attachments ?? [],
        });
      },
      schedule: (taskId, parameters, nextRun, repeat) =>
        invoke("create_schedule", { taskId, parameters, nextRun, repeat }),
      deleteSchedule: (id) => invoke("delete_schedule", { id }),
      openArtifact: (runId) => invoke("open_artifact", { runId }),
      stopForeground: (runId) => invoke("stop_foreground", { runId }),
      approveForegroundSend: (runId) =>
        invoke("approve_foreground_send", { runId }),
    }
  : {
      native: false,
      loadConfig: async () => read().config,
      saveConfig: async (config) => {
        const store = read();
        store.config = parseConfig(config);
        save(store);
      },
      snapshot: async () => read().snapshot,
      run: async (taskId, parameters, options) => {
        validateEmailRequest(taskId, parameters);
        const store = read();
        const task = store.config.tasks.find(
          (t) => t.id === taskId && t.enabled !== false,
        );
        if (!task) throw new Error("Workflow unavailable.");
        validateParameters(task, parameters);
        const run: Run = {
          id: crypto.randomUUID(),
          task_id: taskId,
          title: task.title,
          status: "succeeded",
          parameters,
          started_at: new Date().toISOString(),
          finished_at: new Date().toISOString(),
          logs: [
            "PREVIEW ONLY — no files created, scripts executed, or messages sent.",
            "Configuration and answers validated.",
            "In the desktop app, this plan is executed by the Rust runner.",
          ],
        };
        if (options?.attachments?.length)
          run.logs.push(
            `Preview attachments: ${options.attachments.map((file) => file.name).join(", ")}`,
          );
        store.snapshot.runs.unshift(run);
        store.snapshot.runs = store.snapshot.runs.slice(0, 200);
        save(store);
        return run;
      },
      schedule: async () => {
        throw new Error("Scheduling requires the desktop app to be running.");
      },
      deleteSchedule: async () => {
        throw new Error("Scheduling requires the desktop app.");
      },
      openArtifact: async () => {
        throw new Error("Files are created in the desktop app.");
      },
      stopForeground: async () => {
        throw new Error("Foreground automation requires the desktop app.");
      },
      approveForegroundSend: async () => {
        throw new Error("Foreground automation requires the desktop app.");
      },
    };
