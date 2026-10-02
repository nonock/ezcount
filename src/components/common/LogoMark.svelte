<script lang="ts">
  import { cn } from "@/lib/utils";

  /** The app icon: shares that add up to the total. Size it with `class` (e.g. `size-8`). */
  let { class: className }: { class?: string } = $props();
</script>

<svg viewBox="0 0 100 100" aria-hidden="true" class={cn("shrink-0", className)}>
  <rect width="100" height="100" rx="22" fill="#4f39f6" />
  <rect x="20" y="31" width="17" height="13" rx="6.5" fill="#ffffff" />
  <rect x="42" y="31" width="13" height="13" rx="6.5" fill="#ffffff" />
  <rect x="60" y="31" width="20" height="13" rx="6.5" fill="#7ee2a8" />
  <rect x="20" y="56" width="60" height="13" rx="6.5" fill="#ffffff" />
</svg>
