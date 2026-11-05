export interface AttachmentRef {
  id: string;
  name: string;
  size: number;
  sha256: string;
}
export const MAX_ATTACHMENT_FILES = 10;
export const MAX_ATTACHMENT_BYTES = 18 * 1024 * 1024;
export function validateAttachments(items: AttachmentRef[]): void {
  if (items.length > MAX_ATTACHMENT_FILES)
    throw new Error("Attach at most 10 files per email.");
  if (items.some((a) => !a.name || !Number.isSafeInteger(a.size) || a.size < 0))
    throw new Error("Invalid attachment information.");
  if (new Set(items.map((a) => a.id)).size !== items.length)
    throw new Error("Duplicate attachment reference.");
  if (items.reduce((total, a) => total + a.size, 0) > MAX_ATTACHMENT_BYTES)
    throw new Error("Attachments must total 18 MB or less per email.");
}
export function mergeAttachments(
  current: AttachmentRef[],
  incoming: AttachmentRef[],
): AttachmentRef[] {
  const result = [...current];
  for (const item of incoming)
    if (
      !result.some(
        (a) =>
          a.name === item.name &&
          a.size === item.size &&
          a.sha256 === item.sha256,
      )
    )
      result.push(item);
  validateAttachments(result);
  return result;
}
export function attachmentSize(bytes: number): string {
  return bytes < 1024
    ? `${bytes} B`
    : bytes < 1024 * 1024
      ? `${(bytes / 1024).toFixed(1)} KB`
      : `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}
