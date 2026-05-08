<script lang="ts">
  import { Input } from "$lib/components/ui/input/index.js";

  let {
    hours = $bindable(0),
    minutes = $bindable(0),
  }: { hours: number; minutes: number } = $props();
</script>

<div class="flex items-center gap-2">
  <Input
    type="number"
    min="0"
    max="23"
    bind:value={hours}
    class="w-20 text-center"
  />
  <span class="text-muted-foreground text-sm">h</span>
  <Input
    type="number"
    min="0"
    max="59"
    step="5"
    bind:value={minutes}
    class="w-20 text-center"
  />
  <span class="text-muted-foreground text-sm">min</span>
</div>
