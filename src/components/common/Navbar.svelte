<script lang="ts">
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import LightbulbIcon from "@lucide/svelte/icons/lightbulb";
  import MenuIcon from "@lucide/svelte/icons/menu";
  import MonitorIcon from "@lucide/svelte/icons/monitor";
  import MoonIcon from "@lucide/svelte/icons/moon";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import SmartphoneIcon from "@lucide/svelte/icons/smartphone";
  import SunIcon from "@lucide/svelte/icons/sun";
  import { setMode, userPrefersMode } from "mode-watcher";
  import LogoMark from "@/components/common/LogoMark.svelte";
  import Wordmark from "@/components/common/Wordmark.svelte";
  import * as Avatar from "@/components/ui/avatar";
  import { Button } from "@/components/ui/button";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { session } from "@/lib/state/session.svelte";

  interface Props {
    /** A group is open: creating another one is offered on the group list only. */
    inGroup: boolean;
    onNavigateHome: () => void;
    onOpenCreateGroup: () => void;
    username: string;
  }

  let { inGroup, onNavigateHome, onOpenCreateGroup, username }: Props = $props();

  /** The profile's name once there is one. */
  const name = $derived(session.account?.display_name ?? username);

  type Mode = "light" | "dark" | "system";
  const themes = [
    { value: "light", label: "menu.light", icon: SunIcon },
    { value: "dark", label: "menu.dark", icon: MoonIcon },
    { value: "system", label: "menu.system", icon: MonitorIcon },
  ] as const;
</script>

<header class="z-30 shrink-0 border-b bg-card pt-[env(safe-area-inset-top)]">
  <div class="mx-auto flex h-14 max-w-6xl items-center justify-between gap-3 px-4">
    <!-- On phones the logo is in the bottom bar, and the header names the app. -->
    <Wordmark class="text-lg sm:hidden" />
    <button
      type="button"
      onclick={onNavigateHome}
      class="flex shrink-0 items-center gap-2 rounded-lg outline-none focus-visible:ring-3 focus-visible:ring-ring/50 max-sm:hidden"
    >
      <LogoMark class="size-8" />
      <Wordmark class="text-base" />
    </button>

    <div class="flex shrink-0 items-center gap-2">
      <!-- Only on the group list; on phones it is in the bottom bar. -->
      {#if !inGroup}
        <Button onclick={onOpenCreateGroup} class="max-sm:hidden">
          <PlusIcon data-icon="inline-start" />
          {t("app.newGroup")}
        </Button>
      {/if}

      <!-- One menu: the account (its own dialog, with the language), connecting a device and the theme. -->
      <DropdownMenu.Root>
        <DropdownMenu.Trigger>
          {#snippet child({ props })}
            <Button {...props} variant="ghost" size="icon" aria-label={t("menu.menu")}>
              <MenuIcon />
            </Button>
          {/snippet}
        </DropdownMenu.Trigger>
        <DropdownMenu.Content align="end" class="min-w-60">
          <DropdownMenu.Item onSelect={() => (dialogs.account = true)} class="gap-2.5">
            <Avatar.Root size="sm" aria-hidden="true">
              {#if session.account?.avatar}
                <img
                  src={session.account.avatar}
                  alt=""
                  width="256"
                  height="256"
                  class="size-full rounded-full object-cover"
                />
              {:else}
                <Avatar.Fallback>{name.slice(0, 1).toUpperCase()}</Avatar.Fallback>
              {/if}
            </Avatar.Root>
            <span class="min-w-0">
              <span class="block truncate font-medium">{name}</span>
              <span class="block text-xs text-muted-foreground">{t("menu.account")}</span>
            </span>
            <ChevronRightIcon class="ml-auto text-muted-foreground" />
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => (dialogs.linkDevice = true)}>
            <SmartphoneIcon />
            {t("menu.linkDevice")}
          </DropdownMenu.Item>
          <DropdownMenu.Item onSelect={() => (dialogs.feedback = true)}>
            <LightbulbIcon />
            {t("menu.suggest")}
          </DropdownMenu.Item>
          <DropdownMenu.Separator />
          <!-- The three themes side by side, the current one raised. -->
          <div class="flex items-center justify-between gap-3 py-1 pr-1 pl-2.5">
            <span id="menu-theme" class="text-sm">{t("menu.theme")}</span>
            <DropdownMenu.RadioGroup
              aria-labelledby="menu-theme"
              class="flex gap-0.5 rounded-lg bg-muted p-0.5"
              bind:value={() => userPrefersMode.current, (mode) => setMode(mode as Mode)}
            >
              {#each themes as theme (theme.value)}
                <DropdownMenu.RadioItem
                  value={theme.value}
                  aria-label={t(theme.label)}
                  title={t(theme.label)}
                  class="size-8 justify-center p-0 text-muted-foreground data-[state=checked]:bg-card data-[state=checked]:text-foreground data-[state=checked]:shadow-sm *:data-[slot=dropdown-menu-radio-item-indicator]:hidden"
                >
                  <theme.icon />
                </DropdownMenu.RadioItem>
              {/each}
            </DropdownMenu.RadioGroup>
          </div>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </div>
  </div>
</header>
