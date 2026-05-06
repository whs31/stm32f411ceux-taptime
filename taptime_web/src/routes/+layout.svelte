<script lang="ts">
    import "../app.css";
    import { Toaster } from "$lib/components/ui/sonner/index.js";

    let { children } = $props();
</script>

<Toaster visibleToasts={7} duration={5000} />
{@render children()}
