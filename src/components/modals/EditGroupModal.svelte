<script lang="ts">
  import { untrack } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import * as Select from "@/components/ui/select";
  import { Spinner } from "@/components/ui/spinner";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { openGroup } from "@/lib/state/groups.svelte";
  import { api } from "@/services/api";
  import type { Group } from "@/types";
  import { CURRENCIES, currencyLabel } from "@/utils/currencies";
  import { errorMessage } from "@/utils/errors";

  let { group }: { group: Group } = $props();

  let name = $state("");
  let currency = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  $effect.pre(() => {
    if (!dialogs.editGroup) return;
    const initial = { name: group.name, currency: group.currency };
    untrack(() => {
      name = initial.name;
      currency = initial.currency;
      error = null;
    });
  });

  // A group made elsewhere may use a currency this list doesn't offer.
  const options = $derived(
    CURRENCIES.includes(group.currency) ? CURRENCIES : [group.currency, ...CURRENCIES]
  );

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) {
      error = t("create.needName");
      return;
    }
    submitting = true;
    error = null;
    try {
      await openGroup.change((groupId) => api.updateGroup(groupId, trimmed, currency));
      dialogs.editGroup = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog.Root bind:open={dialogs.editGroup}>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("editGroup.title")}</Dialog.Title>
      <Dialog.Description>{t("editGroup.intro")}</Dialog.Description>
    </Dialog.Header>
    <form onsubmit={handleSubmit}>
      <Field.Group>
        <Field.Field>
          <Field.Label for="input-edit-group-name">{t("create.name")}</Field.Label>
          <Input id="input-edit-group-name" required bind:value={name} />
        </Field.Field>
        <Field.Field>
          <Field.Label for="select-edit-group-currency">{t("common.currency")}</Field.Label>
          <Select.Root type="single" bind:value={currency}>
            <Select.Trigger id="select-edit-group-currency" class="w-full">
              {currencyLabel(currency)}
            </Select.Trigger>
            <Select.Content>
              {#each options as code (code)}
                <Select.Item value={code} label={currencyLabel(code)} />
              {/each}
            </Select.Content>
          </Select.Root>
          <Field.Description>
            {t("editGroup.currencyHelp")}
          </Field.Description>
        </Field.Field>
        {#if error}
          <Field.Error>{error}</Field.Error>
        {/if}
      </Field.Group>
      <Dialog.Footer class="mt-6">
        <Dialog.Close>
          {#snippet child({ props })}
            <Button {...props} variant="outline">{t("common.cancel")}</Button>
          {/snippet}
        </Dialog.Close>
        <Button type="submit" disabled={submitting}>
          {#if submitting}
            <Spinner data-icon="inline-start" />
          {/if}
          {t("common.save")}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
