<script lang="ts">
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import type { ComponentProps } from "svelte";
  import GaugeIcon from "@lucide/svelte/icons/gauge";
  import Logo from "$lib/icons/Logo.svelte";
  import NavMainBlock from "$lib/blocks/sidebar/NavMainBlock.svelte";

  const data = {
    nav: [
      {
        title: "Dashboard",
        url: "#", // todo
        icon: GaugeIcon,
      },
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
    <!-- <NavUser user={data.user} /> -->
  </Sidebar.Footer>
</Sidebar.Root>
