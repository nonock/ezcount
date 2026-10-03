<script lang="ts">
  import PlusIcon from "@lucide/svelte/icons/plus";
  import XIcon from "@lucide/svelte/icons/x";
  import { untrack } from "svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import * as Select from "@/components/ui/select";
  import { Spinner } from "@/components/ui/spinner";
  import { createGroup } from "@/lib/actions";
  import { t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { session } from "@/lib/state/session.svelte";
  import { CURRENCIES, currencyLabel } from "@/utils/currencies";
  import { errorMessage } from "@/utils/errors";

  interface ParticipantField {
    id: string;
    name: string;
  }

  const SELF_ID = "p-self";
  const initialParticipants = (ownName = ""): ParticipantField[] => [
    { id: SELF_ID, name: ownName },
    { id: "p-init-2", name: "" },
    { id: "p-init-3", name: "" },
  ];

  const ownName = $derived(session.account?.username ?? "");

  let name = $state("");
  let currency = $state("EUR");
  let participants = $state<ParticipantField[]>(initialParticipants());
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Suggest the user's name when the dialog opens, without overwriting what they typed.
  $effect.pre(() => {
    if (!dialogs.createGroup) return;
    const suggested = ownName;
    untrack(() => {
      const self = participants[0];
      if (self && !self.name.trim()) self.name = suggested;
    });
  });

  function removeParticipant(id: string) {
    if (id !== SELF_ID) participants = participants.filter((p) => p.id !== id);
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const trimmedName = name.trim();
    // The user's own name stays first.
    const names = participants.map((p) => p.name.trim()).filter((p) => p.length > 0);
    if (!trimmedName) {
      error = t("create.needName");
      return;
    }
    if (!participants[0]?.name.trim()) {
      error = t("create.needYourName");
      return;
    }
    submitting = true;
    error = null;
    try {
      await createGroup(trimmedName, currency, names);
      name = "";
      currency = "EUR";
      participants = initialParticipants(ownName);
      dialogs.createGroup = false;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }
</script>

<Dialog.Root bind:open={dialogs.createGroup}>
  <Dialog.Content class="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>{t("create.title")}</Dialog.Title>
      <Dialog.Description>{t("create.intro")}</Dialog.Description>
    </Dialog.Header>

    <form onsubmit={handleSubmit}>
      <Field.Group>
        <Field.Field>
          <Field.Label for="input-group-name">{t("create.name")}</Field.Label>
          <Input
            id="input-group-name"
            required
            bind:value={name}
            placeholder={t("create.namePlaceholder")}
          />
        </Field.Field>

        <Field.Field>
          <Field.Label for="select-group-currency">{t("common.currency")}</Field.Label>
          <Select.Root type="single" bind:value={currency}>
            <Select.Trigger id="select-group-currency" class="w-full">
              {currencyLabel(currency)}
            </Select.Trigger>
            <Select.Content>
              {#each CURRENCIES as code (code)}
                <Select.Item value={code} label={currencyLabel(code)} />
              {/each}
            </Select.Content>
          </Select.Root>
        </Field.Field>

        <Field.Set>
          <Field.Legend variant="label">{t("create.participants")}</Field.Legend>
          <div class="space-y-2">
            {#each participants as p, idx (p.id)}
              <div class="flex items-center gap-2">
                <Input
                  bind:value={p.name}
                  placeholder={p.id === SELF_ID
                    ? t("create.yourName")
                    : t("create.participant", idx + 1)}
                  aria-label={p.id === SELF_ID
                    ? t("create.yourName")
                    : t("create.participant", idx + 1)}
                />
                {#if p.id === SELF_ID}
                  <span class="w-9 shrink-0 text-center text-xs text-muted-foreground"
                    >{t("common.you")}</span
                  >
                {:else}
                  <Button
                    variant="ghost"
                    size="icon"
                    onclick={() => removeParticipant(p.id)}
                    aria-label={t("create.removeParticipant", idx + 1)}
                  >
                    <XIcon />
                  </Button>
                {/if}
              </div>
            {/each}
          </div>
          <Button
            variant="ghost"
            size="sm"
            class="self-start"
            onclick={() => participants.push({ id: `p-${Date.now()}-${Math.random()}`, name: "" })}
          >
            <PlusIcon data-icon="inline-start" />
            {t("create.addPerson")}
          </Button>
        </Field.Set>

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
          {t("groups.create")}
        </Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
