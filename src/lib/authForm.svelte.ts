// The login screen's form: logging in, creating an account, and choosing a new password with
// the recovery key. `AuthScreen.svelte` shows it.

import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { ScanCancelled, scanQrCode } from "@/services/native.svelte";
import type { AccountInfo, Received } from "@/types";
import { errorMessage } from "@/utils/errors";
import { t } from "./i18n/index.svelte";
import { rememberedServer, rememberServer } from "./server";
import { dialogs } from "./state/dialogs.svelte";
import type { NewRecoveryKey } from "./state/session.svelte";

export type AuthMode = "login" | "signup" | "recover";

/** Called once the device is logged in, with the recovery key to show when there is a new one. */
type OnAuthenticated = (account: AccountInfo, recoveryKey?: NewRecoveryKey) => void;

export class AuthForm {
  #onAuthenticated: OnAuthenticated;

  mode = $state<AuthMode>("login");
  username = $state("");
  password = $state("");
  confirmPassword = $state("");
  recoveryKey = $state("");
  serverUrl = $state(rememberedServer());
  submitting = $state(false);
  error = $state<string | null>(null);

  constructor(onAuthenticated: OnAuthenticated) {
    this.#onAuthenticated = onAuthenticated;
  }

  /** Signing up and recovering both choose a new password. */
  get newPassword(): boolean {
    return this.mode !== "login";
  }

  switchMode(next: AuthMode) {
    this.mode = next;
    this.error = null;
    this.password = "";
    this.confirmPassword = "";
  }

  /** The phone that scanned this device's code logged it into its account. */
  received({ account }: Received) {
    if (!account) return;
    rememberServer(account.server_url);
    this.#onAuthenticated(account);
  }

  /** Logs in with the code another device of the account shows (its menu: Connect a device). */
  async scanToLogIn() {
    this.error = null;
    dialogs.scanning = true;
    let link: string;
    try {
      link = await scanQrCode();
    } catch (err) {
      if (!(err instanceof ScanCancelled)) this.error = errorMessage(err);
      return;
    } finally {
      dialogs.scanning = false;
    }
    this.submitting = true;
    try {
      const account = await api.logInWithLink(link);
      rememberServer(account.server_url);
      this.#onAuthenticated(account);
    } catch (err) {
      this.error = errorMessage(err);
    } finally {
      this.submitting = false;
    }
  }

  async submit() {
    const mode = this.mode;
    if (this.newPassword && this.password !== this.confirmPassword) {
      this.error = t("auth.mismatch");
      return;
    }
    this.submitting = true;
    this.error = null;
    try {
      const server = this.serverUrl.trim();
      if (mode === "login") {
        const account = await api.logIn(server, this.username, this.password);
        rememberServer(server);
        this.#onAuthenticated(account);
        return;
      }
      const signedIn =
        mode === "signup"
          ? await api.signUp(server, this.username, this.password)
          : await api.recoverAccount(server, this.username, this.recoveryKey, this.password);
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
      this.#onAuthenticated(
        signedIn.account,
        signedIn.recovery_key
          ? { key: signedIn.recovery_key, reason: mode === "signup" ? "signup" : "recovered" }
          : undefined
      );
    } catch (err) {
      this.error = errorMessage(err);
    } finally {
      this.submitting = false;
    }
  }
}
