// Settings store (ARCHITECTURE §2) — hydrates from backend, updated via ipc
// calls. Also carries binary health since Onboarding (S1) needs both to
// decide when to dismiss.
import { detectBinaries, getSettings, setBinaryPath, updateSettings } from "../ipc";
import type { AppError, BinaryStatuses, Settings } from "../types";

function createSettingsStore() {
  let settings = $state<Settings | null>(null);
  let binaries = $state<BinaryStatuses | null>(null);
  let error = $state<string | null>(null);

  async function init() {
    error = null;
    try {
      const [s, b] = await Promise.all([getSettings(), detectBinaries()]);
      settings = s;
      binaries = b;
    } catch (err) {
      error = (err as AppError).message;
    }
  }

  async function resolveBinaryPath(which: "ytdlp" | "ffmpeg", path: string) {
    error = null;
    try {
      const status = await setBinaryPath(which, path);
      if (binaries) {
        binaries = { ...binaries, [which]: status };
      }
    } catch (err) {
      error = (err as AppError).message;
    }
  }

  async function saveProxy(globalProxy: string) {
    error = null;
    try {
      settings = await updateSettings({ global_proxy: globalProxy || null });
    } catch (err) {
      error = (err as AppError).message;
    }
  }

  return {
    get settings() {
      return settings;
    },
    get binaries() {
      return binaries;
    },
    get error() {
      return error;
    },
    init,
    resolveBinaryPath,
    saveProxy,
  };
}

export const settingsStore = createSettingsStore();
