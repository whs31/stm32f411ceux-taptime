<script lang="ts">
  import * as Select from "$lib/components/ui/select/index.js";

  let { value = $bindable() }: { value: string } = $props();

  const TIMEZONES: string[] = Intl.supportedValuesOf("timeZone");

  const TIMEZONE_GROUPS: [string, string[]][] = (() => {
    const groups: Record<string, string[]> = {};
    for (const tz of TIMEZONES) {
      const slash = tz.indexOf("/");
      const region = slash === -1 ? "Other" : tz.slice(0, slash);
      (groups[region] ??= []).push(tz);
    }
    return Object.entries(groups).sort(([a], [b]) => a.localeCompare(b));
  })();

  function tzLabel(tz: string): string {
    const last = tz.lastIndexOf("/");
    return (last === -1 ? tz : tz.slice(last + 1)).replaceAll("_", " ");
  }

  const triggerContent = $derived(
    TIMEZONES.includes(value) ? tzLabel(value) : "Select a timezone",
  );
</script>

<Select.Root type="single" name="timezone" bind:value>
  <Select.Trigger class="w-full">{triggerContent}</Select.Trigger>
  <Select.Content class="max-h-100">
    {#each TIMEZONE_GROUPS as [region, tzs]}
      <Select.Group>
        <Select.Label>{region}</Select.Label>
        {#each tzs as tz}
          <Select.Item value={tz} label={tzLabel(tz)}>
            {tzLabel(tz)}
          </Select.Item>
        {/each}
      </Select.Group>
    {/each}
  </Select.Content>
</Select.Root>
