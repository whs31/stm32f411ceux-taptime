<script lang="ts">
  import { onMount } from "svelte";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { WorkHoursInput } from "$lib/blocks/components";
  import * as Popover from "$lib/components/ui/popover/index.js";
  import { Progress } from "$lib/components/ui/progress/index.js";
  import * as ScrollArea from "$lib/components/ui/scroll-area/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import * as Switch from "$lib/components/ui/switch/index.js";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";
  import {
    balanceContribution,
    balanceLabel as sharedBalanceLabel,
    buildEventListItems,
    buildMonthRhythmModel,
    buildSessions,
    buildSummaryMap,
    chartX,
    chartY,
    computePresenceSeconds,
    computeWorkSeconds,
    currentTimeParts,
    currentTimeValue,
    dateLabel,
    dayKey,
    dayKindLabel,
    durationSeconds,
    formatHours,
    formatSeconds,
    formatTime,
    hasFlag,
    isRegularRequiredDay,
    linePath,
    liveBalanceSeconds as sharedLiveBalanceSeconds,
    mondayFirstDayOfWeek,
    monthEndDay,
    monthLabel,
    monthStartDay,
    nextManualEventType,
    pad,
    parseManualTime,
    protoDate,
    requiredDaySeconds,
    summaryRequiredSeconds,
    summaryClockedSeconds as sharedSummaryClockedSeconds,
    workTargetSeconds,
    type EventListItem,
    type ManualEventType,
  } from "$lib/dashboard";
  import { userStore } from "$lib/stores";
  import { StoreService } from "$lib/services";
  import { Duration } from "@bufbuild/protobuf";
  import CalendarXIcon from "@lucide/svelte/icons/calendar-x";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import LogInIcon from "@lucide/svelte/icons/log-in";
  import LogOutIcon from "@lucide/svelte/icons/log-out";
  import Trash2Icon from "@lucide/svelte/icons/trash-2";
  import { DayFlag } from "@taptime/proto/taptime/day_pb.js";
  import type {
    DashboardResponse,
    DaySummary,
    MonthlyStats,
  } from "@taptime/proto/taptime/services/store_pb.js";

  type CalendarCell = DaySummary | null;
  type ManualTarget = "today" | "selected";
  type ChartMonthOption = {
    value: string;
    monthStart: number;
    label: string;
  };
  type StatItem = {
    label: string;
    value: string;
  };
  type MonthStatsViewModel = {
    balanceSeconds: number;
    items: StatItem[];
  };

  const tz = $derived(
    userStore.user?.timeZone?.timeZone ??
      Intl.DateTimeFormat().resolvedOptions().timeZone,
  );

  let dashboard = $state<DashboardResponse | null>(null);
  let loadError = $state<string | null>(null);
  let loading = $state(true);
  let refreshing = $state(false);
  let submitting = $state(false);
  let flagUpdating = $state(false);
  let overrideSaving = $state(false);
  let deletingEventKey = $state<string | null>(null);
  let manualEventOpen = $state(false);
  let manualSubmitting = $state(false);
  let manualTarget = $state<ManualTarget>("today");
  let manualEventType = $state<ManualEventType>("checkIn");
  let manualTime = $state("");
  let overrideHours = $state(0);
  let overrideMinutes = $state(0);
  let overrideLoadedKey = $state("");
  let chartMonthValue = $state("");
  let loadedWindowKey = $state("");
  let selectedDayKey = $state<number | null>(null);

  let currentTimeDisplay = $state("--:--:--");
  let currentSeconds = $state(0);
  let workSeconds = $state(0);

  const summaries = $derived(dashboard?.days ?? []);
  const summaryByDay = $derived(buildSummaryMap(summaries));
  const todayDays = $derived(StoreService.currentDate(tz).daysSinceEpoch);
  const todaySummary = $derived(summaryByDay.get(todayDays) ?? null);
  const day = $derived(todaySummary?.day ?? null);
  const events = $derived(day?.events ?? []);
  const isCheckedIn = $derived(
    events.length > 0 && events[events.length - 1].eventType.case === "checkIn",
  );
  const hasCheckInToday = $derived(
    events.some((event) => event.eventType.case === "checkIn"),
  );
  const todayIsDayOff = $derived(hasFlag(todaySummary, DayFlag.DAY_OFF));
  const takeDayOffDisabled = $derived(
    loading ||
      submitting ||
      flagUpdating ||
      manualSubmitting ||
      hasCheckInToday ||
      todayIsDayOff,
  );
  const takeDayOffLabel = $derived(todayIsDayOff ? "Day Off Set" : "Take Day Off");
  const requiredSeconds = $derived(requiredDaySeconds(day));
  const presenceSeconds = $derived(computePresenceSeconds(day, currentSeconds));
  const progressPercent = $derived(
    requiredSeconds > 0
      ? Math.min(100, (presenceSeconds / requiredSeconds) * 100)
      : 0,
  );
  const monthStats = $derived(dashboard?.monthStats ?? null);
  const calendarCells = $derived(buildCalendarCells(summaries));
  const selectedSummary = $derived(
    selectedDayKey === null ? null : summaryByDay.get(selectedDayKey) ?? null,
  );
  const todaySessions = $derived(buildSessions(day, currentSeconds));
  const todayEventItems = $derived(buildEventListItems(day));
  const selectedEventItems = $derived(buildEventListItems(selectedSummary?.day));
  const chartMonthOptions = $derived(buildChartMonthOptions(summaries));
  const chartMonthStart = $derived(
    parseChartMonthValue(chartMonthValue, todayDays),
  );
  const monthSummaries = $derived(
    summaries.filter((summary) => {
      const key = dayKey(summary.day);
      return (
        key !== null &&
        key >= chartMonthStart &&
        key <= monthEndDay(chartMonthStart)
      );
    }),
  );
  const chartModel = $derived(buildMonthRhythmModel(monthSummaries, chartMonthStart));
  const selectedManualTargetAvailable = $derived(
    Boolean(
      selectedSummary?.day?.date &&
        selectedSummary.day.date.daysSinceEpoch !== todayDays,
    ),
  );
  const manualTargetSummary = $derived(
    manualTarget === "selected" && selectedManualTargetAvailable
      ? selectedSummary
      : todaySummary,
  );
  const manualTargetDate = $derived(
    manualTarget === "selected" && selectedManualTargetAvailable
      ? selectedSummary?.day?.date
      : (day?.date ?? StoreService.currentDate(tz)),
  );
  const manualTargetLabel = $derived(
    manualTarget === "selected" && selectedManualTargetAvailable
      ? `Selected day (${dateLabel(selectedSummary?.day?.date?.daysSinceEpoch ?? todayDays, true)})`
      : `Today (${dateLabel(todayDays, true)})`,
  );
  const validManualEventType = $derived(
    nextManualEventType(manualTargetSummary?.day?.events ?? []),
  );
  const manualEventTypeLabel = $derived(
    manualEventType === "checkIn" ? "Check In" : "Check Out",
  );
  const parsedManualTime = $derived(parseManualTime(manualTime));
  const manualSubmitDisabled = $derived(
    manualSubmitting ||
      submitting ||
      flagUpdating ||
      deletingEventKey !== null ||
      loading ||
      refreshing ||
      !manualTargetDate ||
      !parsedManualTime ||
      manualEventType !== validManualEventType,
  );
  const selectedWorkTargetSeconds = $derived(workTargetSeconds(selectedSummary));
  const selectedLunchSeconds = $derived(
    durationSeconds(selectedSummary?.day?.lunchBreakDuration),
  );
  const overrideSeconds = $derived(overrideHours * 3600 + overrideMinutes * 60);
  const overrideDirty = $derived(
    Boolean(selectedSummary?.day && overrideSeconds !== selectedWorkTargetSeconds),
  );
  const overrideSaveDisabled = $derived(
    overrideSaving ||
      flagUpdating ||
      manualSubmitting ||
      deletingEventKey !== null ||
      !selectedSummary?.day?.date ||
      overrideSeconds <= 0 ||
      !overrideDirty,
  );
  const overrideClearDisabled = $derived(
    overrideSaving ||
      flagUpdating ||
      manualSubmitting ||
      deletingEventKey !== null ||
      !selectedSummary?.day?.date ||
      !selectedSummary.requiredWorkHoursOverridden,
  );

  const flagControls = [
    { flag: DayFlag.WEEKEND, label: "Weekend" },
    { flag: DayFlag.REMOTE, label: "Remote" },
    { flag: DayFlag.DAY_OFF, label: "Day off" },
    { flag: DayFlag.VACATION, label: "Vacation" },
  ];

  const monthStatsView = $derived(buildMonthStatsView(monthStats, todaySummary));
  const statItems = $derived(monthStatsView.items);

  $effect(() => {
    const window = dashboardWindow(tz);
    const key = [
      window.rangeStart.daysSinceEpoch,
      window.rangeEnd.daysSinceEpoch,
      window.monthStart.daysSinceEpoch,
      window.monthEnd.daysSinceEpoch,
    ].join(":");
    if (key !== loadedWindowKey) {
      loadedWindowKey = key;
      void loadDashboard();
    }
  });

  $effect(() => {
    if (manualTarget === "selected" && !selectedManualTargetAvailable) {
      manualTarget = "today";
    }
    const nextType = nextManualEventType(manualTargetSummary?.day?.events ?? []);
    if (manualEventType !== nextType) {
      manualEventType = nextType;
    }
  });

  $effect(() => {
    if (chartMonthOptions.length === 0) return;
    if (!chartMonthOptions.some((option) => option.value === chartMonthValue)) {
      chartMonthValue = String(monthStartDay(selectedDayKey ?? todayDays));
    }
  });

  $effect(() => {
    const key = [
      selectedSummary?.day?.date?.daysSinceEpoch ?? "none",
      selectedWorkTargetSeconds,
      selectedSummary?.requiredWorkHoursOverridden ? "override" : "default",
    ].join(":");
    if (key !== overrideLoadedKey) {
      overrideLoadedKey = key;
      overrideHours = Math.floor(selectedWorkTargetSeconds / 3600);
      overrideMinutes = Math.floor((selectedWorkTargetSeconds % 3600) / 60);
    }
  });

  function dashboardWindow(timeZone: string) {
    const today = StoreService.currentDate(timeZone);
    const rangeEnd = today;
    const rangeStart = protoDate(today.daysSinceEpoch - 364);
    const monthStart = protoDate(monthStartDay(today.daysSinceEpoch));
    const monthEnd = protoDate(monthEndDay(today.daysSinceEpoch));
    return { rangeStart, rangeEnd, monthStart, monthEnd, today };
  }

  function getTz(): string {
    return (
      userStore.user?.timeZone?.timeZone ??
      Intl.DateTimeFormat().resolvedOptions().timeZone
    );
  }

  function summaryClockedSeconds(summary: DaySummary | null | undefined) {
    return sharedSummaryClockedSeconds(summary, todayDays, workSeconds);
  }

  function manualEventTypeAllowed(type: ManualEventType) {
    return type === validManualEventType;
  }

  function resetManualEventForm(target: ManualTarget = "today") {
    manualTarget =
      target === "selected" && selectedManualTargetAvailable ? "selected" : "today";
    manualEventType = nextManualEventType(
      (manualTarget === "selected" && selectedManualTargetAvailable
        ? selectedSummary
        : todaySummary
      )?.day?.events ?? [],
    );
    manualTime = currentTimeValue(getTz());
  }

  function tick() {
    const { h, m, s } = currentTimeParts(getTz());
    currentSeconds = h * 3600 + m * 60 + s;
    currentTimeDisplay = `${pad(h)}:${pad(m)}:${pad(s)}`;
    workSeconds = computeWorkSeconds(day, currentSeconds);
  }

  async function loadDashboard(background = dashboard !== null) {
    if (background) {
      refreshing = true;
    } else {
      loading = true;
    }
    loadError = null;
    try {
      const window = dashboardWindow(tz);
      const nextDashboard = await StoreService.getDashboard(
        window.rangeStart,
        window.rangeEnd,
        window.monthStart,
        window.monthEnd,
        window.today,
      );
      const nextSummaryByDay = buildSummaryMap(nextDashboard.days);
      dashboard = nextDashboard;
      if (selectedDayKey === null || !nextSummaryByDay.has(selectedDayKey)) {
        selectedDayKey = window.today.daysSinceEpoch;
        chartMonthValue = String(monthStartDay(selectedDayKey));
      }
      tick();
    } catch (e) {
      if (!background) {
        dashboard = null;
      }
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      if (background) {
        refreshing = false;
      } else {
        loading = false;
      }
    }
  }

  async function handleCheckInOut() {
    submitting = true;
    try {
      const date = StoreService.currentDate(tz);
      const time = StoreService.currentTime(tz);
      if (isCheckedIn) {
        await StoreService.addCheckOut(date, time);
      } else {
        await StoreService.addCheckIn(date, time);
      }
      await loadDashboard(true);
      tick();
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      submitting = false;
    }
  }

  async function handleManualEventSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (manualSubmitDisabled || !manualTargetDate || !parsedManualTime) return;
    manualSubmitting = true;
    try {
      if (manualEventType === "checkIn") {
        await StoreService.addCheckIn(manualTargetDate, parsedManualTime);
      } else {
        await StoreService.addCheckOut(manualTargetDate, parsedManualTime);
      }
      if (manualTarget === "selected") {
        selectedDayKey = manualTargetDate.daysSinceEpoch;
      }
      manualEventOpen = false;
      await loadDashboard(true);
      tick();
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      manualSubmitting = false;
    }
  }

  async function deleteEvent(item: EventListItem) {
    if (!item.id || deletingEventKey !== null) return;
    deletingEventKey = item.key;
    try {
      await StoreService.deleteEvent(item.id);
      await loadDashboard(true);
      tick();
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      deletingEventKey = null;
    }
  }

  async function handleTakeDayOff() {
    if (takeDayOffDisabled) return;
    const date = day?.date ?? StoreService.currentDate(tz);
    flagUpdating = true;
    try {
      await StoreService.setFlag(date, DayFlag.DAY_OFF);
      selectedDayKey = date.daysSinceEpoch;
      await loadDashboard(true);
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      flagUpdating = false;
    }
  }

  async function toggleSelectedFlag(flag: DayFlag) {
    const selected = selectedSummary?.day;
    if (!selected?.date) return;
    flagUpdating = true;
    try {
      await StoreService.setFlag(selected.date, flag);
      await loadDashboard(true);
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      flagUpdating = false;
    }
  }

  async function saveRequiredOverride() {
    const selected = selectedSummary?.day;
    if (overrideSaveDisabled || !selected?.date) return;
    overrideSaving = true;
    try {
      await StoreService.setRequiredWorkHoursOverride(
        selected.date,
        new Duration({ seconds: BigInt(overrideSeconds) }),
      );
      selectedDayKey = selected.date.daysSinceEpoch;
      await loadDashboard(true);
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      overrideSaving = false;
    }
  }

  async function clearRequiredOverride() {
    const selected = selectedSummary?.day;
    if (overrideClearDisabled || !selected?.date) return;
    overrideSaving = true;
    try {
      await StoreService.setRequiredWorkHoursOverride(selected.date, null);
      selectedDayKey = selected.date.daysSinceEpoch;
      await loadDashboard(true);
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      overrideSaving = false;
    }
  }

  function liveBalanceSeconds(summary: DaySummary | null | undefined) {
    return sharedLiveBalanceSeconds(summary, todayDays, currentSeconds);
  }

  function balanceLabel(summary: DaySummary | null | undefined) {
    return sharedBalanceLabel(summary, todayDays, workSeconds);
  }

  function activityTooltip(summary: DaySummary) {
    return `${dateLabel(summary.day?.date?.daysSinceEpoch ?? 0)} - ${dayKindLabel(summary)} - ${balanceLabel(summary)}`;
  }

  function selectActivityDay(date: number | null | undefined) {
    if (date === null || date === undefined) {
      selectedDayKey = null;
      return;
    }
    selectedDayKey = date;
    chartMonthValue = String(monthStartDay(date));
  }

  function takeDayOffTooltip() {
    if (hasCheckInToday) return "Unavailable after check-in";
    if (todayIsDayOff) return "Today is already marked as a day off";
    return "Mark today as a day off";
  }

  function buildCalendarCells(items: DaySummary[]): CalendarCell[] {
    if (items.length === 0) return [];
    const cells: CalendarCell[] = [];
    const first = dayKey(items[0].day);
    if (first !== null) {
      for (let i = 0; i < mondayFirstDayOfWeek(first); i += 1) {
        cells.push(null);
      }
    }
    cells.push(...items);
    return cells;
  }

  function buildChartMonthOptions(items: DaySummary[]): ChartMonthOption[] {
    const months = new Map<number, ChartMonthOption>();
    for (const summary of items) {
      const key = dayKey(summary.day);
      if (key === null) continue;
      const monthStart = monthStartDay(key);
      if (!months.has(monthStart)) {
        months.set(monthStart, {
          value: String(monthStart),
          monthStart,
          label: monthLabel(monthStart),
        });
      }
    }
    return [...months.values()].sort((a, b) => b.monthStart - a.monthStart);
  }

  function parseChartMonthValue(value: string, fallbackDay: number) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : monthStartDay(fallbackDay);
  }

  function themeMix(token: string, amount: number, base = "var(--card)") {
    return `color-mix(in oklch, var(${token}) ${amount.toFixed(1)}%, ${base})`;
  }

  function activityCell(color: string) {
    return [
      `--activity-cell-bg: ${color}`,
      "background-color: var(--activity-cell-bg)",
      "border-color: color-mix(in oklch, var(--activity-cell-bg) 45%, var(--border))",
    ].join("; ");
  }

  function calendarCellStyle(summary: DaySummary | null) {
    if (!summary?.day) return "background-color: transparent;";
    if (summary.beforeStartDate) {
      return activityCell(themeMix("--muted", 76));
    }
    if (hasFlag(summary, DayFlag.DAY_OFF)) {
      return activityCell(themeMix("--muted-foreground", 28));
    }
    if (hasFlag(summary, DayFlag.VACATION)) {
      return activityCell(themeMix("--primary", 48));
    }
    if (hasFlag(summary, DayFlag.REMOTE)) {
      return activityCell(themeMix("--activity-remote", 82));
    }
    if (hasFlag(summary, DayFlag.WEEKEND)) {
      return summary.fullDayWorked
        ? activityCell(themeMix("--activity-weekend-work", 84))
        : activityCell(themeMix("--muted", 78));
    }
    if (summary.skipped) {
      return activityCell(themeMix("--destructive", 76));
    }

    const delta = liveBalanceSeconds(summary);
    const required = Math.max(1, requiredDaySeconds(summary.day));
    if (delta < 0) {
      const amount = Math.min(1, Math.abs(delta) / required);
      return activityCell(themeMix("--destructive", 42 + amount * 38));
    }

    const ratio = Math.min(1, delta / required);
    const intensity = 48 + Math.sqrt(Math.sqrt(ratio)) * 42;
    return activityCell(themeMix("--activity-overtime", intensity));
  }

  function calendarDots(summary: DaySummary | null) {
    if (!summary?.day) return [];
    const dots: string[] = [];
    const clocked = summaryClockedSeconds(summary);
    if (hasFlag(summary, DayFlag.VACATION) && summary.fullDayWorked) {
      dots.push("var(--activity-weekend-work)");
    } else if (
      (hasFlag(summary, DayFlag.REMOTE) || hasFlag(summary, DayFlag.DAY_OFF)) &&
      clocked > 0
    ) {
      dots.push("var(--chart-1)");
    }
    return dots;
  }

  function signedHours(seconds: number) {
    return seconds > 0 ? `+${formatHours(seconds)}` : formatHours(seconds);
  }

  function buildMonthStatsView(
    stats: MonthlyStats | null,
    today: DaySummary | null,
  ): MonthStatsViewModel {
    const closedToday = today?.beforeStartDate
      ? 0
      : durationSeconds(today?.clockedWork);
    const liveToday = today?.beforeStartDate ? 0 : summaryClockedSeconds(today);
    const liveDelta = Math.max(0, liveToday - closedToday);
    let totalClocked = durationSeconds(stats?.totalClockedWork) + liveDelta;
    let overtime = durationSeconds(stats?.overtime);
    let undertime = durationSeconds(stats?.undertime);

    if (today?.day && isRegularRequiredDay(today.day)) {
      const required = requiredDaySeconds(today.day);
      const closed = balanceContribution(computePresenceSeconds(today.day), required);
      const live = balanceContribution(
        computePresenceSeconds(today.day, currentSeconds),
        required,
      );
      overtime += live.overtime - closed.overtime;
      undertime += live.undertime - closed.undertime;
    }

    return {
      balanceSeconds: overtime - undertime,
      items: [
        {
          label: "Clocked",
          value: formatHours(totalClocked),
        },
        { label: "Overtime", value: formatHours(overtime) },
        {
          label: "Undertime",
          value: formatHours(undertime),
        },
        { label: "Worked days", value: String(stats?.workedDays ?? 0) },
        { label: "Remote", value: String(stats?.remoteDays ?? 0) },
        { label: "Day off", value: String(stats?.dayOffs ?? 0) },
        { label: "Vacation", value: String(stats?.vacationDays ?? 0) },
        { label: "Skipped", value: String(stats?.skippedDays ?? 0) },
        {
          label: "Weekend work",
          value: String(stats?.fullWeekendWorkDays ?? 0),
        },
        {
          label: "Vacation work",
          value: String(stats?.fullVacationWorkDays ?? 0),
        },
      ],
    };
  }

  function selectedHasFlag(flag: DayFlag) {
    return hasFlag(selectedSummary, flag);
  }

  function selectedFlagDisabled(flag: DayFlag) {
    if (flag !== DayFlag.DAY_OFF || !selectedSummary?.day) return false;
    return (
      dayKey(selectedSummary.day) === todayDays &&
      selectedSummary.day.events.some(
        (event) => event.eventType.case === "checkIn",
      )
    );
  }

  onMount(() => {
    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  });
