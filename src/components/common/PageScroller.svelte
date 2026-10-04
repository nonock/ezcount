<script lang="ts">
  import ArrowUpIcon from "@lucide/svelte/icons/arrow-up";
  import RefreshCwIcon from "@lucide/svelte/icons/refresh-cw";
  import type { Snippet } from "svelte";
  import { Button } from "@/components/ui/button";
  import { t } from "@/lib/i18n/index.svelte";
  import { cn } from "@/lib/utils";

  interface Props {
    /** What pulling the page down from its top does; without it, pulling does nothing. */
    onRefresh?: () => Promise<void>;
    class?: string;
    children: Snippet;
  }

  let { onRefresh, class: className, children }: Props = $props();

  /** How far down the page offers to go back up, and how far a pull must go to refresh. */
  const FAR = 600;
  const PULL = 72;

  let scroller = $state<HTMLDivElement>();
  let far = $state(false);
  // How far the finger pulled from the top of the page, slowed down; null when it isn't.
  let pulled = $state<number | null>(null);
  let refreshing = $state(false);
  let startY = 0;

  function toTop() {
    const calm = matchMedia("(prefers-reduced-motion: reduce)").matches;
    scroller?.scrollTo({ top: 0, behavior: calm ? "auto" : "smooth" });
  }

  function onTouchStart(e: TouchEvent) {
    if (!onRefresh || refreshing || e.touches.length !== 1 || scroller?.scrollTop !== 0) return;
    startY = e.touches[0].clientY;
    pulled = 0;
  }

  function onTouchMove(e: TouchEvent) {
    if (pulled === null) return;
    const distance = e.touches[0].clientY - startY;
    // Scrolling down the page instead: not a pull.
    pulled = distance < 0 || scroller?.scrollTop !== 0 ? null : Math.min(distance / 2, PULL * 1.5);
  }

  async function onTouchEnd() {
    const enough = pulled !== null && pulled >= PULL;
    pulled = null;
    if (!enough || !onRefresh) return;
    refreshing = true;
    try {
      await onRefresh();
    } finally {
      refreshing = false;
    }
  }

  // Only watched, never stopped: the page scrolls as usual under the finger.
  $effect(() => {
    if (!scroller) return;
    const cancel = () => (pulled = null);
    const watching = [
      ["touchstart", onTouchStart],
      ["touchmove", onTouchMove],
      ["touchend", onTouchEnd],
      ["touchcancel", cancel],
    ] as const;
    for (const [name, handle] of watching) {
      scroller.addEventListener(name, handle as EventListener, { passive: true });
    }
    return () => {
      for (const [name, handle] of watching) {
        scroller?.removeEventListener(name, handle as EventListener);
      }
    };
  });

  const shown = $derived(refreshing ? PULL : (pulled ?? 0));
</script>

<div class={cn("relative min-h-0 flex-1", className)}>
  <div
    bind:this={scroller}
    class="h-full overflow-y-auto overscroll-y-none"
    onscroll={() => (far = (scroller?.scrollTop ?? 0) > FAR)}
  >
    {@render children()}
  </div>

  <!-- Comes down with the finger, and turns while the group is brought up to date. -->
  {#if shown > 0}
    <div
      class="pointer-events-none absolute inset-x-0 top-0 z-20 flex justify-center"
      style:transform={`translateY(${shown - 44}px)`}
      role="status"
      aria-label={refreshing ? t("app.refreshing") : undefined}
    >
      <div class="rounded-full bg-card p-2 shadow-md ring-1 ring-foreground/10">
        <RefreshCwIcon
          class={cn("size-5 text-primary", refreshing && "animate-spin")}
          style={refreshing ? undefined : `transform: rotate(${shown * 3}deg)`}
          aria-hidden="true"
        />
      </div>
    </div>
  {/if}

  {#if far}
    <!-- On phones on the left, the right being the button that adds an expense. -->
    <Button
      variant="outline"
      size="icon"
      onclick={toTop}
      aria-label={t("app.backToTop")}
      class="absolute bottom-4 left-4 z-20 size-11 rounded-full bg-card shadow-lg sm:right-6 sm:bottom-6 sm:left-auto"
    >
      <ArrowUpIcon />
    </Button>
  {/if}
</div>
