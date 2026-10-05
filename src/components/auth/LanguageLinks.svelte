<script lang="ts">
  import { i18n, LANGUAGES } from "@/lib/i18n/index.svelte";
  import { cn } from "@/lib/utils";

  /** The languages, each under its own name: before logging in there is no menu to pick one from. */
</script>

<div class="flex justify-center gap-4 text-sm text-muted-foreground">
  {#each LANGUAGES as language (language.code)}
    <button
      type="button"
      lang={language.code}
      aria-pressed={i18n.language === language.code}
      class={cn(
        "underline-offset-4 hover:text-foreground hover:underline",
        i18n.language === language.code && "font-medium text-foreground"
      )}
      onclick={() => i18n.choose(language.code)}
    >
      {language.name}
    </button>
  {/each}
</div>
