<script lang="ts">
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import type { ComponentProps } from "svelte";
  import GaugeIcon from "@lucide/svelte/icons/gauge";
  import HistoryIcon from "@lucide/svelte/icons/history";
  import BarChart3Icon from "@lucide/svelte/icons/bar-chart-3";
  import SettingsIcon from "@lucide/svelte/icons/settings";
  import UserIcon from "@lucide/svelte/icons/user";
  import Logo from "$lib/icons/Logo.svelte";
  import NavMainBlock from "$lib/blocks/sidebar/NavMainBlock.svelte";
  import NavUserBlock from "$lib/blocks/sidebar/NavUserBlock.svelte";

  const data = {
    nav: [
      { title: "Dashboard", url: "/dashboard", icon: GaugeIcon },
      { title: "History", url: "/history", icon: HistoryIcon },
      { title: "Reports", url: "/reports", icon: BarChart3Icon },
      { title: "Settings", url: "/settings", icon: SettingsIcon },
      { title: "Profile", url: "/profile", icon: UserIcon },
    ],
  };

  let { ...restProps }: ComponentProps<typeof Sidebar.Root> = $props();
</script>

<Sidebar.Root collapsible="offcanvas" {...restProps}>
  <Sidebar.Header>
    <Sidebar.Menu>
      <Sidebar.MenuItem>
        <Sidebar.MenuButton class="data-[slot=sidebar-menu-button]:p-1.5!">
          {#snippet child({ props })}
            <a href="##" {...props}>
              <Logo class="size-5!" />
              <span class="text-base font-semibold">TapTime</span>
            </a>
          {/snippet}
        </Sidebar.MenuButton>
      </Sidebar.MenuItem>
    </Sidebar.Menu>
  </Sidebar.Header>
  <Sidebar.Content>
    <NavMainBlock items={data.nav} />
    <!-- <NavSecondary items={data.navSecondary} class="mt-auto" /> -->
  </Sidebar.Content>
  <Sidebar.Footer>
    <NavUserBlock />
  </Sidebar.Footer>
</Sidebar.Root>
