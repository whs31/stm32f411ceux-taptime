import type { Day } from "@taptime/proto/taptime/day_pb.js";
import { StoreService } from "$lib/services";
import { userStore } from "$lib/stores";

let _today = $state<Day | null>(null);
let _loading = $state(false);
let _error = $state<string | null>(null);

async function loadToday(): Promise<void> {
  _loading = true;
  _error = null;
  try {
    const tz = userStore.user?.timeZone;
    if (!tz) return;
    _today = await StoreService.getDay(StoreService.currentDate(tz.timeZone));
  } catch (e) {
    _error = e instanceof Error ? e.message : String(e);
    _today = null;
  } finally {
    _loading = false;
  }
}

export const todayStore = {
  get today(): Day | null {
    return _today;
  },
  get loading(): boolean {
    return _loading;
  },
  get error(): string | null {
    return _error;
  },
  load: loadToday,
};
