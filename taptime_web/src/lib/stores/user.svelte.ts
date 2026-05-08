import type { User } from "@taptime/proto/taptime/user_pb.js";
import { AuthService } from "$lib/services";

let _user = $state<User | null>(null);
let _loading = $state(false);
let _error = $state<string | null>(null);

async function fetchUser(): Promise<void> {
  _loading = true;
  _error = null;
  try {
    _user = await AuthService.getUser();
  } catch (e) {
    _error = e instanceof Error ? e.message : String(e);
    _user = null;
  } finally {
    _loading = false;
  }
}

function clearUser(): void {
  _user = null;
  _error = null;
  AuthService.logout();
}

export const userStore = {
  get user(): User | null {
    return _user;
  },
  get loading(): boolean {
    return _loading;
  },
  get error(): string | null {
    return _error;
  },
  fetch: fetchUser,
  clear: clearUser,
};
