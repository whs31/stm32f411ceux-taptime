<script lang="ts">
  import * as Avatar from "$lib/components/ui/avatar/index.js";
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import * as Sidebar from "$lib/components/ui/sidebar/index.js";
  import { Skeleton } from "$lib/components/ui/skeleton/index.js";
  import { goto } from "$app/navigation";
  import { userStore } from "$lib/stores";
  import { EllipsisVerticalIcon, LogOutIcon } from "@lucide/svelte";

  const sidebar = Sidebar.useSidebar();

  function logout() {
    userStore.clear();
    goto("/login");
  }
</script>

<Sidebar.Menu>
  <Sidebar.MenuItem>
    {#if userStore.loading || !userStore.user}
      <!-- Loading/empty state without dropdown -->
      <Sidebar.MenuButton size="lg" class="pointer-events-none">
        {#if userStore.loading}
          <Skeleton class="size-8 rounded-lg" />
          <div class="grid flex-1 text-start text-sm leading-tight gap-1.5">
            <Skeleton class="h-4 w-24" />
            <Skeleton class="h-3 w-32" />
          </div>
        {:else}
          <!-- Optional: show a placeholder when user is undefined -->
          <div class="size-8 rounded-lg bg-muted"></div>
          <div class="grid flex-1 text-start text-sm leading-tight">
            <span class="text-muted-foreground text-sm">Not signed in</span>
          </div>
        {/if}
      </Sidebar.MenuButton>
    {:else}
      <!-- Regular dropdown when user is loaded -->
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Sidebar.MenuButton
              {...props}
              size="lg"
              class="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
            >
              <Avatar.Root class="size-8 rounded-lg grayscale">
                <Avatar.Fallback class="rounded-lg">CN</Avatar.Fallback>
              </Avatar.Root>
              <div class="grid flex-1 text-start text-sm leading-tight">
                <span class="truncate font-medium">{userStore.user.name}</span>
                <span class="text-muted-foreground truncate text-xs">
                  {userStore.user.email}
                </span>
              </div>
              <EllipsisVerticalIcon class="ms-auto size-4" />
            </Sidebar.MenuButton>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content
          class="w-(--bits-dropdown-menu-anchor-width) min-w-56 rounded-lg"
          side={sidebar.isMobile ? "bottom" : "right"}
          align="end"
          sideOffset={4}
        >
          <DropdownMenu.Label class="p-0 font-normal">
            <div class="flex items-center gap-2 px-1 py-1.5 text-start text-sm">
              <Avatar.Root class="size-8 rounded-lg">
                <Avatar.Fallback class="rounded-lg">CN</Avatar.Fallback>
              </Avatar.Root>
              <div class="grid flex-1 text-start text-sm leading-tight">
                <span class="truncate font-medium">{userStore.user.name}</span>
                <span class="text-muted-foreground truncate text-xs">
                  {userStore.user.email}
                </span>
              </div>
            </div>
          </DropdownMenu.Label>
          <DropdownMenu.Separator />
          <DropdownMenu.Item onclick={logout}>
            <LogOutIcon />
            Log out
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    {/if}
  </Sidebar.MenuItem>
</Sidebar.Menu>
