<script lang="ts">
  import KeyRoundIcon from "@lucide/svelte/icons/key-round";
  import QrCodeIcon from "@lucide/svelte/icons/qr-code";
  import ScanQrCodeIcon from "@lucide/svelte/icons/scan-qr-code";
  import { toast } from "svelte-sonner";
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
  import { i18n, LANGUAGES, t } from "@/lib/i18n/index.svelte";
  import { dialogs } from "@/lib/state/dialogs.svelte";
  import type { NewRecoveryKey } from "@/lib/state/session.svelte";
  import { cn } from "@/lib/utils";
  import { api } from "@/services/api";
  import { nativeFeatures, ScanCancelled, scanQrCode } from "@/services/native.svelte";
  import type { AccountInfo, Received } from "@/types";
  import { errorMessage } from "@/utils/errors";
  import { serverName } from "@/utils/formatters";
  import PasswordStrengthMeter from "./PasswordStrengthMeter.svelte";
  import { ratePassword } from "./password-strength.svelte";

  type Mode = "login" | "signup" | "recover";

  let {
    onAuthenticated,
  }: { onAuthenticated: (account: AccountInfo, recoveryKey?: NewRecoveryKey) => void } = $props();

  // A relay the user picked instead of the default; unset when they use the default.
  const SERVER_KEY = "ezcount_sync_server";
  /** The relay accounts live on unless the user picks another. */
  const DEFAULT_SERVER =
    import.meta.env.VITE_EZCOUNT_SERVER ||
    // The web version is served by its relay (proxied to it in development).
    (import.meta.env.MODE === "web"
      ? location.origin
      : import.meta.env.DEV
        ? "http://localhost:8787"
        : "https://ezcount-relay.fly.dev");

  function rememberedServer(): string {
    try {
      return localStorage.getItem(SERVER_KEY) || DEFAULT_SERVER;
    } catch {
      return DEFAULT_SERVER;
    }
  }

  function rememberServer(server: string) {
    try {
      if (server === DEFAULT_SERVER) localStorage.removeItem(SERVER_KEY);
      else localStorage.setItem(SERVER_KEY, server);
    } catch {
      // Remembering the server is only a convenience.
    }
  }

  const TITLES = {
    login: ["auth.loginTitle", "auth.loginIntro"],
    signup: ["auth.signupTitle", "auth.signupIntro"],
    recover: ["auth.recoverTitle", "auth.recoverIntro"],
  } as const;

  let mode = $state<Mode>("login");
  let username = $state("");
  let password = $state("");
  let confirmPassword = $state("");
  let recoveryKey = $state("");
  let serverUrl = $state(rememberedServer());
  // Most people use the default relay, so the field stays out of the way until asked for.
  let editingServer = $state(false);
  let submitting = $state(false);
  let error = $state<string | null>(null);

  // Signing up and recovering both choose a new password, rated as the user types.
  const newPassword = $derived(mode !== "login");
  const strength = ratePassword(() => ({ password, username, enabled: newPassword }));
  const [title, description] = $derived(TITLES[mode].map((key) => t(key)));

  function switchMode(next: Mode) {
    mode = next;
    error = null;
    password = "";
    confirmPassword = "";
  }

  // Shows a code for a phone that is logged in to scan: for a device that can't scan.
  let receiving = $state(false);

  /** The phone that scanned the code logged this device into its account. */
  function received({ account }: Received) {
    receiving = false;
    if (!account) return;
    rememberServer(account.server_url);
    onAuthenticated(account);
  }

  /** Logs in with the code another device of the account shows (its menu: Connect a device). */
  async function scanToLogIn() {
    error = null;
    dialogs.scanning = true;
    let link: string;
    try {
      link = await scanQrCode();
    } catch (err) {
      if (!(err instanceof ScanCancelled)) error = errorMessage(err);
      return;
    } finally {
      dialogs.scanning = false;
    }
    submitting = true;
    try {
      const account = await api.logInWithLink(link);
      rememberServer(account.server_url);
      onAuthenticated(account);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (newPassword && password !== confirmPassword) {
      error = t("auth.mismatch");
      return;
    }
    submitting = true;
    error = null;
    try {
      const server = serverUrl.trim();
      if (mode === "login") {
        const account = await api.logIn(server, username, password);
        rememberServer(server);
        onAuthenticated(account);
        return;
      }
      const signedIn =
        mode === "signup"
          ? await api.signUp(server, username, password)
          : await api.recoverAccount(server, username, recoveryKey, password);
      rememberServer(server);
      if (!signedIn.recovery_key) {
        // A relay from before recovery keys: say so, rather than leave the user thinking they
        // have a way back in.
        toast.warning(t("auth.noRecoveryKey"), {
          description: t("auth.noRecoveryKeyHelp"),
          duration: Number.POSITIVE_INFINITY,
          closeButton: true,
        });
      }
      onAuthenticated(
        signedIn.account,
        signedIn.recovery_key
          ? { key: signedIn.recovery_key, reason: mode === "signup" ? "signup" : "recovered" }
          : undefined
      );
    } catch (err) {
      error = errorMessage(err);
    } finally {
      submitting = false;
    }
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
        {#if mode !== "recover"}
          <Tabs.Root value={mode} onValueChange={(value) => switchMode(value as Mode)}>
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
                bind:value={username}
                autocomplete="username"
                autocapitalize="off"
                autocorrect="off"
                spellcheck={false}
              />
            </Field.Field>
            {#if mode === "recover"}
              <Field.Field>
                <Field.Label for="input-recovery-key">{t("auth.recoveryKey")}</Field.Label>
                <Input
                  id="input-recovery-key"
                  required
                  bind:value={recoveryKey}
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
                  {mode === "recover" ? t("auth.newPassword") : t("common.password")}
                </Field.Label>
                {#if mode === "login"}
                  <button
                    type="button"
                    class="text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
                    onclick={() => switchMode("recover")}
                  >
                    {t("auth.forgot")}
                  </button>
                {/if}
              </div>
              <Input
                id="input-password"
                type="password"
                required
                minlength={newPassword ? 8 : undefined}
                bind:value={password}
                autocomplete={newPassword ? "new-password" : "current-password"}
                aria-describedby={newPassword && strength.current ? "password-strength" : undefined}
              />
              {#if newPassword && strength.current}
                <PasswordStrengthMeter strength={strength.current} id="password-strength" />
              {/if}
            </Field.Field>
            {#if newPassword}
              <Field.Field>
                <Field.Label for="input-confirm-password">
                  {mode === "recover" ? t("auth.confirmNewPassword") : t("auth.confirmPassword")}
                </Field.Label>
                <Input
                  id="input-confirm-password"
                  type="password"
                  required
                  bind:value={confirmPassword}
                  autocomplete="new-password"
                />
              </Field.Field>
            {/if}
            {#if editingServer}
              <Field.Field>
                <Field.Label for="input-server">{t("common.server")}</Field.Label>
                <Input
                  id="input-server"
                  type="url"
                  required
                  bind:value={serverUrl}
                  autocomplete="url"
                  class="font-mono text-sm"
                />
                <Field.Description>
                  {t("auth.serverHelp")}
                  {#if serverUrl.trim() !== DEFAULT_SERVER}
                    <button
                      type="button"
                      class="underline underline-offset-4 hover:text-foreground"
                      onclick={() => (serverUrl = DEFAULT_SERVER)}
                    >
                      {t("auth.defaultServer")}
                    </button>
                  {/if}
                </Field.Description>
              </Field.Field>
            {/if}

            {#if mode === "signup"}
              <Alert.Root>
                <KeyRoundIcon />
                <Alert.Description>
                  {t("auth.signupNote")}
                </Alert.Description>
              </Alert.Root>
            {/if}

            {#if error}
              <Field.Error>{error}</Field.Error>
            {/if}

            <Button
              type="submit"
              disabled={submitting || (newPassword && !strength.current?.acceptable)}
              class="w-full"
            >
              {#if submitting}
                <Spinner data-icon="inline-start" />
              {/if}
              {mode === "login"
                ? t("auth.logIn")
                : mode === "signup"
                  ? t("auth.createAccount")
                  : t("auth.resetPassword")}
            </Button>

            {#if mode === "login" && nativeFeatures.scan}
              <Button type="button" variant="outline" disabled={submitting} onclick={scanToLogIn}>
                <ScanQrCodeIcon data-icon="inline-start" />
                {t("auth.scan")}
              </Button>
              <Field.Description class="text-center">{t("auth.scanHelp")}</Field.Description>
            {/if}

            {#if mode === "login"}
              <Button
                type="button"
                variant="outline"
                disabled={submitting}
                onclick={() => (receiving = true)}
              >
                <QrCodeIcon data-icon="inline-start" />
                {t("receive.login")}
              </Button>
            {/if}

            {#if mode === "recover"}
              <Button type="button" variant="ghost" onclick={() => switchMode("login")}>
                {t("auth.backToLogin")}
              </Button>
            {/if}

            {#if !editingServer}
              <p class="text-center text-sm text-muted-foreground">
                {t("common.server")}: <span class="font-mono">{serverName(serverUrl)}</span> ·
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
          <ReceiveCode serverUrl={serverUrl.trim()} purpose="login" onReceived={received} />
        {/if}
      </Dialog.Content>
    </Dialog.Root>

    <!-- Before logging in there is no menu to pick the language from. -->
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
  </div>
</main>
