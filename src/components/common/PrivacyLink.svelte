<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { i18n, t } from "@/lib/i18n/index.svelte";
  import { privacyUrl } from "@/lib/server";
  import { cn } from "@/lib/utils";

  /**
   * The way to a relay's privacy policy, a page of its own, in the app's language. The app
   * has no browser in it, so the device's opens the page; the web version is in one, which
   * follows the link.
   */
  let { serverUrl, class: className }: { serverUrl: string; class?: string } = $props();

  const href = $derived(privacyUrl(serverUrl, i18n.language));

  function open(e: MouseEvent) {
    if (import.meta.env.MODE === "web") return;
    e.preventDefault();
    openUrl(href).catch((err) => console.error("Could not open the privacy policy:", err));
  }
</script>

<a
  {href}
  target="_blank"
  rel="noreferrer"
  class={cn("underline underline-offset-4 hover:text-foreground", className)}
  onclick={open}
>
  {t("menu.privacy")}
</a>
