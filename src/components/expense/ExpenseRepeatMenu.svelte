<script lang="ts">
  import RepeatIcon from "@lucide/svelte/icons/repeat";
  import { Button } from "@/components/ui/button";
  import * as DropdownMenu from "@/components/ui/dropdown-menu";
  import { NEVER, REPEATS } from "@/lib/expenseForm.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import { cn } from "@/lib/utils";

  /** How often a new expense comes back: every week, month or year, or never. */
  let { value = $bindable() }: { value: string } = $props();
</script>

<!-- Seldom used: a small button under the date, which says more once set. -->
<div class="col-span-2 -mt-2 text-sm">
  <DropdownMenu.Root>
    <DropdownMenu.Trigger>
      {#snippet child({ props })}
        <Button
          {...props}
          variant="ghost"
          size="sm"
          class={cn("-ml-2 h-7 font-normal", value === NEVER && "text-muted-foreground")}
        >
          <RepeatIcon data-icon="inline-start" />
          {#if value === NEVER}
            {t("expense.repeat")}…
          {:else}
            <span class="sr-only">{t("expense.repeat")}:</span>
            {t(REPEATS.find((r) => r.value === value)?.label ?? REPEATS[0].label)}
          {/if}
        </Button>
      {/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content align="start">
      <DropdownMenu.RadioGroup aria-label={t("expense.repeat")} bind:value>
        {#each REPEATS as option (option.value)}
          <DropdownMenu.RadioItem value={option.value}>
            {t(option.label)}
          </DropdownMenu.RadioItem>
        {/each}
      </DropdownMenu.RadioGroup>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
  {#if value !== NEVER}
    <p class="text-xs text-muted-foreground">{t("expense.repeatHelp")}</p>
  {/if}
</div>
