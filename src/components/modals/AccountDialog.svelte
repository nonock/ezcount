<script lang="ts">
  import KeyRoundIcon from "@lucide/svelte/icons/key-round";
  import LockKeyholeIcon from "@lucide/svelte/icons/lock-keyhole";
  import LogOutIcon from "@lucide/svelte/icons/log-out";
  import SmartphoneIcon from "@lucide/svelte/icons/smartphone";
  import * as Avatar from "@/components/ui/avatar";
  import { Button } from "@/components/ui/button";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import * as Select from "@/components/ui/select";
  import { Separator } from "@/components/ui/separator";
  import { logOut } from "@/lib/actions";
  import { i18n, LANGUAGES, type LanguageChoice, t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import { session } from "@/lib/state/session.svelte";
  import { serverName } from "@/utils/formatters";

  /** The account: who is logged in and where, and what can be done with it. */

  const account = $derived(session.account);
  const languageName = $derived(
    LANGUAGES.find((language) => language.code === i18n.choice)?.name ?? t("menu.system")
  );

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
      <div class="flex items-center gap-3">
        <Avatar.Root aria-hidden="true">
          <Avatar.Fallback>{account.username.slice(0, 1).toUpperCase()}</Avatar.Fallback>
        </Avatar.Root>
        <dl class="min-w-0">
          <dt class="sr-only">{t("auth.username")}</dt>
          <dd class="truncate font-medium">{account.username}</dd>
          <dt class="sr-only">{t("common.server")}</dt>
          <dd class="truncate font-mono text-xs text-muted-foreground">
            {serverName(account.server_url)}
          </dd>
        </dl>
      </div>

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
