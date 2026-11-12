import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
import { recipientAddresses } from "../domain/recipients";
import type { RecipientUserGroup } from "../domain/recipientUserGroups";
const key = "taskhub.recipient-groups.preview.v1";
function read(): RecipientUserGroup[] {
  return JSON.parse(localStorage.getItem(key) ?? "[]");
}
export const userGroups = {
  async list(): Promise<RecipientUserGroup[]> {
    return bridge.native ? invoke("list_recipient_groups") : read();
  },
  async save(
    id: string | null,
    name: string,
    members: string[],
  ): Promise<void> {
    name = name.trim();
    members = [...new Set(members.map((m) => m.toLowerCase()))];
    if (!name || name.length > 100)
      throw new Error("Enter a group name of 1–100 characters.");
    recipientAddresses(members.join(", "));
    if (bridge.native)
      return invoke("save_recipient_group", { id, name, members });
    const groups = read();
    if (
      groups.some(
        (g) => g.id !== id && g.name.toLowerCase() === name.toLowerCase(),
      )
    )
      throw new Error("A group with this name already exists.");
    const group = { id: id ?? crypto.randomUUID(), name, members };
    localStorage.setItem(
      key,
      JSON.stringify(
        id ? groups.map((g) => (g.id === id ? group : g)) : [...groups, group],
      ),
    );
  },
  async remove(id: string): Promise<void> {
    if (bridge.native) return invoke("delete_recipient_group", { id });
    localStorage.setItem(
      key,
      JSON.stringify(read().filter((g) => g.id !== id)),
    );
  },
};
