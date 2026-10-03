<script lang="ts">
  import XIcon from "@lucide/svelte/icons/x";
  import { Button } from "@/components/ui/button";
  import { t } from "@/lib/i18n/index.svelte";

  /**
   * Shown while the camera scans. The camera preview sits behind the web view, so this hides
   * the rest of the app (the `scanning` class, see styles.css) and draws a frame and a way out.
   * It's moved to `body`, the only part left visible.
   */
  let { onCancel }: { onCancel: () => void } = $props();

  $effect(() => {
    document.documentElement.classList.add("scanning");
    return () => document.documentElement.classList.remove("scanning");
  });

  function toBody(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }
</script>

<section
  use:toBody
  class="scan-overlay fixed inset-0 z-50 flex flex-col items-center justify-between p-6 text-white"
  aria-label={t("join.scanLabel")}
>
  <p class="rounded-full bg-black/60 px-4 py-2 text-center text-sm">
    {t("join.scanHelp")}
  </p>
  <div
    class="aspect-square w-3/4 max-w-72 rounded-3xl border-4 border-white/90 shadow-[0_0_0_100vmax_rgb(0_0_0/0.35)]"
    aria-hidden="true"
  ></div>
  <Button size="lg" variant="secondary" onclick={onCancel}>
    <XIcon data-icon="inline-start" />
    {t("common.cancel")}
  </Button>
</section>
