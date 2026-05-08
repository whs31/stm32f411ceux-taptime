<script lang="ts">
  import { cn } from "$lib/utils";
  import { Weekday } from "@taptime/proto/taptime/weekday_pb.js";
  import { Button } from "$lib/components/ui/button/index.js";

  let { value = $bindable<Weekday[]>([]) }: { value: Weekday[] } = $props();

  const WEEKDAYS: { label: string; day: Weekday }[] = [
    { label: "MON", day: Weekday.MONDAY },
    { label: "TUE", day: Weekday.TUESDAY },
    { label: "WED", day: Weekday.WEDNESDAY },
    { label: "THU", day: Weekday.THURSDAY },
    { label: "FRI", day: Weekday.FRIDAY },
    { label: "SAT", day: Weekday.SATURDAY },
    { label: "SUN", day: Weekday.SUNDAY },
  ];

  function toggle(day: Weekday) {
    value = value.includes(day)
      ? value.filter((d) => d !== day)
      : [...value, day];
  }
</script>

<div class="flex gap-1.5">
  {#each WEEKDAYS as { label, day } (day)}
    <button
      type="button"
      onclick={() => toggle(day)}
      class={cn(
        "h-9 w-10 rounded-md text-xs font-medium transition-colors",
        value.includes(day)
          ? "bg-primary text-primary-foreground"
          : "bg-muted text-muted-foreground hover:bg-muted/70",
      )}
    >
      {label}
    </button>
  {/each}
</div>
