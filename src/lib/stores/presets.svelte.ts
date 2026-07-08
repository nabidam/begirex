// Presets store (ARCHITECTURE §2) — hydrates from backend, updated via ipc
// calls. S6 (Presets view) and S3 (Add Download's preset dropdown) both read
// from this one store per CONVENTIONS "one store per ARCHITECTURE §2 store
// name".
import {
  createPreset as ipcCreatePreset,
  deletePreset as ipcDeletePreset,
  listPresets,
  setDefaultPreset as ipcSetDefaultPreset,
  updatePreset as ipcUpdatePreset,
} from "../ipc";
import type { AppError, CreatePresetRequest, Preset, UpdatePresetRequest } from "../types";

function createPresetsStore() {
  let presets = $state<Preset[]>([]);
  let error = $state<AppError | null>(null);

  async function init() {
    error = null;
    try {
      presets = await listPresets();
    } catch (err) {
      error = err as AppError;
    }
  }

  async function create(request: CreatePresetRequest): Promise<Preset | null> {
    error = null;
    try {
      const preset = await ipcCreatePreset(request);
      presets = [...presets, preset].sort(sortPresets);
      return preset;
    } catch (err) {
      error = err as AppError;
      return null;
    }
  }

  async function update(request: UpdatePresetRequest): Promise<Preset | null> {
    error = null;
    try {
      const preset = await ipcUpdatePreset(request);
      presets = presets.map((p) => (p.id === preset.id ? preset : p)).sort(sortPresets);
      return preset;
    } catch (err) {
      error = err as AppError;
      return null;
    }
  }

  async function remove(id: number): Promise<boolean> {
    error = null;
    try {
      const { presets: updated } = await ipcDeletePreset(id);
      presets = updated.sort(sortPresets);
      return true;
    } catch (err) {
      error = err as AppError;
      return false;
    }
  }

  async function setDefault(id: number): Promise<boolean> {
    error = null;
    try {
      const { presets: updated } = await ipcSetDefaultPreset(id);
      presets = updated.sort(sortPresets);
      return true;
    } catch (err) {
      error = err as AppError;
      return false;
    }
  }

  function sortPresets(a: Preset, b: Preset): number {
    if (a.is_default !== b.is_default) return a.is_default ? -1 : 1;
    return a.name.localeCompare(b.name);
  }

  return {
    get presets() {
      return presets;
    },
    get defaultPreset() {
      return presets.find((p) => p.is_default) ?? null;
    },
    get error() {
      return error;
    },
    init,
    create,
    update,
    remove,
    setDefault,
  };
}

export const presetsStore = createPresetsStore();
