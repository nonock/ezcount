<script lang="ts">
  import KeyRoundIcon from "@lucide/svelte/icons/key-round";
  import QrCodeIcon from "@lucide/svelte/icons/qr-code";
  import ScanQrCodeIcon from "@lucide/svelte/icons/scan-qr-code";
  import LogoMark from "@/components/common/LogoMark.svelte";
  import ReceiveCode from "@/components/common/ReceiveCode.svelte";
  import Wordmark from "@/components/common/Wordmark.svelte";
  import * as Alert from "@/components/ui/alert";
  import { Button } from "@/components/ui/button";
  import * as Card from "@/components/ui/card";
  import * as Dialog from "@/components/ui/dialog";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import * as Tabs from "@/components/ui/tabs";
  import { AuthForm, type AuthMode } from "@/lib/authForm.svelte";
  import { t } from "@/lib/i18n/index.svelte";
  import type { NewRecoveryKey } from "@/lib/state/session.svelte";
  import { nativeFeatures } from "@/services/native.svelte";
  import type { AccountInfo, Received } from "@/types";
  import { serverName } from "@/utils/formatters";
  import AuthServerField from "./AuthServerField.svelte";
  import LanguageLinks from "./LanguageLinks.svelte";
  import PasswordStrengthMeter from "./PasswordStrengthMeter.svelte";
  import { ratePassword } from "./password-strength.svelte";

  let {
    onAuthenticated,
  }: { onAuthenticated: (account: AccountInfo, recoveryKey?: NewRecoveryKey) => void } = $props();

  const TITLES = {
    login: ["auth.loginTitle", "auth.loginIntro"],
    signup: ["auth.signupTitle", "auth.signupIntro"],
    recover: ["auth.recoverTitle", "auth.recoverIntro"],
  } as const;

  const form = new AuthForm((account, recoveryKey) => onAuthenticated(account, recoveryKey));
  // Most people use the default relay, so the field stays out of the way until asked for.
  let editingServer = $state(false);

  // A new password is rated as the user types.
  const strength = ratePassword(() => ({
    password: form.password,
    username: form.username,
    enabled: form.newPassword,
  }));
  const [title, description] = $derived(TITLES[form.mode].map((key) => t(key)));

  // Shows a code for a phone that is logged in to scan: for a device that can't scan.
  let receiving = $state(false);

  function received(what: Received) {
    receiving = false;
    form.received(what);
  }

  function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    form.submit();
  }
</script>

<main
  class="flex min-h-screen items-center justify-center px-4 py-10 pt-[max(2.5rem,env(safe-area-inset-top))]"
