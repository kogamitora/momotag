/**
 * File-name rules shared by the editor and the native write command.
 * Keep this module side-effect free so it can be tested without Tauri.
 */
export function sanitizeFileName(title: string): string {
  const cleaned = title
    .replace(/[<>:"/\\|?*\x00-\x1F]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^[. ]+|[. ]+$/g, "");

  return cleaned || "Untitled";
}

export function inferTitleFromFileName(fileName: string): string {
  const stem = fileName.replace(/\.[^.\\/]+$/, "").trim();
  const withoutNumber = stem
    .replace(/^\s*(?:tr(?:ack)?\.?\s*)?\d{1,3}[\s._-]+/i, "")
    .trim();

  return withoutNumber || stem;
}
