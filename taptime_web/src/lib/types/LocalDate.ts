import { Date as LocalDateProto } from "@taptime/proto/taptime/date_pb.js";
import {
  format,
  parse,
  addDays,
  addWeeks,
  addMonths,
  addYears,
  isBefore,
  isAfter,
  isEqual,
  differenceInDays,
  startOfMonth,
  endOfMonth,
  getYear,
  getMonth,
  getDate,
  getDay,
  getDaysInMonth,
  isValid,
  startOfDay,
} from "date-fns";

export class LocalDate {
  private readonly daysSinceEpoch: number;
  private readonly internalDate: Date;

  private constructor(daysSinceEpoch: number) {
    this.daysSinceEpoch = daysSinceEpoch;
    this.internalDate = new Date(daysSinceEpoch * 86400000);
  }

  static from(year: number, month: number, day: number): LocalDate {
    const jsDate = new Date(year, month - 1, day);
    if (!isValid(jsDate)) {
      throw new Error(
        `Invalid date components: year=${year}, month=${month}, day=${day}`,
      );
    }
    if (
      getYear(jsDate) !== year ||
      getMonth(jsDate) + 1 !== month ||
      getDate(jsDate) !== day
    ) {
      throw new Error(`Invalid date: ${year}-${month}-${day}`);
    }
    const daysSinceEpoch = Math.floor(jsDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  /**
   * Create a Date from a JavaScript Date object
   * @param jsDate - JavaScript Date object
   * @param useUTC - If true, use UTC date components (default: true for consistency)
   */
  static fromJSDate(jsDate: Date, useUTC: boolean = true): LocalDate {
    if (!isValid(jsDate)) throw new Error("Invalid JavaScript Date object");
    const normalizedDate = startOfDay(jsDate);
    if (useUTC) {
      const year = jsDate.getUTCFullYear();
      const month = jsDate.getUTCMonth() + 1;
      const day = jsDate.getUTCDate();
      return LocalDate.from(year, month, day);
    }
    const daysSinceEpoch = Math.floor(normalizedDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  /**
   * Create a Date from a string in YYYY-MM-DD format
   */
  static fromString(dateString: string): LocalDate {
    const parsedDate = parse(dateString, "yyyy-MM-dd", new Date());
    if (!isValid(parsedDate))
      throw new Error(
        `Invalid date string: ${dateString}. Expected YYYY-MM-DD format`,
      );
    const daysSinceEpoch = Math.floor(parsedDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  /**
   * Create a Date from a string with custom format
   */
  static fromFormattedString(dateString: string, formatStr: string): LocalDate {
    const parsedDate = parse(dateString, formatStr, new Date());
    if (!isValid(parsedDate))
      throw new Error(
        `Invalid date string: "${dateString}" for format "${formatStr}"`,
      );
    const daysSinceEpoch = Math.floor(parsedDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  /**
   * Create a Date representing today's date
   */
  static today(): LocalDate {
    const today = startOfDay(new Date());
    const daysSinceEpoch = Math.floor(today.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  /**
   * Create a Date from days since epoch
   */
  static fromDaysSinceEpoch(days: number): LocalDate {
    return new LocalDate(days);
  }

  static decode(proto: LocalDateProto): LocalDate {
    return new LocalDate(proto.daysSinceEpoch);
  }

  encode(): LocalDateProto {
    return new LocalDateProto({
      daysSinceEpoch: this.daysSinceEpoch,
    });
  }

  getDaysSinceEpoch(): number {
    return this.daysSinceEpoch;
  }

  getYear(): number {
    return this.internalDate.getUTCFullYear();
  }

  getMonth(): number {
    return this.internalDate.getUTCMonth() + 1;
  }

  getDay(): number {
    return this.internalDate.getUTCDate();
  }

  getDayOfWeek(): number {
    return this.internalDate.getUTCDay(); // 0 = Sunday, 6 = Saturday
  }

  /**
   * Convert to a string in YYYY-MM-DD format
   */
  toString(): string {
    return format(this.internalDate, "yyyy-MM-dd");
  }

  /**
   * Convert to a JavaScript Date object
   */
  toJSDate(): Date {
    return new Date(this.internalDate);
  }

  /**
   * Check if this is a leap year
   */
  isLeapYear(): boolean {
    const year = this.getYear();
    return (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
  }

  /**
   * Get the number of days in the current month
   */
  daysInMonth(): number {
    return getDaysInMonth(this.internalDate);
  }

  /**
   * Compare this date with another Date
   * Returns negative if this is before other, positive if after, 0 if equal
   */
  compareTo(other: LocalDate): number {
    return this.daysSinceEpoch - other.daysSinceEpoch;
  }

  isBefore(other: LocalDate): boolean {
    return isBefore(this.internalDate, other.internalDate);
  }

  isAfter(other: LocalDate): boolean {
    return isAfter(this.internalDate, other.internalDate);
  }

  equals(other: LocalDate): boolean {
    return isEqual(this.internalDate, other.internalDate);
  }

  // ========== Arithmetic Methods ==========

  addDays(days: number): LocalDate {
    const newDate = addDays(this.internalDate, days);
    const daysSinceEpoch = Math.floor(newDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  addWeeks(weeks: number): LocalDate {
    const newDate = addWeeks(this.internalDate, weeks);
    const daysSinceEpoch = Math.floor(newDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  addMonths(months: number): LocalDate {
    const newDate = addMonths(this.internalDate, months);
    const daysSinceEpoch = Math.floor(newDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  addYears(years: number): LocalDate {
    const newDate = addYears(this.internalDate, years);
    const daysSinceEpoch = Math.floor(newDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  /**
   * Calculate the difference in days between this date and another
   */
  daysBetween(other: LocalDate): number {
    return differenceInDays(this.internalDate, other.internalDate);
  }

  /**
   * Get the start of the current month
   */
  startOfMonth(): LocalDate {
    const newDate = startOfMonth(this.internalDate);
    const daysSinceEpoch = Math.floor(newDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }

  /**
   * Get the end of the current month
   */
  endOfMonth(): LocalDate {
    const newDate = endOfMonth(this.internalDate);
    const daysSinceEpoch = Math.floor(newDate.getTime() / 86400000);
    return new LocalDate(daysSinceEpoch);
  }
}
