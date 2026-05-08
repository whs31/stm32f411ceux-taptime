import { LocalTime as LocalTimeProto } from "@taptime/proto/taptime/local_time_pb.js";

export class LocalTime {
  private readonly hour: number;
  private readonly minute: number;
  private readonly second: number;

  constructor(hour: number, minute: number, second: number) {
    if (
      hour < 0 ||
      hour >= 24 ||
      minute < 0 ||
      minute >= 60 ||
      second < 0 ||
      second >= 60
    )
      throw new Error(`Invalid time values: ${hour}:${minute}:${second}`);
    this.hour = hour;
    this.minute = minute;
    this.second = second;
  }

  static from(hour: number, minute: number, second: number = 0): LocalTime {
    return new LocalTime(hour, minute, second);
  }

  static fromDateTime(dateTime: Date): LocalTime {
    return new LocalTime(
      dateTime.getHours(),
      dateTime.getMinutes(),
      dateTime.getSeconds(),
    );
  }

  static fromString(timeString: string): LocalTime {
    const parts = timeString.split(":").map(Number);
    if (parts.length < 2 || parts.length > 3)
      throw new Error(
        `Invalid time string format: ${timeString}. Expected HH:mm or HH:mm:ss`,
      );
    const hour = parts[0];
    const minute = parts[1];
    const second = parts.length === 3 ? parts[2] : 0;
    return new LocalTime(hour, minute, second);
  }

  static now(tz: string): LocalTime {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone: tz,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hourCycle: "h23",
    }).formatToParts(new globalThis.Date());
    return new LocalTime(
      parseInt(parts.find((p) => p.type === "hour")?.value ?? "0") % 24,
      parseInt(parts.find((p) => p.type === "minute")?.value ?? "0"),
      parseInt(parts.find((p) => p.type === "second")?.value ?? "0"),
    );
  }

  static decode(proto: LocalTimeProto): LocalTime {
    return new LocalTime(proto.hour, proto.minute, proto.second);
  }

  encode(): LocalTimeProto {
    return new LocalTimeProto({
      hour: this.hour,
      minute: this.minute,
      second: this.second,
    });
  }

  /**
   * Convert to a string in HH:mm:ss format
   */
  toString(): string {
    return `${this.pad(this.hour)}:${this.pad(this.minute)}:${this.pad(this.second)}`;
  }

  /**
   * Convert to a string in HH:mm format
   */
  toShortString(): string {
    return `${this.pad(this.hour)}:${this.pad(this.minute)}`;
  }

  /**
   * Convert to a string in 12-hour format with AM/PM
   */
  to12HourString(): string {
    const hour12 =
      this.hour === 0 ? 12 : this.hour > 12 ? this.hour - 12 : this.hour;
    const amPm = this.hour >= 12 ? "PM" : "AM";
    return `${this.pad(hour12)}:${this.pad(this.minute)}:${this.pad(this.second)} ${amPm}`;
  }

  /**
   * Convert to milliseconds since midnight
   */
  toMillisSinceMidnight(): number {
    return (this.hour * 3600 + this.minute * 60 + this.second) * 1000;
  }

  /**
   * Convert to total seconds since midnight
   */
  toSecondsSinceMidnight(): number {
    return this.hour * 3600 + this.minute * 60 + this.second;
  }

  /**
   * Get the hour component (0-23)
   */
  getHour(): number {
    return this.hour;
  }

  /**
   * Get the minute component (0-59)
   */
  getMinute(): number {
    return this.minute;
  }

  /**
   * Get the second component (0-59)
   */
  getSecond(): number {
    return this.second;
  }

  /**
   * Compare this time with another LocalTime
   * Returns negative if this is before other, positive if after, 0 if equal
   */
  compareTo(other: LocalTime): number {
    const thisSeconds = this.toSecondsSinceMidnight();
    const otherSeconds = other.toSecondsSinceMidnight();
    return thisSeconds - otherSeconds;
  }

  /**
   * Check if this time is before another time
   */
  isBefore(other: LocalTime): boolean {
    return this.compareTo(other) < 0;
  }

  /**
   * Check if this time is after another time
   */
  isAfter(other: LocalTime): boolean {
    return this.compareTo(other) > 0;
  }

  /**
   * Check if this time equals another time
   */
  equals(other: LocalTime): boolean {
    return this.compareTo(other) === 0;
  }

  /**
   * Add hours to this time
   */
  addHours(hours: number): LocalTime {
    return this.addMinutes(hours * 60);
  }

  /**
   * Add minutes to this time
   */
  addMinutes(minutes: number): LocalTime {
    return this.addSeconds(minutes * 60);
  }

  /**
   * Add seconds to this time
   */
  addSeconds(seconds: number): LocalTime {
    const totalSeconds = this.toSecondsSinceMidnight() + seconds;
    const normalizedSeconds = ((totalSeconds % 86400) + 86400) % 86400; // Handle negative and wrap around

    const hour = Math.floor(normalizedSeconds / 3600);
    const minute = Math.floor((normalizedSeconds % 3600) / 60);
    const second = normalizedSeconds % 60;

    return new LocalTime(hour, minute, second);
  }

  /**
   * Calculate the difference in seconds between this time and another
   */
  secondsBetween(other: LocalTime): number {
    return this.toSecondsSinceMidnight() - other.toSecondsSinceMidnight();
  }

  /**
   * Pad a number with leading zero if needed
   */
  private pad(num: number): string {
    return num.toString().padStart(2, "0");
  }
}
