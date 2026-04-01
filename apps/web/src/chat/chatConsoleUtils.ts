import type { ChatTranscriptRecord, ConsoleApiClient } from "../consoleApi";

import type { DetailPanelState, TranscriptSearchMatch } from "./ChatInspectorColumn";
import {
  parseTapePayload,
  prettifyEventType,
  shortId,
  type ComposerAttachment,
  type TranscriptEntry,
} from "./chatShared";

export function buildDetailFromLiveEntry(entry: TranscriptEntry): DetailPanelState {
  return {
    id: entry.id,
    title: entry.title,
    subtitle: `${entry.run_id !== undefined ? `Run ${shortId(entry.run_id)} · ` : ""}${new Date(entry.created_at_unix_ms).toLocaleString()}`,
    body: entry.text,
    payload: entry.payload,
  };
}

export function buildDetailFromTranscriptRecord(record: ChatTranscriptRecord): DetailPanelState {
  return {
    id: `${record.run_id}:${record.seq}`,
    title: `${prettifyEventType(record.event_type)} #${record.seq}`,
    subtitle: `${new Date(record.created_at_unix_ms).toLocaleString()} · ${record.origin_kind}${record.origin_run_id !== undefined ? ` · from ${shortId(record.origin_run_id)}` : ""}`,
    payload: parseTapePayload(record.payload_json),
  };
}

export function buildDetailFromSearchMatch(match: TranscriptSearchMatch): DetailPanelState {
  return {
    id: `search-${match.run_id}-${match.seq}`,
    title: `${prettifyEventType(match.event_type)} #${match.seq}`,
    subtitle: `${new Date(match.created_at_unix_ms).toLocaleString()} · ${match.origin_kind}`,
    body: match.snippet,
  };
}

export function downloadTextFile(filename: string, content: string, mimeType: string): void {
  const blob = new Blob([content], { type: mimeType });
  const href = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = href;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(href);
}

export function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("Failed to read attachment."));
    reader.onload = () => {
      if (typeof reader.result !== "string") {
        reject(new Error("Attachment reader returned an unexpected payload."));
        return;
      }
      resolve(reader.result);
    };
    reader.readAsDataURL(file);
  });
}

export async function uploadComposerAttachments(
  api: ConsoleApiClient,
  sessionId: string,
  files: readonly File[],
): Promise<ComposerAttachment[]> {
  const nextAttachments: ComposerAttachment[] = [];
  for (const file of files) {
    const dataUrl = await readFileAsDataUrl(file);
    const base64 = dataUrl.includes(",") ? dataUrl.slice(dataUrl.indexOf(",") + 1) : dataUrl;
    const response = await api.uploadChatAttachment(sessionId, {
      filename: file.name,
      content_type: file.type || "application/octet-stream",
      bytes_base64: base64,
    });
    nextAttachments.push({
      local_id: `${response.attachment.artifact_id}-${Date.now()}`,
      ...response.attachment,
      preview_url: response.attachment.kind === "image" ? dataUrl : undefined,
    });
  }
  return nextAttachments;
}
