<script lang="ts">
  import { cn } from "@/lib/utils";

  /** "ezcount", with "ez" in the brand color. */
  let { class: className }: { class?: string } = $props();
</script>

<span class={cn("font-semibold tracking-tight", className)}
  ><span class="text-[#4f39f6] dark:text-[#9b8cff]">ez</span>count</span
>
