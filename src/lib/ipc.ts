// The only file importing @tauri-apps/api (+ the dialog plugin) — every
// other module calls through these typed wrappers (CONVENTIONS "Folder rules").
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  AddDownloadRequest,
  AppError,
  BinaryDownloadEvent,
  BinaryHealthEvent,
  BinaryStatus,
  BinaryStatuses,
  BulkActionRequest,
  CreatePresetRequest,
  Item,
  ItemRemovedEvent,
  LogLine,
  LogLineEvent,
  Preset,
  PresetListResponse,
  ProbeFormatsRequest,
  ProbeFormatsResponse,
  ProgressEvent,
  Settings,
  SettingsUpdate,
  StageChangedEvent,
  UpdatePresetRequest,
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

// T17: S7 "Re-check" re-runs detection (same shape as detect_binaries, but
// the wire command is named recheck_binaries per ARCHITECTURE §7.2).
export function recheckBinaries(): Promise<BinaryStatuses> {
  return call<BinaryStatuses>("recheck_binaries");
}

export function setBinaryPath(which: "ytdlp" | "ffmpeg", path: string): Promise<BinaryStatus> {
  return call<BinaryStatus>("set_binary_path", { request: { which, path } });
}

export function downloadBinary(which: "ytdlp" | "ffmpeg"): Promise<BinaryStatus> {
  return call<BinaryStatus>("download_binary", { request: { which } });
}

export function onBinaryDownload(cb: (payload: BinaryDownloadEvent) => void): Promise<UnlistenFn> {
  return listen<BinaryDownloadEvent>("binary_download", (event) => cb(event.payload));
}

export function onBinaryHealth(cb: (payload: BinaryHealthEvent) => void): Promise<UnlistenFn> {
  return listen<BinaryHealthEvent>("binary_health", (event) => cb(event.payload));
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

export function getItem(id: number): Promise<Item> {
  return call<Item>("get_item", { request: { id } });
}

// Native file picker for Onboarding's "Set path…" (Tauri 2's standard dialog
// plugin). Returns null if the user cancels.
export async function pickBinaryPath(): Promise<string | null> {
  const result = await open({ multiple: false, directory: false });
  return typeof result === "string" ? result : null;
}

// T17: S7's "Default output dir" [...] picker.
export async function pickDirectory(): Promise<string | null> {
  const result = await open({ multiple: false, directory: true });
  return typeof result === "string" ? result : null;
}

export function onProgress(cb: (payload: ProgressEvent) => void): Promise<UnlistenFn> {
  return listen<ProgressEvent>("progress", (event) => cb(event.payload));
}

export function onStageChanged(cb: (payload: StageChangedEvent) => void): Promise<UnlistenFn> {
  return listen<StageChangedEvent>("stage_changed", (event) => cb(event.payload));
}

export function onItemAdded(cb: (payload: Item) => void): Promise<UnlistenFn> {
  return listen<Item>("item_added", (event) => cb(event.payload));
}

export function onItemRemoved(cb: (payload: ItemRemovedEvent) => void): Promise<UnlistenFn> {
  return listen<ItemRemovedEvent>("item_removed", (event) => cb(event.payload));
}

// --- T6: queue lifecycle ----------------------------------------------------

export function pauseItem(id: number): Promise<Item> {
  return call<Item>("pause_item", { request: { id } });
}

export function resumeItem(id: number): Promise<Item> {
  return call<Item>("resume_item", { request: { id } });
}

export function cancelItem(id: number): Promise<Item> {
  return call<Item>("cancel_item", { request: { id } });
}

export function removeItem(id: number): Promise<{ ok: boolean }> {
  return call<{ ok: boolean }>("remove_item", { request: { id } });
}

export function retryItem(id: number): Promise<Item> {
  return call<Item>("retry_item", { request: { id } });
}

export function reorderItem(id: number, newPosition: number): Promise<{ ok: boolean }> {
  return call<{ ok: boolean }>("reorder_item", { request: { id, new_position: newPosition } });
}

export function bulkAction(request: BulkActionRequest): Promise<{ updated: Item[] }> {
  return call<{ updated: Item[] }>("bulk_action", { request });
}

export function setConcurrency(n: number): Promise<{ n: number }> {
  return call<{ n: number }>("set_concurrency", { request: { n } });
}

// --- T15: S5 detail drawer ---------------------------------------------------

export function getItemLog(id: number, tail?: number): Promise<LogLine[]> {
  return call<LogLine[]>("get_item_log", { request: { id, tail: tail ?? null } });
}

// Toggles whether `log_line` events fire for `id` — call with `on:true` when
// the drawer's log disclosure opens, `on:false` when it closes/unmounts.
export function watchLog(id: number, on: boolean): Promise<{ ok: boolean }> {
  return call<{ ok: boolean }>("watch_log", { request: { id, on } });
}

export function onLogLine(cb: (payload: LogLineEvent) => void): Promise<UnlistenFn> {
  return listen<LogLineEvent>("log_line", (event) => cb(event.payload));
}

export function openPath(path: string, reveal?: boolean): Promise<{ ok: boolean }> {
  return call<{ ok: boolean }>("open_path", { request: { path, reveal: reveal ?? null } });
}

// --- T9: probe (S3/S4) -------------------------------------------------------

export function probeFormats(request: ProbeFormatsRequest): Promise<ProbeFormatsResponse> {
  return call<ProbeFormatsResponse>("probe_formats", { request });
}

// --- T11: presets (S6, S3) ---------------------------------------------------

export function listPresets(): Promise<Preset[]> {
  return call<Preset[]>("list_presets");
}

export function createPreset(request: CreatePresetRequest): Promise<Preset> {
  return call<Preset>("create_preset", { request });
}

export function updatePreset(request: UpdatePresetRequest): Promise<Preset> {
  return call<Preset>("update_preset", { request });
}

export function deletePreset(id: number): Promise<PresetListResponse> {
  return call<PresetListResponse>("delete_preset", { request: { id } });
}

export function setDefaultPreset(id: number): Promise<PresetListResponse> {
  return call<PresetListResponse>("set_default_preset", { request: { id } });
}
