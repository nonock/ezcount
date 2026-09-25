import { LogoMark, Wordmark } from "@/components/common/Logo";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api } from "@/services/api";
import type { AccountInfo } from "@/types";
import { errorMessage } from "@/utils/errors";
import { serverName } from "@/utils/formatters";
import { KeyRoundIcon } from "lucide-react";
import type React from "react";
import { useState } from "react";
import { toast } from "sonner";
import { PasswordStrengthMeter, usePasswordStrength } from "./PasswordStrengthMeter";

type Mode = "login" | "signup" | "recover";

/** A recovery key to show once the user is in, and why it's new. */
export interface NewRecoveryKey {
  key: string;
  reason: "signup" | "recovered";
}

// A relay the user picked instead of the default; unset when they use the default.
const SERVER_KEY = "ezcount_sync_server";
/** The relay accounts live on unless the user picks another. */
const DEFAULT_SERVER =
  import.meta.env.VITE_EZCOUNT_SERVER ||
  (import.meta.env.DEV ? "http://localhost:8787" : "https://ezcount-relay.fly.dev");

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
  recover: ["Reset your password", "Use the recovery key you saved when you created your account."],
};

interface AuthScreenProps {
  onAuthenticated: (account: AccountInfo, recoveryKey?: NewRecoveryKey) => void;
}

export const AuthScreen: React.FC<AuthScreenProps> = ({ onAuthenticated }) => {
  const [mode, setMode] = useState<Mode>("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [serverUrl, setServerUrl] = useState(rememberedServer);
  // Most people use the default relay, so the field stays out of the way until asked for.
  const [editingServer, setEditingServer] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Signing up and recovering both choose a new password, rated as the user types.
  const newPassword = mode !== "login";
  const strength = usePasswordStrength(password, username, newPassword);
  const [title, description] = TITLES[mode];

  const switchMode = (next: Mode) => {
    setMode(next);
    setError(null);
    setPassword("");
    setConfirmPassword("");
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (newPassword && password !== confirmPassword) {
      setError("The passwords don't match.");
      return;
    }
    setSubmitting(true);
    setError(null);
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
      setError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <main className="flex min-h-screen items-center justify-center px-4 py-10 pt-[max(2.5rem,env(safe-area-inset-top))]">
      <div className="w-full max-w-sm space-y-6">
        <div className="flex items-center justify-center gap-2">
          <LogoMark className="size-10" />
          <Wordmark className="text-xl" />
        </div>

        <Card>
          <CardHeader>
            <CardTitle>
              <h1>{title}</h1>
            </CardTitle>
            <CardDescription>{description}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            {mode !== "recover" && (
              <Tabs value={mode} onValueChange={(value) => switchMode(value as Mode)}>
                <TabsList className="w-full">
                  <TabsTrigger value="login">Log in</TabsTrigger>
                  <TabsTrigger value="signup">Sign up</TabsTrigger>
                </TabsList>
              </Tabs>
            )}

            <form onSubmit={handleSubmit}>
              <FieldGroup>
                <Field>
                  <FieldLabel htmlFor="input-username">Username</FieldLabel>
                  <Input
                    id="input-username"
                    required
                    value={username}
                    onChange={(e) => setUsername(e.target.value)}
                    autoComplete="username"
                    autoCapitalize="off"
                    autoCorrect="off"
                    spellCheck={false}
                  />
                </Field>
                {mode === "recover" && (
                  <Field>
                    <FieldLabel htmlFor="input-recovery-key">Recovery key</FieldLabel>
                    <Input
                      id="input-recovery-key"
                      required
                      value={recoveryKey}
                      onChange={(e) => setRecoveryKey(e.target.value)}
                      placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
                      autoComplete="off"
                      autoCapitalize="characters"
                      autoCorrect="off"
                      spellCheck={false}
                      className="font-mono text-sm"
                    />
                  </Field>
                )}
                <Field>
                  <div className="flex items-baseline justify-between gap-2">
                    <FieldLabel htmlFor="input-password">
                      {mode === "recover" ? "New password" : "Password"}
                    </FieldLabel>
                    {mode === "login" && (
                      <button
                        type="button"
                        className="text-sm text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
                        onClick={() => switchMode("recover")}
                      >
                        Forgot password?
                      </button>
                    )}
                  </div>
                  <Input
                    id="input-password"
                    type="password"
                    required
                    minLength={newPassword ? 8 : undefined}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    autoComplete={newPassword ? "new-password" : "current-password"}
                    aria-describedby={newPassword && strength ? "password-strength" : undefined}
                  />
                  {newPassword && strength && (
                    <PasswordStrengthMeter strength={strength} id="password-strength" />
                  )}
                </Field>
                {newPassword && (
                  <Field>
                    <FieldLabel htmlFor="input-confirm-password">
                      {mode === "recover" ? "Confirm new password" : "Confirm password"}
                    </FieldLabel>
                    <Input
                      id="input-confirm-password"
                      type="password"
                      required
                      value={confirmPassword}
                      onChange={(e) => setConfirmPassword(e.target.value)}
                      autoComplete="new-password"
                    />
                  </Field>
                )}
                {editingServer && (
                  <Field>
                    <FieldLabel htmlFor="input-server">Server</FieldLabel>
                    <Input
                      id="input-server"
                      type="url"
                      required
                      value={serverUrl}
                      onChange={(e) => setServerUrl(e.target.value)}
                      autoComplete="url"
                      className="font-mono text-sm"
                    />
                    <FieldDescription>
                      Only for a relay you run yourself. Your account and groups live on it.
                      {serverUrl.trim() !== DEFAULT_SERVER && (
                        <>
                          {" "}
                          <button
                            type="button"
                            className="underline underline-offset-4 hover:text-foreground"
                            onClick={() => setServerUrl(DEFAULT_SERVER)}
                          >
                            Use the default server
                          </button>
                        </>
                      )}
                    </FieldDescription>
                  </Field>
                )}

                {mode === "signup" && (
                  <Alert>
                    <KeyRoundIcon />
                    <AlertDescription>
                      Nobody can reset your password, not even the server. You'll get a recovery key
                      next: it's the way back in if you forget it.
                    </AlertDescription>
                  </Alert>
                )}

                <FieldError>{error}</FieldError>

                <Button
                  type="submit"
                  disabled={submitting || (newPassword && !strength?.acceptable)}
                  className="w-full"
                >
                  {submitting && <Spinner data-icon="inline-start" />}
                  {mode === "login"
                    ? "Log In"
                    : mode === "signup"
                      ? "Create Account"
                      : "Reset Password"}
                </Button>

                {mode === "recover" && (
                  <Button type="button" variant="ghost" onClick={() => switchMode("login")}>
                    Back to log in
                  </Button>
                )}

                {!editingServer && (
                  <p className="text-center text-sm text-muted-foreground">
                    Server: <span className="font-mono">{serverName(serverUrl)}</span> ·{" "}
                    <button
                      type="button"
                      className="underline underline-offset-4 hover:text-foreground"
                      onClick={() => setEditingServer(true)}
                      aria-label="Change server"
                    >
                      Change
                    </button>
                  </p>
                )}
              </FieldGroup>
            </form>
          </CardContent>
        </Card>
      </div>
    </main>
  );
};
