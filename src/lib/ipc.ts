// The only file importing @tauri-apps/api (+ the dialog plugin) — every
// other module calls through these typed wrappers (CONVENTIONS "Folder rules").
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AddDownloadRequest,
  AppError,
  BinaryStatus,
  BinaryStatuses,
  Item,
  ProgressEvent,
  Settings,
  SettingsUpdate,
  StageChangedEvent,
} from "./types";

// Tauri rejects with the AppError JSON shape (serde `Err` side) — normalize
// whatever comes back into a typed AppError rather than swallowing it.
function toAppError(err: unknown): AppError {
  if (err && typeof err === "object" && "code" in err && "message" in err) {
    return err as AppError;
  }
  return { code: "IO_ERROR", message: String(err) };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    const appError = toAppError(err);
    console.error(`[ipc] ${cmd} failed:`, appError);
    throw appError;
  }
}

export function detectBinaries(): Promise<BinaryStatuses> {
  return call<BinaryStatuses>("detect_binaries");
}

export function setBinaryPath(which: "ytdlp" | "ffmpeg", path: string): Promise<BinaryStatus> {
  return call<BinaryStatus>("set_binary_path", { request: { which, path } });
}

export function getSettings(): Promise<Settings> {
  return call<Settings>("get_settings");
}

export function updateSettings(update: SettingsUpdate): Promise<Settings> {
  // NB: the Rust fn param is named `update`, not `request` — see ipc.rs's
  // `update_settings(state, update: SettingsUpdate)`.
  return call<Settings>("update_settings", { update });
}

export function addDownload(request: AddDownloadRequest): Promise<{ items: Item[] }> {
  return call<{ items: Item[] }>("add_download", { request });
}

export function listItems(filter?: string): Promise<Item[]> {
  return call<Item[]>("list_items", { request: { filter: filter ?? null } });
}

// Native file picker for Onboarding's "Set path…" (Tauri 2's standard dialog
// plugin). Returns null if the user cancels.
export async function pickBinaryPath(): Promise<string | null> {
  const result = await open({ multiple: false, directory: false });
  return typeof result === "string" ? result : null;
}

export function onProgress(cb: (payload: ProgressEvent) => void): Promise<UnlistenFn> {
  return listen<ProgressEvent>("progress", (event) => cb(event.payload));
}

export function onStageChanged(cb: (payload: StageChangedEvent) => void): Promise<UnlistenFn> {
  return listen<StageChangedEvent>("stage_changed", (event) => cb(event.payload));
}
