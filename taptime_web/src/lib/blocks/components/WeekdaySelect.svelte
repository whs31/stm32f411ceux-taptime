<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Weekday } from "@taptime/proto/taptime/weekday_pb.js";

  let { value = $bindable() }: { value: number } = $props();

  const values = [
    { value: 0, label: "No lunch break" },
    { value: 15, label: "15 minutes" },
    { value: 30, label: "30 minutes" },
    { value: 45, label: "45 minutes" },
    { value: 60, label: "1 hour" },
  ];

  const triggerContent = $derived(
    values.find((f) => f.value === value)?.label ?? "Select a lunch break time",
  );
</script>

<Select.Root type="single" name="lunchBreak" bind:value>
  <Select.Trigger class="w-full">{triggerContent}</Select.Trigger>
  <Select.Content class="max-h-100">
    {#each values as v (v.value)}
      <Select.Item value={v.value} label={v.label}>
        {v.label}
      </Select.Item>
    {/each}
  </Select.Content>
</Select.Root>
