import { LogoMark, Wordmark } from "@/components/common/Logo";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { api } from "@/services/api";
import type { AccountInfo, PasswordStrength } from "@/types";
import { errorMessage } from "@/utils/errors";
import { serverName } from "@/utils/formatters";
import { LockIcon } from "lucide-react";
import type React from "react";
import { useEffect, useState } from "react";
import { PasswordStrengthMeter } from "./PasswordStrengthMeter";

type Mode = "login" | "signup";

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

interface AuthScreenProps {
  onAuthenticated: (account: AccountInfo) => void;
}

export const AuthScreen: React.FC<AuthScreenProps> = ({ onAuthenticated }) => {
  const [mode, setMode] = useState<Mode>("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [serverUrl, setServerUrl] = useState(rememberedServer);
  // Most people use the default relay, so the field stays out of the way until asked for.
  const [editingServer, setEditingServer] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const signingUp = mode === "signup";

  // Rated as the user types, by the same check sign-up enforces.
  const [strength, setStrength] = useState<PasswordStrength | null>(null);
  useEffect(() => {
    if (!signingUp || !password) {
      setStrength(null);
      return;
    }
    let current = true;
    api
      .passwordStrength(password, username)
      .then((s) => current && setStrength(s))
      .catch((err) => console.error("Could not rate the password:", err));
    return () => {
      current = false;
    };
  }, [signingUp, password, username]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (signingUp && password !== confirmPassword) {
      setError("The passwords don't match.");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const server = serverUrl.trim();
      const account = signingUp
        ? await api.signUp(server, username, password)
        : await api.logIn(server, username, password);
      rememberServer(server);
      onAuthenticated(account);
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
              <h1>{signingUp ? "Create your account" : "Welcome back"}</h1>
            </CardTitle>
            <CardDescription>
              {signingUp
                ? "One account for your phone, your computer and every group."
                : "Log in to get your groups on this device."}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <Tabs
              value={mode}
              onValueChange={(value) => {
                setMode(value as Mode);
                setError(null);
              }}
            >
              <TabsList className="w-full">
                <TabsTrigger value="login">Log in</TabsTrigger>
                <TabsTrigger value="signup">Sign up</TabsTrigger>
              </TabsList>
            </Tabs>

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
                <Field>
                  <FieldLabel htmlFor="input-password">Password</FieldLabel>
                  <Input
                    id="input-password"
                    type="password"
                    required
                    minLength={signingUp ? 8 : undefined}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    autoComplete={signingUp ? "new-password" : "current-password"}
                    aria-describedby={signingUp && strength ? "password-strength" : undefined}
                  />
                  {signingUp && strength && (
                    <PasswordStrengthMeter strength={strength} id="password-strength" />
                  )}
                </Field>
                {signingUp && (
                  <Field>
                    <FieldLabel htmlFor="input-confirm-password">Confirm password</FieldLabel>
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

                {signingUp && (
                  <Alert>
                    <LockIcon />
                    <AlertDescription>
                      Your password encrypts your data, so it can't be reset. Keep it in a password
                      manager.
                    </AlertDescription>
                  </Alert>
                )}

                <FieldError>{error}</FieldError>

                <Button
                  type="submit"
                  disabled={submitting || (signingUp && !strength?.acceptable)}
                  className="w-full"
                >
                  {submitting && <Spinner data-icon="inline-start" />}
                  {signingUp ? "Create Account" : "Log In"}
                </Button>

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
