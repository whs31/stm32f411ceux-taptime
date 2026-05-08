import { createPromiseClient } from "@connectrpc/connect";
import { StoreService as StoreServiceRpc } from "@taptime/proto/taptime/services/store_connect.js";
import { Date as ProtoDate } from "@taptime/proto/taptime/date_pb.js";
import { LocalTime } from "@taptime/proto/taptime/local_time_pb.js";
import { EventRequest } from "@taptime/proto/taptime/services/store_pb.js";
import { transport } from "$lib/grpc";
import { AuthService } from "./AuthService";

export class StoreService {
  private static client = createPromiseClient(StoreServiceRpc, transport);

  private static authHeaders() {
    const jwt = AuthService.getStoredJwt();
    if (!jwt) throw new Error("Not authenticated");
    return { authorization: `Bearer ${jwt}` };
  }

  static todayProtoDate(tz: string): ProtoDate {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone: tz,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    }).formatToParts(new globalThis.Date());
    const y = parseInt(parts.find((p) => p.type === "year")?.value ?? "1970");
    const mo = parseInt(parts.find((p) => p.type === "month")?.value ?? "1");
    const d = parseInt(parts.find((p) => p.type === "day")?.value ?? "1");
    const utcMs = globalThis.Date.UTC(y, mo - 1, d);
    return new ProtoDate({ daysSinceEpoch: Math.floor(utcMs / 86_400_000) });
  }

  static nowLocalTime(tz: string): LocalTime {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone: tz,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hourCycle: "h23",
    }).formatToParts(new globalThis.Date());
    return new LocalTime({
      hour: parseInt(parts.find((p) => p.type === "hour")?.value ?? "0") % 24,
      minute: parseInt(parts.find((p) => p.type === "minute")?.value ?? "0"),
      second: parseInt(parts.find((p) => p.type === "second")?.value ?? "0"),
    });
  }

  static async getDay(date: ProtoDate) {
    return this.client.getDay(date, { headers: this.authHeaders() });
  }

  static async addCheckIn(date: ProtoDate, time: LocalTime) {
    return this.client.addCheckIn(new EventRequest({ date, time }), {
      headers: this.authHeaders(),
    });
  }

  static async addCheckOut(date: ProtoDate, time: LocalTime) {
    return this.client.addCheckOut(new EventRequest({ date, time }), {
      headers: this.authHeaders(),
    });
  }
}
