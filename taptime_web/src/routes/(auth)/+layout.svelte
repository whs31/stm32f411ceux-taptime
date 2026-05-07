<script lang="ts">
  let { children } = $props();
</script>

<div class="flex min-h-svh items-center justify-center bg-muted/40 p-4">
  {@render children()}
</div>