>
  <div class="w-full max-w-sm space-y-6">
    <div class="flex items-center justify-center gap-2">
      <LogoMark class="size-10" />
      <Wordmark class="text-xl" />
    </div>

    <Card.Root>
      <Card.Header>
        <Card.Title><h1>{title}</h1></Card.Title>
        <Card.Description>{description}</Card.Description>
      </Card.Header>
      <Card.Content class="space-y-6">
        {#if form.mode !== "recover"}
          <Tabs.Root
            value={form.mode}
            onValueChange={(value) => form.switchMode(value as AuthMode)}
          >
            <Tabs.List class="w-full">
              <Tabs.Trigger value="login">{t("auth.logInTab")}</Tabs.Trigger>
              <Tabs.Trigger value="signup">{t("auth.signUpTab")}</Tabs.Trigger>
            </Tabs.List>
          </Tabs.Root>
        {/if}

        <form onsubmit={handleSubmit}>
          <Field.Group>
            <Field.Field>
              <Field.Label for="input-username">{t("auth.username")}</Field.Label>
              <Input
                id="input-username"
                required
                bind:value={form.username}
                autocomplete="username"
                autocapitalize="off"
                autocorrect="off"
                spellcheck={false}
              />
            </Field.Field>
            {#if form.mode === "recover"}
              <Field.Field>
                <Field.Label for="input-recovery-key">{t("auth.recoveryKey")}</Field.Label>
                <Input
                  id="input-recovery-key"
                  required
                  bind:value={form.recoveryKey}
                  placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
                  autocomplete="off"
                  autocapitalize="characters"
                  autocorrect="off"
                  spellcheck={false}
                  class="font-mono text-sm"
                />
              </Field.Field>
            {/if}
            <Field.Field>
              <div class="flex items-baseline justify-between gap-2">
                <Field.Label for="input-password">
                  {form.mode === "recover" ? t("auth.newPassword") : t("common.password")}
                </Field.Label>
                {#if form.mode === "login"}
                  <button
                    type="button"
                    class="text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
                    onclick={() => form.switchMode("recover")}
                  >
                    {t("auth.forgot")}
                  </button>
                {/if}
              </div>
              <Input
                id="input-password"
                type="password"
                required
                minlength={form.newPassword ? 8 : undefined}
                bind:value={form.password}
                autocomplete={form.newPassword ? "new-password" : "current-password"}
                aria-describedby={form.newPassword && strength.current
                  ? "password-strength"
                  : undefined}
              />
              {#if form.newPassword && strength.current}
                <PasswordStrengthMeter strength={strength.current} id="password-strength" />
              {/if}
            </Field.Field>
            {#if form.newPassword}
              <Field.Field>
                <Field.Label for="input-confirm-password">
                  {form.mode === "recover"
                    ? t("auth.confirmNewPassword")
                    : t("auth.confirmPassword")}
                </Field.Label>
                <Input
                  id="input-confirm-password"
                  type="password"
                  required
                  bind:value={form.confirmPassword}
                  autocomplete="new-password"
                />
              </Field.Field>
            {/if}
            {#if editingServer}
              <AuthServerField bind:value={form.serverUrl} />
            {/if}

            {#if form.mode === "signup"}
              <Alert.Root>
                <KeyRoundIcon />
                <Alert.Description>
                  {t("auth.signupNote")}
                </Alert.Description>
              </Alert.Root>
            {/if}

            {#if form.error}
              <Field.Error>{form.error}</Field.Error>
            {/if}

            <Button
              type="submit"
              disabled={form.submitting || (form.newPassword && !strength.current?.acceptable)}
              class="w-full"
            >
              {#if form.submitting}
                <Spinner data-icon="inline-start" />
              {/if}
              {form.mode === "login"
                ? t("auth.logIn")
                : form.mode === "signup"
                  ? t("auth.createAccount")
                  : t("auth.resetPassword")}
            </Button>

            {#if form.mode === "login" && nativeFeatures.scan}
              <Button
                type="button"
                variant="outline"
                disabled={form.submitting}
                onclick={() => form.scanToLogIn()}
              >
                <ScanQrCodeIcon data-icon="inline-start" />
                {t("auth.scan")}
              </Button>
              <Field.Description class="text-center">{t("auth.scanHelp")}</Field.Description>
            {/if}

            {#if form.mode === "login"}
              <Button
                type="button"
                variant="outline"
                disabled={form.submitting}
                onclick={() => (receiving = true)}
              >
                <QrCodeIcon data-icon="inline-start" />
                {t("receive.login")}
              </Button>
            {/if}

            {#if form.mode === "recover"}
              <Button type="button" variant="ghost" onclick={() => form.switchMode("login")}>
                {t("auth.backToLogin")}
              </Button>
            {/if}

            {#if !editingServer}
              <p class="text-center text-sm text-muted-foreground">
                {t("common.server")}: <span class="font-mono">{serverName(form.serverUrl)}</span> ·
                <button
                  type="button"
                  class="underline underline-offset-4 hover:text-foreground"
                  onclick={() => (editingServer = true)}
                  aria-label={t("auth.changeServer")}
                >
                  {t("common.change")}
                </button>
              </p>
            {/if}
          </Field.Group>
        </form>
      </Card.Content>
    </Card.Root>

    <Dialog.Root bind:open={receiving}>
      <Dialog.Content class="sm:max-w-md">
        <Dialog.Header>
          <Dialog.Title>{t("receive.login")}</Dialog.Title>
          <Dialog.Description>{t("receive.loginHelp")}</Dialog.Description>
        </Dialog.Header>
        {#if receiving}
          <ReceiveCode serverUrl={form.serverUrl.trim()} purpose="login" onReceived={received} />
        {/if}
      </Dialog.Content>
    </Dialog.Root>

    <LanguageLinks />
  </div>
</main>
