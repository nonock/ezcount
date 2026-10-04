<script lang="ts">
  import KeyRoundIcon from "@lucide/svelte/icons/key-round";
  import LockKeyholeIcon from "@lucide/svelte/icons/lock-keyhole";
  import LogOutIcon from "@lucide/svelte/icons/log-out";
  import SmartphoneIcon from "@lucide/svelte/icons/smartphone";
  import { untrack } from "svelte";
  import { toast } from "svelte-sonner";
  import PictureField from "@/components/common/PictureField.svelte";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import * as Select from "@/components/ui/select";
  import { Separator } from "@/components/ui/separator";
  import { Spinner } from "@/components/ui/spinner";
  import { logOut } from "@/lib/actions";
  import { i18n, LANGUAGES, type LanguageChoice, t } from "@/lib/i18n/index.svelte";
  import { formatIban, isIban } from "@/lib/sepa";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { groupList, openGroup } from "@/lib/state/groups.svelte";
  import { session } from "@/lib/state/session.svelte";
  import { api } from "@/services/api";
  import { errorMessage } from "@/utils/errors";
  import { serverName } from "@/utils/formatters";

  /** The account: the profile its groups show, where it lives, and what can be done with it. */

  const account = $derived(session.account);
  const languageName = $derived(
    LANGUAGES.find((language) => language.code === i18n.choice)?.name ?? t("menu.system")
  );

  let name = $state("");
  let avatar = $state<string | null>(null);
  let iban = $state("");
  let saving = $state(false);
  let error = $state<string | null>(null);

  $effect.pre(() => {
    if (!dialogs.account) return;
    const initial = {
      name: account?.display_name ?? "",
      avatar: account?.avatar ?? null,
      iban: formatIban(account?.iban ?? ""),
    };
    untrack(() => {
      name = initial.name;
      avatar = initial.avatar;
      iban = initial.iban;
      error = null;
    });
  });

  /** As stored: upper-case, without spaces. */
  const typedIban = $derived(iban.replace(/\s/g, "").toUpperCase());
  const changed = $derived(
    name.trim() !== (account?.display_name ?? "") ||
      avatar !== (account?.avatar ?? null) ||
      typedIban !== (account?.iban ?? "")
  );

  /** The profile goes onto the user's member in each group, so those are read again. */
  async function saveProfile(e: SubmitEvent) {
    e.preventDefault();
    if (typedIban && !isIban(typedIban)) {
      error = t("account.ibanInvalid");
      return;
    }
    saving = true;
    error = null;
    try {
      session.account = await api.updateProfile(name.trim(), avatar, typedIban || null);
      await groupList.refresh();
      if (openGroup.group) await openGroup.load(openGroup.group.id);
      toast.success(t("account.saved"));
    } catch (err) {
      error = errorMessage(err);
    } finally {
      saving = false;
    }
  }

  /** Each of these has its own dialog, which takes this one's place. */
  function go(open: () => void) {
    dialogs.account = false;
    open();
  }
</script>

<Dialog.Root bind:open={dialogs.account}>
  <Dialog.Content class="sm:max-w-sm">
    <Dialog.Header>
      <Dialog.Title>{t("menu.account")}</Dialog.Title>
      <Dialog.Description>{t("account.intro")}</Dialog.Description>
    </Dialog.Header>

    {#if account}
      <form onsubmit={saveProfile}>
        <Field.Group>
          <PictureField bind:value={avatar} label={t("account.picture")} round>
            {#snippet placeholder()}
              {(name.trim() || account.username).slice(0, 1).toUpperCase()}
            {/snippet}
          </PictureField>
          <Field.Field>
            <Field.Label for="input-profile-name">{t("account.name")}</Field.Label>
            <Input
              id="input-profile-name"
              bind:value={name}
              maxlength={50}
              placeholder={account.username}
              autocomplete="name"
            />
            <Field.Description>{t("account.nameHelp")}</Field.Description>
          </Field.Field>
          <Field.Field>
            <Field.Label for="input-profile-iban">{t("account.iban")}</Field.Label>
            <Input
              id="input-profile-iban"
              bind:value={iban}
              maxlength={42}
              placeholder="FR76 …"
              autocomplete="off"
              autocapitalize="characters"
              spellcheck={false}
              class="font-mono text-sm"
            />
            <Field.Description>{t("account.ibanHelp")}</Field.Description>
          </Field.Field>
          {#if error}
            <Field.Error>{error}</Field.Error>
          {/if}
          {#if changed}
            <Button type="submit" disabled={saving} class="w-fit">
              {#if saving}
                <Spinner data-icon="inline-start" />
              {/if}
              {t("account.save")}
            </Button>
          {/if}
        </Field.Group>
      </form>

      <dl class="text-xs text-muted-foreground">
        <dt class="sr-only">{t("auth.username")}</dt>
        <dd class="truncate">{account.username}</dd>
        <dt class="sr-only">{t("common.server")}</dt>
        <dd class="truncate font-mono">{serverName(account.server_url)}</dd>
      </dl>

      <div class="flex flex-col gap-2">
        <Button
          variant="outline"
          class="justify-start"
          onclick={() => go(() => (dialogs.linkDevice = true))}
        >
          <SmartphoneIcon data-icon="inline-start" />
          {t("menu.linkDevice")}
        </Button>
        <Button
          variant="outline"
          class="justify-start"
          onclick={() => go(() => (dialogs.changePassword = true))}
        >
          <LockKeyholeIcon data-icon="inline-start" />
          {t("menu.changePassword")}
        </Button>
        <Button
          variant="outline"
          class="justify-start"
          onclick={() => go(() => (dialogs.newRecoveryKey = true))}
        >
          <KeyRoundIcon data-icon="inline-start" />
          {t("menu.newRecoveryKey")}
        </Button>
      </div>

      <Field.Field>
        <Field.Label for="select-language">{t("menu.language")}</Field.Label>
        <Select.Root
          type="single"
          bind:value={() => i18n.choice, (choice) => i18n.choose(choice as LanguageChoice)}
        >
          <Select.Trigger id="select-language" class="w-full">{languageName}</Select.Trigger>
          <Select.Content>
            <Select.Item value="system" label={t("menu.system")} />
            {#each LANGUAGES as language (language.code)}
              <Select.Item value={language.code} label={language.name} lang={language.code} />
            {/each}
          </Select.Content>
        </Select.Root>
      </Field.Field>

      <Separator />
      <Button variant="ghost" class="justify-start text-destructive" onclick={() => go(logOut)}>
        <LogOutIcon data-icon="inline-start" />
        {t("menu.logOut")}
      </Button>
    {/if}
  </Dialog.Content>
</Dialog.Root>
