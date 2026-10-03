<script lang="ts">
  import { t } from "@/lib/i18n/index.svelte";
  // The launch screen. Same markup as the splash in index.html, which also holds its styles, so
  // the handoff from the static page to the app doesn't move a pixel.
</script>

<output class="ez-splash" aria-label={t("app.loading")}>
  <svg width="96" height="96" viewBox="0 0 100 100" aria-hidden="true">
    <rect width="100" height="100" rx="22" fill="#4f39f6" />
    <rect x="20" y="31" width="17" height="13" rx="6.5" fill="#ffffff" />
    <rect x="42" y="31" width="13" height="13" rx="6.5" fill="#ffffff" />
    <rect class="ez-splash-part" x="60" y="31" width="20" height="13" rx="6.5" fill="#7ee2a8" />
    <rect x="20" y="56" width="60" height="13" rx="6.5" fill="#ffffff" />
  </svg>
  <span class="ez-splash-name"><span>ez</span>count</span>
</output>
