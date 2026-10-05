<script lang="ts">
  import * as Field from "@/components/ui/field";
  import * as Select from "@/components/ui/select";
  import { CATEGORIES, categoryName, categoryOf } from "@/lib/categories";
  import { NO_CATEGORY } from "@/lib/expenseForm.svelte";
  import { t } from "@/lib/i18n/index.svelte";

  /** The kind of spending an expense is filed under: a category's key, or none. */
  let { value = $bindable() }: { value: string } = $props();
</script>

<Field.Field>
  <Field.Label for="select-expense-category">{t("category.label")}</Field.Label>
  <Select.Root type="single" bind:value>
    <Select.Trigger id="select-expense-category" class="w-full">
      {@const chosen = categoryOf(value)}
      <span class="flex items-center gap-2">
        {#if chosen}
          <chosen.icon class="size-4 text-muted-foreground" aria-hidden="true" />
        {/if}
        {categoryName(value)}
      </span>
    </Select.Trigger>
    <Select.Content>
      <Select.Item value={NO_CATEGORY} label={t("category.none")} />
      <Select.Separator />
      {#each CATEGORIES as option (option.key)}
        <Select.Item value={option.key} label={t(option.label)}>
          <option.icon class="text-muted-foreground" aria-hidden="true" />
          {t(option.label)}
        </Select.Item>
      {/each}
    </Select.Content>
  </Select.Root>
</Field.Field>
