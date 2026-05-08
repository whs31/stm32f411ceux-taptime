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

<div class="grid grid-cols-7 gap-1.5">
  {#each WEEKDAYS as { label, day } (day)}
    <Button
      onclick={() => toggle(day)}
      variant={value.includes(day) ? "default" : "outline"}
      class="rounded-md"
    >
      {label}
    </Button>
  {/each}
</div>
