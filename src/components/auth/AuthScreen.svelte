<script lang="ts">
  import KeyRoundIcon from "@lucide/svelte/icons/key-round";
  import { toast } from "svelte-sonner";
  import LogoMark from "@/components/common/LogoMark.svelte";
  import Wordmark from "@/components/common/Wordmark.svelte";
  import * as Alert from "@/components/ui/alert";
  import { Button } from "@/components/ui/button";
  import * as Card from "@/components/ui/card";
  import * as Field from "@/components/ui/field";
  import { Input } from "@/components/ui/input";
  import { Spinner } from "@/components/ui/spinner";
  import * as Tabs from "@/components/ui/tabs";
  import type { NewRecoveryKey } from "@/lib/state/session.svelte";
  import { api } from "@/services/api";
  import type { AccountInfo } from "@/types";
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

  const TITLES: Record<Mode, [string, string]> = {
    login: ["Welcome back", "Log in to get your groups on this device."],
    signup: ["Create your account", "One account for your phone, your computer and every group."],
    recover: [
      "Reset your password",
      "Use the recovery key you saved when you created your account.",
    ],
  };

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
  const [title, description] = $derived(TITLES[mode]);

  function switchMode(next: Mode) {
    mode = next;
    error = null;
    password = "";
    confirmPassword = "";
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (newPassword && password !== confirmPassword) {
      error = "The passwords don't match.";
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
        toast.warning("Your account has no recovery key", {
          description:
            "This server can't store recovery keys yet, so a forgotten password can't be reset. Once the server is updated, create one from the account menu: New recovery key.",
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
              <Tabs.Trigger value="login">Log in</Tabs.Trigger>
              <Tabs.Trigger value="signup">Sign up</Tabs.Trigger>
            </Tabs.List>
          </Tabs.Root>
        {/if}

        <form onsubmit={handleSubmit}>
          <Field.Group>
            <Field.Field>
              <Field.Label for="input-username">Username</Field.Label>
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
                <Field.Label for="input-recovery-key">Recovery key</Field.Label>
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
                  {mode === "recover" ? "New password" : "Password"}
                </Field.Label>
                {#if mode === "login"}
                  <button
                    type="button"
                    class="text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
                    onclick={() => switchMode("recover")}
                  >
                    Forgot password?
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
                  {mode === "recover" ? "Confirm new password" : "Confirm password"}
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
                <Field.Label for="input-server">Server</Field.Label>
                <Input
                  id="input-server"
                  type="url"
                  required
                  bind:value={serverUrl}
                  autocomplete="url"
                  class="font-mono text-sm"
                />
                <Field.Description>
                  Only for a relay you run yourself. Your account and groups live on it.
                  {#if serverUrl.trim() !== DEFAULT_SERVER}
                    <button
                      type="button"
                      class="underline underline-offset-4 hover:text-foreground"
                      onclick={() => (serverUrl = DEFAULT_SERVER)}
                    >
                      Use the default server
                    </button>
                  {/if}
                </Field.Description>
              </Field.Field>
            {/if}

            {#if mode === "signup"}
              <Alert.Root>
                <KeyRoundIcon />
                <Alert.Description>
                  Nobody can reset your password, not even the server. You'll get a recovery key
                  next: it's the way back in if you forget it.
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
                ? "Log In"
                : mode === "signup"
                  ? "Create Account"
                  : "Reset Password"}
            </Button>

            {#if mode === "recover"}
              <Button type="button" variant="ghost" onclick={() => switchMode("login")}>
                Back to log in
              </Button>
            {/if}

            {#if !editingServer}
              <p class="text-center text-sm text-muted-foreground">
                Server: <span class="font-mono">{serverName(serverUrl)}</span> ·
                <button
                  type="button"
                  class="underline underline-offset-4 hover:text-foreground"
                  onclick={() => (editingServer = true)}
                  aria-label="Change server"
                >
                  Change
                </button>
              </p>
            {/if}
          </Field.Group>
        </form>
      </Card.Content>
    </Card.Root>
  </div>
</main>
