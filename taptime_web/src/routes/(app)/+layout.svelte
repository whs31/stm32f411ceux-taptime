<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import SidebarBlock from "$lib/blocks/SidebarBlock.svelte";
  import SiteHeaderBlock from "$lib/blocks/SiteHeaderBlock.svelte";
  import { userStore } from "$lib/stores/index.js";

  let { children } = $props();

  onMount(async () => {
    if (!localStorage.getItem("jwt")) {
      goto("/login");
      return;
    }
    await userStore.fetch();
    if (!userStore.user) {
      goto("/login");
    }
  });
</script>

<Sidebar.Provider
  style="--sidebar-width: calc(var(--spacing) * 72); --header-height: calc(var(--spacing) * 12);"
>
  <SidebarBlock variant="inset" />
  <Sidebar.Inset>
    <SiteHeaderBlock />
    <div class="flex flex-1 flex-col gap-4 p-4 pt-0 lg:p-6 lg:pt-0">
      {@render children()}
    </div>
  </Sidebar.Inset>
</Sidebar.Provider>
