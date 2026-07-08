// Settings store (ARCHITECTURE §2) — hydrates from backend, updated via ipc
// calls. Also carries binary health since Onboarding (S1) needs both to
// decide when to dismiss, plus (T17) the in-app binary fetch used by S1's
// "Download for me" and S7's "Re-check"/"Change…".
import {
  detectBinaries,
  downloadBinary as ipcDownloadBinary,
  getSettings,
  onBinaryDownload,
  recheckBinaries,
  setBinaryPath,
  updateSettings,
} from "../ipc";
import type { AppError, BinaryStatuses, Settings, SettingsUpdate } from "../types";

type Which = "ytdlp" | "ffmpeg";

// T17: per-binary in-app fetch state (S1 AC1's determinate bar + cancel +
// inline failure/Retry/stderr). Cancel has no backend counterpart (T17's
// file list is frontend-only, no ipc.rs command exists to abort a
// download_binary in flight) — ponytail: "cancel" just stops the UI from
// awaiting/showing the fetch; the backend request still runs to completion
// in the background. Upgrade path: a real abort once ipc.rs grows a cancel
// command for it.
interface DownloadState {
  active: boolean;
  percent: number;
  error: string | null;
  stderr: string | null;
}

function emptyDownloadState(): DownloadState {
  return { active: false, percent: 0, error: null, stderr: null };
}

function createSettingsStore() {
  let settings = $state<Settings | null>(null);
  let binaries = $state<BinaryStatuses | null>(null);
  let error = $state<string | null>(null);
  let downloads = $state<Record<Which, DownloadState>>({
    ytdlp: emptyDownloadState(),
    ffmpeg: emptyDownloadState(),
  });
  let progressListenerStarted = false;

  async function init() {
    error = null;
    try {
      const [s, b] = await Promise.all([getSettings(), detectBinaries()]);
      settings = s;
      binaries = b;
    } catch (err) {
      error = (err as AppError).message;
    }
    if (!progressListenerStarted) {
      progressListenerStarted = true;
      onBinaryDownload((payload) => {
        const prev = downloads[payload.which];
        downloads = { ...downloads, [payload.which]: { ...prev, active: true, percent: payload.percent } };
      });
    }
  }

  async function resolveBinaryPath(which: Which, path: string) {
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

  // S1 AC1's "Download for me": fetches the binary in-app, tracking
  // determinate progress via the `binary_download` event; failure surfaces
  // inline on that binary's row (Retry re-calls this) without blocking the
  // other binary (K1-AC1/AC2, PRD §7).
  async function downloadBinary(which: Which) {
    downloads = { ...downloads, [which]: { active: true, percent: 0, error: null, stderr: null } };
    try {
      const status = await ipcDownloadBinary(which);
      if (binaries) binaries = { ...binaries, [which]: status };
      downloads = { ...downloads, [which]: emptyDownloadState() };
    } catch (err) {
      const appError = err as AppError;
      downloads = {
        ...downloads,
        [which]: { active: false, percent: 0, error: appError.message, stderr: appError.stderr ?? null },
      };
    }
  }

  // S7 AC3's "Re-check re-runs detection".
  async function recheck() {
    error = null;
    try {
      binaries = await recheckBinaries();
    } catch (err) {
      error = (err as AppError).message;
    }
  }

  async function saveProxy(globalProxy: string) {
    return update({ global_proxy: globalProxy || null });
  }

  // Generic Settings (S7) field save — N, output dir/template, default
  // preset, proxy all round-trip through this one command (ARCHITECTURE §9
  // single source of runtime config).
  async function update(partial: SettingsUpdate): Promise<boolean> {
    error = null;
    try {
      settings = await updateSettings(partial);
      return true;
    } catch (err) {
      error = (err as AppError).message;
      return false;
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
    get downloads() {
      return downloads;
    },
    init,
    resolveBinaryPath,
    downloadBinary,
    recheck,
    saveProxy,
    update,
  };
}

export const settingsStore = createSettingsStore();
