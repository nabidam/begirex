// Wire types — snake_case verbatim, mirroring the Rust structs in
// src-tauri/src/{persistence,binary_manager,settings_service,error}.rs.
// CONVENTIONS.md: "no renaming layer" — do not camelCase-ify these.

export interface BinaryStatus {
  found: boolean;
  path?: string | null;
  version?: string | null;
}

export interface BinaryStatuses {
  ytdlp: BinaryStatus;
  ffmpeg: BinaryStatus;
}

export interface Settings {
  global_proxy: string | null;
  default_concurrency: number;
  default_output_dir: string;
  default_output_template: string;
  default_preset_id: number | null;
  build_flavor: string;
  ytdlp_version: string | null;
  ffmpeg_version: string | null;
}

// Partial update payload for update_settings — only present fields are applied.
export interface SettingsUpdate {
  global_proxy?: string | null;
  default_concurrency?: number;
  default_output_dir?: string;
  default_output_template?: string;
  default_preset_id?: number | null;
}

export interface Item {
  id: number;
  url: string;
  playlist_id: string | null;
  title: string | null;
  stage: string;
  format_expr: string;
  output_dir: string;
  output_template: string;
  proxy: string | null;
  extra_args: string | null;
  preset_id: number | null;
  total_bytes: number | null;
  downloaded_bytes: number;
  percent: number;
  speed_bps: number | null;
  eta_seconds: number | null;
  resume_capable: boolean;
  output_path: string | null;
  error_message: string | null;
  queue_position: number;
  created_at: number;
  updated_at: number;
}

export interface AddDownloadRequest {
  url: string;
  format_expr: string;
  output_dir?: string | null;
  output_template?: string | null;
  proxy?: string | null;
  extra_args?: string | null;
  preset_id?: number | null;
}

// error.rs: #[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
export type AppErrorCode =
  | "BINARY_NOT_FOUND"
  | "BINARY_DOWNLOAD_FAILED"
  | "PROBE_FAILED"
  | "INVALID_FORMAT_EXPR"
  | "DUPLICATE_URL"
  | "PRESET_NAME_TAKEN"
  | "LAST_PRESET"
  | "DB_ERROR"
  | "PROCESS_ERROR"
  | "VALIDATION"
  | "IO_ERROR";

export interface AppError {
  code: AppErrorCode;
  message: string;
  stderr?: string | null;
}

// Event payloads (ipc.rs's ProgressPayload / StageChangedPayload).
export interface ProgressEvent {
  id: number;
  percent: number;
  downloaded_bytes: number | null;
  total_bytes: number | null;
  speed_bps: number | null;
  eta_seconds: number | null;
  stage: string;
}

export interface StageChangedEvent {
  id: number;
  stage: string;
  error_message: string | null;
}

// ipc.rs's ItemRemovedPayload; item_added carries the full Item verbatim.
export interface ItemRemovedEvent {
  id: number;
}

export interface BulkActionRequest {
  ids: number[];
  action: "pause" | "resume" | "cancel" | "remove";
}

// persistence.rs's Preset (ARCHITECTURE §3 presets table).
export interface Preset {
  id: number;
  name: string;
  format_expr: string;
  output_template: string;
  proxy: string | null;
  extra_args: string | null;
  is_default: boolean;
  created_at: number;
  updated_at: number;
}

// preset_service.rs's CreatePresetRequest.
export interface CreatePresetRequest {
  name: string;
  format_expr: string;
  output_template: string;
  proxy?: string | null;
  extra_args?: string | null;
  is_default?: boolean;
}

// preset_service.rs's UpdatePresetRequest, flattened with `id` by
// ipc.rs's UpdatePresetPathRequest — only present fields are applied.
export interface UpdatePresetRequest {
  id: number;
  name?: string;
  format_expr?: string;
  output_template?: string;
  proxy?: string | null;
  extra_args?: string | null;
}

export interface PresetListResponse {
  presets: Preset[];
}

// engine_supervisor.rs's ProbeFormat / ProbeResult (ARCHITECTURE §7.2).
export interface Format {
  id: string;
  resolution: string | null;
  ext: string;
  fps: number | null;
  filesize: number | null;
  codec: string | null;
  note: string | null;
  has_audio: boolean;
}

export interface ProbeFormatsRequest {
  url: string;
  proxy?: string | null;
}

export interface ProbeFormatsResponse {
  title: string;
  formats: Format[];
}
