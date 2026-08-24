import { invoke } from "@tauri-apps/api/core";
import type { AppError } from "../types";

export class TauriUnavailableError extends Error {
  constructor() {
    super("Tauri runtime is unavailable.");
    this.name = "TauriUnavailableError";
  }
}

export async function invokeCommand<T>(
  command: string,
  payload?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, payload);
  } catch (error) {
    throw normalizeNativeError(error);
  }
}

export function isTauriUnavailableError(error: unknown): boolean {
  return error instanceof TauriUnavailableError;
}

function normalizeNativeError(error: unknown): Error | AppError {
  if (isAppError(error)) return error;

  const message = error instanceof Error ? error.message : String(error);
  if (message.trim().startsWith("{")) {
    try {
      const parsed: unknown = JSON.parse(message);
      if (isAppError(parsed)) return parsed;
    } catch {
      // Keep the original message when a native runtime returns non-JSON text.
    }
  }
  if (
    /invoke|tauri|ipc|webview/i.test(message) &&
    /not available|not defined|undefined|cannot read|failed/i.test(message)
  ) {
    return new TauriUnavailableError();
  }

  return error instanceof Error ? error : new Error(message);
}

export function isAppError(error: unknown): error is AppError {
  return Boolean(
    error &&
      typeof error === "object" &&
      "code" in error &&
      typeof error.code === "string" &&
      "message" in error &&
      typeof error.message === "string",
  );
}
