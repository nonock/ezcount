<script lang="ts">
  import type { Snippet } from "svelte";
  import LogoMark from "@/components/common/LogoMark.svelte";
  import { cn } from "@/lib/utils";

  interface Props {
    onHome: () => void;
    /** The group list is on screen. */
    home?: boolean;
    class?: string;
    children?: Snippet;
  }

  /**
   * On phones, a bar at the bottom of the screen, within reach of the thumb: the logo leads
   * back to the group list, followed by `children`. On wider screens only `children` show, in
   * place.
   */
  let { onHome, home = false, class: className, children }: Props = $props();
</script>

<nav
  aria-label="App"
  class={cn(
    "max-sm:fixed max-sm:inset-x-0 max-sm:bottom-0 max-sm:z-30 max-sm:flex max-sm:items-center max-sm:gap-2 max-sm:border-t max-sm:bg-background/95 max-sm:px-2 max-sm:pt-1.5 max-sm:pb-[max(0.375rem,env(safe-area-inset-bottom))] max-sm:backdrop-blur-lg",
    className
  )}
>
  <button
    type="button"
    onclick={onHome}
    aria-current={home ? "page" : undefined}
    class={cn(
      "flex shrink-0 flex-col items-center gap-0.5 rounded-md px-3 py-1.5 text-xs font-medium text-foreground/60 outline-none focus-visible:ring-3 focus-visible:ring-ring/50 sm:hidden dark:text-muted-foreground",
      home && "bg-muted text-foreground dark:text-foreground"
    )}
  >
    <LogoMark class="size-5" />
    Groups
  </button>
  {@render children?.()}
</nav>
