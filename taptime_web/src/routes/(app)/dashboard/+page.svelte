<script lang="ts">
  import { onMount } from "svelte";
  import * as Card from "$lib/components/ui/card/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Progress } from "$lib/components/ui/progress/index.js";
  import { Separator } from "$lib/components/ui/separator/index.js";
  import { userStore } from "$lib/stores";
  import { StoreService } from "$lib/services";
  import type { Day } from "@taptime/proto/taptime/day_pb.js";

  const tz = $derived(
    userStore.user?.timeZone?.timeZone ??
      Intl.DateTimeFormat().resolvedOptions().timeZone,
  );

  let day = $state<Day | null>(null);
  let loadError = $state<string | null>(null);
  let loading = $state(true);
  let submitting = $state(false);

  let currentTimeDisplay = $state("--:--:--");
  let workSeconds = $state(0);

  const events = $derived(day?.events ?? []);
  const lastEvent = $derived(events.length > 0 ? events[events.length - 1] : null);
  const isCheckedIn = $derived(lastEvent?.eventType.case === "checkIn");
  const firstCheckIn = $derived(
    events.find((e) => e.eventType.case === "checkIn")?.eventType.value,
  );
  const lastCheckOut = $derived(
    [...events].reverse().find((e) => e.eventType.case === "checkOut")
      ?.eventType.value,
  );
  const requiredSeconds = $derived(Number(day?.requiredWorkHours?.seconds ?? 0n));
  const lunchSeconds = $derived(Number(day?.lunchBreakDuration?.seconds ?? 0n));

  function tzTimeParts(): { h: number; m: number; s: number } {
    const parts = new Intl.DateTimeFormat("en-US", {
      timeZone: tz,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hourCycle: "h23",
    }).formatToParts(new Date());
    return {
      h: parseInt(parts.find((p) => p.type === "hour")?.value ?? "0") % 24,
      m: parseInt(parts.find((p) => p.type === "minute")?.value ?? "0"),
      s: parseInt(parts.find((p) => p.type === "second")?.value ?? "0"),
    };
  }

  function ltToSeconds(lt: { hour: number; minute: number; second: number }) {
    return lt.hour * 3600 + lt.minute * 60 + lt.second;
  }

  function pad(n: number) {
    return String(n).padStart(2, "0");
  }

  function formatSeconds(secs: number) {
    const s = Math.max(0, Math.floor(secs));
    return `${pad(Math.floor(s / 3600))}:${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}`;
  }

  function computeWorkSeconds(): number {
    if (isCheckedIn && firstCheckIn) {
      const { h, m, s } = tzTimeParts();
      const nowSecs = h * 3600 + m * 60 + s;
      return Math.max(0, nowSecs - ltToSeconds(firstCheckIn) - lunchSeconds);
    }
    if (!isCheckedIn && firstCheckIn && lastCheckOut) {
      return Math.max(
        0,
        ltToSeconds(lastCheckOut) - ltToSeconds(firstCheckIn) - lunchSeconds,
      );
    }
    return 0;
  }

  function tick() {
    const { h, m, s } = tzTimeParts();
    currentTimeDisplay = `${pad(h)}:${pad(m)}:${pad(s)}`;
    workSeconds = computeWorkSeconds();
  }

  async function loadDay() {
    loading = true;
    loadError = null;
    try {
      day = await StoreService.getDay(StoreService.todayProtoDate(tz));
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function handleCheckInOut() {
    submitting = true;
    try {
      const date = StoreService.todayProtoDate(tz);
      const time = StoreService.nowLocalTime(tz);
      if (isCheckedIn) {
        await StoreService.addCheckOut(date, time);
      } else {
        await StoreService.addCheckIn(date, time);
      }
      await loadDay();
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      submitting = false;
    }
  }

  const progressPercent = $derived(
    requiredSeconds > 0
      ? Math.min(100, (workSeconds / requiredSeconds) * 100)
      : 0,
  );

  const todayLabel = $derived(
    new Intl.DateTimeFormat("en-US", {
      timeZone: tz,
      weekday: "long",
      month: "long",
      day: "numeric",
    }).format(new Date()),
  );

  onMount(() => {
    loadDay();
    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  });
</script>

<div class="py-4 flex flex-col gap-6">
  <div>
    <h2 class="text-2xl font-semibold tracking-tight">Dashboard</h2>
    <p class="mt-1 text-muted-foreground">Overview of your time tracking activity.</p>
  </div>

  <Card.Root class="w-full max-w-sm">
    <Card.Header>
      <Card.Title>Today</Card.Title>
      <Card.Description>{todayLabel}</Card.Description>
    </Card.Header>

    <Card.Content class="flex flex-col gap-5">
      <!-- Current time -->
      <div class="flex flex-col gap-0.5">
        <span class="text-muted-foreground text-xs uppercase tracking-widest">Current time</span>
        <span class="font-mono text-4xl font-semibold tabular-nums tracking-tight">
          {currentTimeDisplay}
        </span>
      </div>

      <Separator />

      {#if loading}
        <div class="text-muted-foreground text-sm">Loading…</div>
      {:else if loadError}
        <div class="text-destructive text-sm">{loadError}</div>
      {:else}
        <!-- Work time -->
        <div class="flex flex-col gap-0.5">
          <span class="text-muted-foreground text-xs uppercase tracking-widest">Work time</span>
          <span
            class="font-mono text-4xl font-semibold tabular-nums tracking-tight transition-colors {isCheckedIn
              ? 'text-primary'
              : ''}"
          >
            {formatSeconds(workSeconds)}
          </span>
        </div>

        <!-- Progress -->
        <div class="flex flex-col gap-2">
          <Progress value={progressPercent} max={100} class="h-1.5" />
          <div class="flex justify-between text-xs tabular-nums text-muted-foreground">
            <span>{formatSeconds(workSeconds)}</span>
            <span>{formatSeconds(requiredSeconds)}</span>
          </div>
        </div>

        <!-- Check in/out -->
        <Button
          onclick={handleCheckInOut}
          disabled={submitting}
          variant={isCheckedIn ? "outline" : "default"}
          class="w-full"
        >
          {#if submitting}
            …
          {:else if isCheckedIn}
            Check Out
          {:else}
            Check In
          {/if}
        </Button>
      {/if}
    </Card.Content>
  </Card.Root>
</div>
