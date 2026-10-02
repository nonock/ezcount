<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import KeyRoundIcon from "@lucide/svelte/icons/key-round";
  import LockKeyholeIcon from "@lucide/svelte/icons/lock-keyhole";
  import LogOutIcon from "@lucide/svelte/icons/log-out";
  import MenuIcon from "@lucide/svelte/icons/menu";
  import MonitorIcon from "@lucide/svelte/icons/monitor";
  import MoonIcon from "@lucide/svelte/icons/moon";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import SunIcon from "@lucide/svelte/icons/sun";
  import { setMode, userPrefersMode } from "mode-watcher";
  import LogoMark from "@/components/common/LogoMark.svelte";
  import Wordmark from "@/components/common/Wordmark.svelte";
  import * as Avatar from "@/components/ui/avatar";
  import { Button } from "@/components/ui/button";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import type { Group } from "@/types";
  import { serverName } from "@/utils/formatters";

  interface Props {
    currentGroup: Group | null;
    onNavigateHome: () => void;
    onOpenCreateGroup: () => void;
    username: string;
    serverUrl: string;
    onChangePassword: () => void;
    onNewRecoveryKey: () => void;
    onLogOut: () => void;
  }

  let {
    currentGroup,
    onNavigateHome,
    onOpenCreateGroup,
    username,
    serverUrl,
    onChangePassword,
    onNewRecoveryKey,
    onLogOut,
  }: Props = $props();

  type Mode = "light" | "dark" | "system";
  const themes: { value: Mode; label: string; icon: typeof SunIcon }[] = [
    { value: "light", label: "Light", icon: SunIcon },
    { value: "dark", label: "Dark", icon: MoonIcon },
    { value: "system", label: "System", icon: MonitorIcon },
  ];
</script>

{#snippet themeItems()}
  <DropdownMenu.RadioGroup
    bind:value={() => userPrefersMode.current, (mode) => setMode(mode as Mode)}
  >
    {#each themes as theme (theme.value)}
      <DropdownMenu.RadioItem value={theme.value}>
        <theme.icon />
        {theme.label}
      </DropdownMenu.RadioItem>
    {/each}
  </DropdownMenu.RadioGroup>
{/snippet}

{#snippet accountItems()}
  <DropdownMenu.Label>
    <span class="block text-xs font-normal text-muted-foreground">Logged in as</span>
    <span class="block truncate">{username}</span>
    <span class="block truncate font-mono text-xs font-normal text-muted-foreground">
      {serverName(serverUrl)}
    </span>
  </DropdownMenu.Label>
{/snippet}

{#snippet settingsItems()}
  <DropdownMenu.Item onSelect={onChangePassword}>
    <LockKeyholeIcon />
    Change password
  </DropdownMenu.Item>
  <DropdownMenu.Item onSelect={onNewRecoveryKey}>
    <KeyRoundIcon />
    New recovery key
  </DropdownMenu.Item>
  <DropdownMenu.Separator />
  <DropdownMenu.Item onSelect={onLogOut}><LogOutIcon /> Log out</DropdownMenu.Item>
{/snippet}

<header
  class="sticky top-0 z-30 border-b bg-background/80 pt-[env(safe-area-inset-top)] backdrop-blur-lg"
>
  <div class="mx-auto flex h-14 max-w-5xl items-center justify-between gap-3 px-4">
    <!-- On phones the logo is in the bottom bar, and the header names the app. -->
    <Wordmark class="text-lg sm:hidden" />
    <nav aria-label="Breadcrumb" class="flex min-w-0 items-center gap-1.5 max-sm:hidden">
      <button
        type="button"
        onclick={onNavigateHome}
        class="flex shrink-0 items-center gap-2 rounded-lg outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <LogoMark class="size-8" />
        <Wordmark class="text-base" />
      </button>
      {#if currentGroup}
        <ChevronRightIcon class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span class="truncate text-sm text-muted-foreground" aria-current="page">
          {currentGroup.name}
        </span>
      {/if}
    </nav>

    <!-- On phones, everything in one menu: the account, the theme and the account's settings. -->
    <div class="sm:hidden">
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button {...props} variant="ghost" size="icon" aria-label="Menu">
              <MenuIcon />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end" class="min-w-56">
          {@render accountItems()}
          <DropdownMenu.Separator />
          <DropdownMenu.Label class="text-xs font-normal text-muted-foreground">
            Theme
          </DropdownMenu.Label>
          {@render themeItems()}
          <DropdownMenu.Separator />
          {@render settingsItems()}
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </div>

    <div class="flex shrink-0 items-center gap-1 max-sm:hidden">
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button {...props} variant="ghost" size="icon" aria-label="Change theme">
              <SunIcon class="dark:hidden" />
              <MoonIcon class="hidden dark:block" />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end">{@render themeItems()}</DropdownMenu.Content>
      </DropdownMenu.Root>

      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button {...props} variant="ghost" size="icon" aria-label="Account">
              <Avatar.Root size="sm" aria-hidden="true">
                <Avatar.Fallback>{username.slice(0, 1).toUpperCase()}</Avatar.Fallback>
              </Avatar.Root>
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end" class="min-w-48">
          {@render accountItems()}
          <DropdownMenu.Separator />
          {@render settingsItems()}
        </DropdownMenu.Content>
      </DropdownMenu.Root>

      <Button onclick={onOpenCreateGroup} aria-label="New group">
        <PlusIcon data-icon="inline-start" />
        <span class="hidden sm:inline">New Group</span>
      </Button>
    </div>
  </div>
</header>
