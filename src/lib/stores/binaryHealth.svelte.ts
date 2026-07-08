// T16 mid-session binary health (ARCHITECTURE §7.3 `binary_health`). Backend
// emits this only when a resolved binary vanishes mid-session (K1-AC7) — the
// store just remembers the most recent missing one so GlobalBanner can show
// it app-wide; a later `found:true` for the same binary clears it.
import { onBinaryHealth } from "../ipc";
import type { BinaryHealthEvent } from "../types";

function createBinaryHealthStore() {
  let missing = $state<BinaryHealthEvent | null>(null);

  function init() {
    onBinaryHealth((payload) => {
      if (!payload.found) {
        missing = payload;
      } else if (missing?.which === payload.which) {
        missing = null;
      }
    });
  }

  function clear() {
    missing = null;
  }

  return {
    get missing() {
      return missing;
    },
    init,
    clear,
  };
}

export const binaryHealthStore = createBinaryHealthStore();
