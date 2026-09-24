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
import { LockIcon } from "lucide-react";
import type React from "react";
import { useState } from "react";

type Mode = "login" | "signup";

const SERVER_KEY = "ezcount_sync_server";
const DEFAULT_SERVER = import.meta.env.VITE_EZCOUNT_SERVER || "http://localhost:8787";

function rememberedServer(): string {
  try {
    return localStorage.getItem(SERVER_KEY) || DEFAULT_SERVER;
  } catch {
    return DEFAULT_SERVER;
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
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const signingUp = mode === "signup";

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
      try {
        localStorage.setItem(SERVER_KEY, server);
      } catch {
        // Remembering the server is only a convenience.
      }
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
          <span className="flex size-10 items-center justify-center rounded-xl bg-primary text-base font-black text-primary-foreground">
            ez
          </span>
          <span className="text-xl font-semibold tracking-tight">ezcount</span>
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
                  />
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
                  <FieldDescription>The ezcount relay that keeps your account.</FieldDescription>
                </Field>

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

                <Button type="submit" disabled={submitting} className="w-full">
                  {submitting && <Spinner data-icon="inline-start" />}
                  {signingUp ? "Create Account" : "Log In"}
                </Button>
              </FieldGroup>
            </form>
          </CardContent>
        </Card>
      </div>
    </main>
  );
};
