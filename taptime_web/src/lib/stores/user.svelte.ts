import { createPromiseClient } from "@connectrpc/connect";
import { AuthService } from "@taptime/proto/taptime/services/auth_connect.js";
import type { User } from "@taptime/proto/taptime/user_pb.js";
import { transport } from "$lib/grpc";

const client = createPromiseClient(AuthService, transport);

let _user = $state<User | null>(null);
let _loading = $state(false);
let _error = $state<string | null>(null);

async function fetchUser(): Promise<void> {
  if (typeof localStorage === "undefined") return;
  const jwt = localStorage.getItem("jwt");
  if (!jwt) {
    _user = null;
    return;
  }
  _loading = true;
  _error = null;
  try {
    _user = await client.getUser(
      {},
      { headers: { authorization: `Bearer ${jwt}` } },
    );
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
  if (typeof localStorage !== "undefined") localStorage.removeItem("jwt");
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
