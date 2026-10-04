<script lang="ts">
  import type { Snippet } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Field from "@/components/ui/field";
  import { t } from "@/lib/i18n/index.svelte";
  import { pictureFromFile } from "@/lib/picture";
  import { cn } from "@/lib/utils";

  interface Props {
    /** The picture, as a `data:` URL, or null for none. */
    value: string | null;
    /** What the picture is of, for the file input. */
    label: string;
    /** Round for a person, a rounded square for a group. */
    round?: boolean;
    /** Shown in place of the picture while there is none. */
    placeholder?: Snippet;
  }

  let { value = $bindable(), label, round = false, placeholder }: Props = $props();

  let input = $state<HTMLInputElement>();
  let error = $state<string | null>(null);

  async function choose() {
    const file = input?.files?.[0];
    if (!input || !file) return;
    // So choosing the same file again is a change too.
    input.value = "";
    error = null;
    try {
      value = await pictureFromFile(file);
    } catch {
      error = t("picture.unreadable");
    }
  }
</script>

<div class="flex items-center gap-3">
  <span
    aria-hidden="true"
    class={cn(
      "flex size-16 shrink-0 items-center justify-center overflow-hidden bg-muted text-xl text-muted-foreground",
      round ? "rounded-full" : "rounded-xl"
    )}
  >
    {#if value}
      <img
        src={value}
        alt=""
        width="256"
        height="256"
        class="size-full object-cover"
        data-testid="picture-preview"
      />
    {:else}
      {@render placeholder?.()}
    {/if}
  </span>
  <div class="flex flex-wrap gap-2">
    <Button type="button" variant="outline" size="sm" onclick={() => input?.click()}>
      {value ? t("picture.change") : t("picture.choose")}
    </Button>
    {#if value}
      <Button type="button" variant="ghost" size="sm" onclick={() => (value = null)}>
        {t("picture.remove")}
      </Button>
    {/if}
  </div>
  <input
    bind:this={input}
    type="file"
    accept="image/*"
    class="hidden"
    aria-label={label}
    onchange={choose}
  />
</div>
{#if error}
  <Field.Error>{error}</Field.Error>
{/if}