</script>

<div class="flex flex-col gap-4 py-3" aria-busy={loading || refreshing}>
  <div class="flex flex-col gap-0.5">
    <h2 class="text-xl font-semibold">Dashboard</h2>
    <p class="text-muted-foreground text-sm">
      Today, rolling activity, and month-to-date work.
    </p>
  </div>

  {#if loadError}
    <div class="border-destructive/30 bg-destructive/10 text-destructive rounded-md border px-3 py-2 text-sm">
      {loadError}
    </div>
  {/if}

  <div class="grid items-stretch gap-4 xl:grid-cols-[minmax(0,1fr)_340px]">
    <div class="flex h-full min-w-0 flex-col gap-4">
      <Card.Root size="sm">
        <Card.Header class="pb-3">
          <Card.Title>Today</Card.Title>
          <Card.Description>{dateLabel(todayDays)}</Card.Description>
        </Card.Header>
        <Card.Content class="grid gap-4 md:grid-cols-[minmax(0,1fr)_240px]">
        <div class="flex flex-col gap-4">
          <div class="grid gap-4 sm:grid-cols-2">
            <div class="flex flex-col gap-1">
              <span class="text-muted-foreground text-xs uppercase">Current time</span>
              <span class="font-mono text-3xl font-semibold tabular-nums lg:text-4xl">
                {currentTimeDisplay}
              </span>
            </div>
            <div class="flex flex-col gap-1">
              <span class="text-muted-foreground text-xs uppercase">Work time</span>
              <span
                class="font-mono text-3xl font-semibold tabular-nums transition-colors lg:text-4xl {isCheckedIn
                  ? 'text-primary'
                  : ''}"
              >
                {formatSeconds(workSeconds)}
              </span>
            </div>
          </div>

          <div class="flex flex-col gap-2">
            <Progress value={progressPercent} max={100} class="h-1.5" />
            <div class="flex justify-between text-xs tabular-nums text-muted-foreground">
              <span>{formatSeconds(presenceSeconds)}</span>
              <span>{formatSeconds(requiredSeconds)}</span>
            </div>
          </div>

          <div class="flex flex-col gap-2 sm:flex-row sm:flex-wrap">
            <Button
              onclick={handleCheckInOut}
              disabled={submitting || loading || manualSubmitting || deletingEventKey !== null}
              variant={isCheckedIn ? "outline" : "default"}
              class="w-full sm:w-36"
            >
              {#if isCheckedIn}
                <LogOutIcon />
              {:else}
                <LogInIcon />
              {/if}
              {#if submitting}
                ...
              {:else if isCheckedIn}
                Check Out
              {:else}
                Check In
              {/if}
            </Button>
            <Tooltip.Root>
              <Tooltip.Trigger>
                {#snippet child({ props })}
                  <Button
                    {...props}
                    onclick={handleTakeDayOff}
                    disabled={takeDayOffDisabled}
                    variant="secondary"
                    class="w-full sm:w-36"
                  >
                    <CalendarXIcon />
                    {takeDayOffLabel}
                  </Button>
                {/snippet}
              </Tooltip.Trigger>
              <Tooltip.Content sideOffset={6}>
                {takeDayOffTooltip()}
              </Tooltip.Content>
            </Tooltip.Root>
            <Popover.Root bind:open={manualEventOpen}>
              <Popover.Trigger>
                {#snippet child({ props })}
                  <Button
                    {...props}
                    onclick={() => {
                      resetManualEventForm("today");
                      manualEventOpen = true;
                    }}
                    disabled={
                      loading ||
                      submitting ||
                      flagUpdating ||
                      manualSubmitting ||
                      deletingEventKey !== null
                    }
                    variant="secondary"
                    class="w-full sm:w-36"
                  >
                    <PlusIcon />
                    Add Event
                  </Button>
                {/snippet}
              </Popover.Trigger>
              <Popover.Content align="start" sideOffset={8} class="w-80">
                <form class="flex flex-col gap-4" onsubmit={handleManualEventSubmit}>
                  <div class="flex flex-col gap-1">
                    <div class="font-medium">Add Event</div>
                    <div class="text-muted-foreground text-xs">
                      Add a manual check-in or checkout with minute precision.
                    </div>
                  </div>

                  <div class="grid gap-2">
                    <Label for="manual-event-target">Day</Label>
                    <Select.Root
                      type="single"
                      name="manual-event-target"
                      value={manualTarget}
                      onValueChange={(value) => {
                        if (value === "today" || value === "selected") {
                          manualTarget = value;
                        }
                      }}
                    >
                      <Select.Trigger id="manual-event-target" class="w-full">
                        {manualTargetLabel}
                      </Select.Trigger>
                      <Select.Content>
                        <Select.Item value="today" label={`Today (${dateLabel(todayDays, true)})`}>
                          Today ({dateLabel(todayDays, true)})
                        </Select.Item>
                        {#if selectedManualTargetAvailable}
                          <Select.Item
                            value="selected"
                            label={`Selected day (${dateLabel(selectedSummary?.day?.date?.daysSinceEpoch ?? todayDays, true)})`}
                          >
                            Selected day ({dateLabel(
                              selectedSummary?.day?.date?.daysSinceEpoch ??
                                todayDays,
                              true,
                            )})
                          </Select.Item>
                        {/if}
                      </Select.Content>
                    </Select.Root>
                  </div>

                  <div class="grid gap-2">
                    <Label for="manual-event-type">Event</Label>
                    <Select.Root
                      type="single"
                      name="manual-event-type"
                      value={manualEventType}
                      onValueChange={(value) => {
                        if (value === "checkIn" || value === "checkOut") {
                          manualEventType = value;
                        }
                      }}
                    >
                      <Select.Trigger id="manual-event-type" class="w-full">
                        {manualEventTypeLabel}
                      </Select.Trigger>
                      <Select.Content>
                        <Select.Item
                          value="checkIn"
                          label="Check In"
                          disabled={!manualEventTypeAllowed("checkIn")}
                        >
                          Check In
                        </Select.Item>
                        <Select.Item
                          value="checkOut"
                          label="Check Out"
                          disabled={!manualEventTypeAllowed("checkOut")}
                        >
                          Check Out
                        </Select.Item>
                      </Select.Content>
                    </Select.Root>
                  </div>

                  <div class="grid gap-2">
                    <Label for="manual-event-time">Time</Label>
                    <Input
                      id="manual-event-time"
                      type="time"
                      step="60"
                      bind:value={manualTime}
                      aria-invalid={!parsedManualTime}
                    />
                  </div>

                  <div class="flex justify-end gap-2">
                    <Button
                      type="button"
                      variant="ghost"
                      onclick={() => (manualEventOpen = false)}
                    >
                      Cancel
                    </Button>
                    <Button type="submit" disabled={manualSubmitDisabled}>
                      {manualSubmitting ? "Adding..." : `Add ${manualEventTypeLabel}`}
                    </Button>
                  </div>
                </form>
              </Popover.Content>
            </Popover.Root>
          </div>
        </div>

        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between gap-3">
            <span class="text-sm font-medium">Today history</span>
            <span class="text-muted-foreground text-xs">{todaySessions.length} sessions</span>
          </div>
          <Separator />
          <ScrollArea.Root class="h-28 pr-3">
            {#if loading}
              <div class="text-muted-foreground text-sm">Loading...</div>
            {:else if todayEventItems.length === 0}
              <div class="text-muted-foreground text-sm">No check-ins yet.</div>
            {:else}
              <div class="flex flex-col gap-2">
                {#each todayEventItems as item (item.key)}
                  <div class="grid grid-cols-[24px_1fr_auto_auto] items-center gap-3 text-sm">
                    <span class="text-muted-foreground tabular-nums">{item.index + 1}</span>
                    <span>{item.label}</span>
                    <span class="font-mono tabular-nums">{formatTime(item.time)}</span>
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      aria-label={`Delete ${item.label} at ${formatTime(item.time)}`}
                      title={`Delete ${item.label} at ${formatTime(item.time)}`}
                      disabled={
                        !item.id ||
                        deletingEventKey !== null ||
                        manualSubmitting ||
                        submitting
                      }
                      onclick={() => deleteEvent(item)}
                    >
                      <Trash2Icon />
                    </Button>
                  </div>
                {/each}
              </div>
            {/if}
          </ScrollArea.Root>
        </div>
        </Card.Content>
      </Card.Root>

      <Card.Root size="sm" class="flex-1">
        <Card.Header class="items-center pb-3 text-center">
          <Card.Title>Activity</Card.Title>
          <Card.Description>Rolling year ending today</Card.Description>
        </Card.Header>
        <Card.Content class="flex flex-1 items-center overflow-x-auto pb-3">
          {#if loading}
            <div class="text-muted-foreground text-sm">Loading...</div>
          {:else}
            <div class="mx-auto grid w-max grid-cols-[32px_max-content] gap-2">
            <div class="grid grid-rows-7 gap-[3px] text-[10px] text-muted-foreground">
              <span>Mon</span>
              <span></span>
              <span>Wed</span>
              <span></span>
              <span>Fri</span>
              <span></span>
              <span></span>
            </div>
            <div
              class="grid grid-flow-col grid-rows-7 auto-cols-[13px] gap-[3px]"
              aria-label="Rolling year activity calendar"
            >
              {#each calendarCells as summary}
                {#if summary?.day}
                  <Tooltip.Root>
                    <Tooltip.Trigger>
                      {#snippet child({ props })}
                        <button
                          {...props}
                          type="button"
                          class="relative size-[13px] rounded-[3px] border border-border/40 outline-none transition-colors hover:border-ring/70 focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-card {selectedDayKey ===
                          summary.day?.date?.daysSinceEpoch
                            ? 'shadow-[inset_0_0_0_2px_var(--ring)]'
                            : ''}"
                          style={calendarCellStyle(summary)}
                          onclick={() =>
                            selectActivityDay(summary.day?.date?.daysSinceEpoch)}
                        >
                          <span class="sr-only">
                            {dateLabel(summary.day?.date?.daysSinceEpoch ?? 0)}
                          </span>
                          {#each calendarDots(summary) as color}
                            <span
                              class="absolute bottom-[1px] right-[1px] size-[4px] rounded-full border border-background"
                              style={`background-color: ${color};`}
                            ></span>
                          {/each}
                        </button>
                      {/snippet}
                    </Tooltip.Trigger>
                    <Tooltip.Content sideOffset={6}>
                      {activityTooltip(summary)}
                    </Tooltip.Content>
                  </Tooltip.Root>
                {:else}
                  <span class="size-[13px]"></span>
                {/if}
              {/each}
            </div>
            </div>
          {/if}
        </Card.Content>
      </Card.Root>
    </div>

    <Card.Root size="sm" class="h-full">
      <Card.Header class="pb-3">
        <Card.Title>Selected Day</Card.Title>
        <Card.Description>
          {selectedSummary?.day?.date
            ? dateLabel(selectedSummary.day.date.daysSinceEpoch)
            : "Pick a day"}
        </Card.Description>
      </Card.Header>
      <Card.Content class="flex flex-col gap-3">
        <div class="grid grid-cols-2 gap-2 text-sm">
          <div>
            <div class="text-muted-foreground text-xs uppercase">Status</div>
            <div class="font-medium">{dayKindLabel(selectedSummary)}</div>
          </div>
          <div>
            <div class="text-muted-foreground text-xs uppercase">Balance</div>
            <div class="font-medium">{balanceLabel(selectedSummary)}</div>
          </div>
          <div>
            <div class="text-muted-foreground text-xs uppercase">Clocked</div>
            <div class="font-mono">{formatSeconds(summaryClockedSeconds(selectedSummary))}</div>
          </div>
          <div>
            <div class="text-muted-foreground text-xs uppercase">Required</div>
            <div class="font-mono">
              {formatSeconds(summaryRequiredSeconds(selectedSummary))}
            </div>
          </div>
        </div>

        <Separator />

        <div class="flex flex-col gap-2">
          <div class="flex items-start justify-between gap-3">
            <div>
              <div class="text-sm font-medium">Required Work Override</div>
              <div class="text-muted-foreground text-xs">
                {formatHours(overrideSeconds)} work + {formatHours(selectedLunchSeconds)} lunch = {formatHours(overrideSeconds + selectedLunchSeconds)} target
              </div>
            </div>
            {#if selectedSummary?.requiredWorkHoursOverridden}
              <span class="rounded-sm bg-primary/10 px-1.5 py-0.5 text-xs text-primary">
                Override
              </span>
            {/if}
          </div>
          <WorkHoursInput bind:hours={overrideHours} bind:minutes={overrideMinutes} />
          <div class="flex justify-end gap-2">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onclick={clearRequiredOverride}
              disabled={overrideClearDisabled}
            >
              Clear
            </Button>
            <Button
              type="button"
              size="sm"
              onclick={saveRequiredOverride}
              disabled={overrideSaveDisabled}
            >
              {overrideSaving ? "Saving..." : "Save Target"}
            </Button>
          </div>
        </div>

        <Separator />

        <div class="flex flex-col gap-2">
          {#each flagControls as control}
            <div class="flex items-center justify-between gap-3">
              <span class="text-sm">{control.label}</span>
              <Switch.Root
                checked={selectedHasFlag(control.flag)}
                disabled={
                  flagUpdating ||
                  deletingEventKey !== null ||
                  !selectedSummary ||
                  selectedFlagDisabled(control.flag)
                }
                onclick={() => toggleSelectedFlag(control.flag)}
              />
            </div>
          {/each}
        </div>

        <Separator />

        <div class="flex flex-col gap-2">
          <div class="flex items-center justify-between gap-3">
            <div>
              <div class="text-sm font-medium">Events</div>
              <div class="text-muted-foreground text-xs">
                {selectedEventItems.length} taps
              </div>
            </div>
          </div>
          <ScrollArea.Root class="h-40 pr-3">
            {#if selectedEventItems.length === 0}
              <div class="text-muted-foreground text-sm">No events.</div>
            {:else}
              <div class="flex flex-col gap-2">
                {#each selectedEventItems as item (item.key)}
                  <div class="grid grid-cols-[24px_1fr_auto_auto] items-center gap-3 text-sm">
                    <span class="text-muted-foreground tabular-nums">{item.index + 1}</span>
                    <span>{item.label}</span>
                    <span class="font-mono tabular-nums">{formatTime(item.time)}</span>
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      aria-label={`Delete ${item.label} at ${formatTime(item.time)}`}
                      title={`Delete ${item.label} at ${formatTime(item.time)}`}
                      disabled={
                        !item.id ||
                        deletingEventKey !== null ||
                        manualSubmitting ||
                        submitting
                      }
                      onclick={() => deleteEvent(item)}
                    >
                      <Trash2Icon />
                    </Button>
                  </div>
                {/each}
              </div>
            {/if}
          </ScrollArea.Root>
        </div>
      </Card.Content>
    </Card.Root>
  </div>

  <div class="grid items-stretch gap-4 xl:grid-cols-[minmax(0,1fr)_minmax(420px,1fr)]">
    <Card.Root size="sm" class="h-full">
      <Card.Header class="pb-3">
        <Card.Title>{monthLabel(todayDays)}</Card.Title>
        <Card.Description>Month-to-date stats</Card.Description>
      </Card.Header>
      <Card.Content>
        <div class="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-5">
          <div class="col-span-full rounded-md border bg-muted/30 px-2.5 py-2 text-center">
            <div class="text-muted-foreground text-xs uppercase">Balance</div>
            <div
              class="mt-1 font-mono text-lg font-semibold tabular-nums"
              class:text-destructive={monthStatsView.balanceSeconds < 0}
              class:text-chart-1={monthStatsView.balanceSeconds > 0}
            >
              {signedHours(monthStatsView.balanceSeconds)}
            </div>
          </div>
          {#each statItems as item}
            <div class="rounded-md border bg-muted/30 px-2.5 py-2">
              <div class="text-muted-foreground text-xs uppercase">{item.label}</div>
              <div class="mt-1 font-mono text-lg font-semibold tabular-nums">
                {item.value}
              </div>
            </div>
          {/each}
        </div>
      </Card.Content>
    </Card.Root>

    <Card.Root size="sm" class="h-full">
      <Card.Header class="gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div class="space-y-1">
          <Card.Title>Month Rhythm</Card.Title>
          <Card.Description>{monthLabel(chartMonthStart)} check-ins, checkouts, and taps</Card.Description>
        </div>
        <Select.Root
          type="single"
          name="dashboard-chart-month"
          value={chartMonthValue}
          onValueChange={(value) => {
            if (value) chartMonthValue = value;
          }}
        >
          <Select.Trigger id="dashboard-chart-month" class="w-full sm:w-44">
            {monthLabel(chartMonthStart)}
          </Select.Trigger>
          <Select.Content>
            {#each chartMonthOptions as option (option.value)}
              <Select.Item value={option.value} label={option.label}>
                {option.label}
              </Select.Item>
            {/each}
          </Select.Content>
        </Select.Root>
      </Card.Header>
      <Card.Content>
        <div class="w-full overflow-x-auto">
          <svg
            viewBox="0 0 720 240"
            class="h-[220px] min-w-[620px] w-full text-xs"
            role="img"
            aria-label="Month check-in and checkout chart"
          >
            <g class="text-muted-foreground">
              {#each [0, 6, 12, 18, 24] as hour}
                <line
                  x1="46"
                  x2="672"
                  y1={chartY(hour * 3600)}
                  y2={chartY(hour * 3600)}
                  stroke="currentColor"
                  stroke-opacity="0.16"
                />
                <text x="8" y={chartY(hour * 3600) + 4} fill="currentColor">
                  {pad(hour)}:00
                </text>
              {/each}
              {#each [1, 8, 15, 22, chartModel.daysInMonth] as dayNumber}
                <line
                  x1={chartX(dayNumber, chartModel.daysInMonth)}
                  x2={chartX(dayNumber, chartModel.daysInMonth)}
                  y1="16"
                  y2="200"
                  stroke="currentColor"
                  stroke-opacity="0.1"
                />
                <text
                  x={chartX(dayNumber, chartModel.daysInMonth)}
                  y="224"
                  text-anchor="middle"
                  fill="currentColor"
                >
                  {dayNumber}
                </text>
              {/each}
            </g>

            <path
              d={linePath(chartModel.firstLine, chartModel.daysInMonth)}
              fill="none"
              stroke="var(--chart-1)"
              stroke-width="2"
            />
            <path
              d={linePath(chartModel.lastLine, chartModel.daysInMonth)}
              fill="none"
              stroke="var(--destructive)"
              stroke-width="2"
            />

            {#each chartModel.points as point}
              <circle
                cx={chartX(point.day, chartModel.daysInMonth)}
                cy={chartY(point.seconds)}
                r={point.kind === "checkIn" ? 3 : 3.5}
                fill={point.kind === "checkIn"
                  ? "var(--chart-1)"
                  : "var(--destructive)"}
              >
                <title>{point.label}</title>
              </circle>
            {/each}
            {#if chartModel.points.length === 0}
              <text
                x="359"
                y="112"
                text-anchor="middle"
                fill="currentColor"
                class="text-muted-foreground"
              >
                No check-in or checkout events in {monthLabel(chartMonthStart)}
              </text>
            {/if}
          </svg>
        </div>
        <div class="mt-3 flex flex-wrap gap-4 text-xs text-muted-foreground">
          <span class="inline-flex items-center gap-2">
            <span
              class="size-2 rounded-full"
              style="background-color: var(--chart-1);"
            ></span>
            First check-in
          </span>
          <span class="inline-flex items-center gap-2">
            <span
              class="size-2 rounded-full"
              style="background-color: var(--destructive);"
            ></span>
            Last checkout and checkouts
          </span>
        </div>
      </Card.Content>
    </Card.Root>
  </div>
</div>
