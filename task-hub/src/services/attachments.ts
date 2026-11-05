import { invoke } from "@tauri-apps/api/core";
import { bridge } from "./bridge";
import { validateAttachments, type AttachmentRef } from "../domain/attachments";
export const attachmentService = {
  async pick(): Promise<AttachmentRef[]> {
    if (!bridge.native)
      throw new Error("Use the file input in browser preview.");
    return invoke("pick_email_attachments");
  },
  async preview(files: File[]): Promise<AttachmentRef[]> {
    const items = files.map((file) => ({
      id: crypto.randomUUID(),
      name: file.name,
      size: file.size,
      sha256: "",
    }));
    validateAttachments(items);
    for (let i = 0; i < files.length; i++) {
      const digest = await crypto.subtle.digest(
        "SHA-256",
        await files[i].arrayBuffer(),
      );
      items[i].sha256 = [...new Uint8Array(digest)]
        .map((n) => n.toString(16).padStart(2, "0"))
        .join("");
    }
    return items;
  },
};
