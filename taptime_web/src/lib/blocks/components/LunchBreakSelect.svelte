<script lang="ts">
  import * as Select from "$lib/components/ui/select/index.js";

  let { value = $bindable() }: { value: number } = $props();

  const LUNCH_BREAK_DURATIONS = [
    { value: 0, label: "No lunch break" },
    { value: 15, label: "15 minutes" },
    { value: 30, label: "30 minutes" },
    { value: 45, label: "45 minutes" },
    { value: 60, label: "1 hour" },
  ];

  const triggerContent = $derived(
    LUNCH_BREAK_DURATIONS.find((f) => f.value === value)?.label ??
      "Select a lunch break time",
  );
</script>

<Select.Root type="single" name="lunchBreak" bind:value>
  <Select.Trigger class="w-full">{triggerContent}</Select.Trigger>
  <Select.Content class="max-h-100">
    {#each LUNCH_BREAK_DURATIONS as v (v.value)}
      <Select.Item value={v.value} label={v.label}>
        {v.label}
      </Select.Item>
    {/each}
  </Select.Content>
</Select.Root>
