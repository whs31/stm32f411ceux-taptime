import { LocalTime } from "$lib/types";
import { Event as EventProto } from "@taptime/proto/taptime/event_pb.js";

export enum EventType {
  CHECK_IN = "CHECK_IN",
  CHECK_OUT = "CHECK_OUT",
}

interface EventBase {
  readonly time: LocalTime;
  readonly type: EventType;

  toString(): string;
  encode(): EventProto;
}

export class CheckInEvent implements EventBase {
  readonly type = EventType.CHECK_IN;

  private constructor(readonly time: LocalTime) {}

  static create(time: LocalTime): CheckInEvent {
    return new CheckInEvent(time);
  }

  static from(hour: number, minute: number, second: number = 0): CheckInEvent {
    return new CheckInEvent(LocalTime.from(hour, minute, second));
  }

  static fromString(timeString: string): CheckInEvent {
    return new CheckInEvent(LocalTime.fromString(timeString));
  }

  toString(): string {
    return `Check-in at ${this.time.toString()}`;
  }

  encode(): EventProto {
    const event = new EventProto();
    event.eventType = {
      case: "checkIn",
      value: this.time.encode(),
    };
    return event;
  }

  isCheckIn(): this is CheckInEvent {
    return true;
  }

  isCheckOut(): this is CheckOutEvent {
    return false;
  }
}

export class CheckOutEvent implements EventBase {
  readonly type = EventType.CHECK_OUT;

  private constructor(readonly time: LocalTime) {}

  static create(time: LocalTime): CheckOutEvent {
    return new CheckOutEvent(time);
  }

  static from(hour: number, minute: number, second: number = 0): CheckOutEvent {
    return new CheckOutEvent(LocalTime.from(hour, minute, second));
  }

  static fromString(timeString: string): CheckOutEvent {
    return new CheckOutEvent(LocalTime.fromString(timeString));
  }

  toString(): string {
    return `Check-out at ${this.time.toString()}`;
  }

  encode(): EventProto {
    const event = new EventProto();
    event.eventType = {
      case: "checkOut",
      value: this.time.encode(),
    };
    return event;
  }

  isCheckIn(): this is CheckInEvent {
    return false;
  }

  isCheckOut(): this is CheckOutEvent {
    return true;
  }
}

export type Event = CheckInEvent | CheckOutEvent;

export const EventUtils = {
  decode(proto: EventProto): Event {
    switch (proto.eventType.case) {
      case "checkIn":
        return CheckInEvent.create(LocalTime.decode(proto.eventType.value));
      case "checkOut":
        return CheckOutEvent.create(LocalTime.decode(proto.eventType.value));
      default:
        throw new Error(`Unknown event type: ${proto.eventType.case}`);
    }
  },

  isCheckIn(event: Event): event is CheckInEvent {
    return event.isCheckIn();
  },

  isCheckOut(event: Event): event is CheckOutEvent {
    return event.isCheckOut();
  },

  getTime(event: Event): LocalTime {
    return event.time;
  },

  encodeArray(events: Event[]): EventProto[] {
    return events.map((event) => event.encode());
  },

  decodeArray(events: EventProto[]): Event[] {
    return events.map((event) => EventUtils.decode(event));
  },
};
