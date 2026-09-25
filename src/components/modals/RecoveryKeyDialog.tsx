import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { api } from "@/services/api";
import { errorMessage } from "@/utils/errors";
import { CopyIcon } from "lucide-react";
import type React from "react";
import { useEffect, useState } from "react";
import { toast } from "sonner";

/** Why a recovery key is on screen. */
export type RecoveryKeyReason = "signup" | "recovered" | "replace";

const INTROS: Record<RecoveryKeyReason, string> = {
  signup:
    "If you forget your password, this key is the only way back into your account: nobody can reset it for you. Keep it in a password manager, or write it down.",
  recovered:
    "Your new password is set. The recovery key you used no longer works, so here is a new one to keep.",
  replace: "Your previous recovery key no longer works. Keep this one instead.",
};

interface RecoveryKeyDialogProps {
  isOpen: boolean;
  onClose: () => void;
  reason: RecoveryKeyReason;
  /** The key to show. Without it (reason "replace"), the dialog asks for the password first. */
  recoveryKey?: string | null;
}

/**
 * Shows a recovery key once, and stays up until the user confirms they saved it. For
 * "replace", it first makes a new key, which needs the password.
 */
export const RecoveryKeyDialog: React.FC<RecoveryKeyDialogProps> = ({
  isOpen,
  onClose,
  reason,
  recoveryKey = null,
}) => {
  const [key, setKey] = useState<string | null>(recoveryKey);
  const [password, setPassword] = useState("");
  const [saved, setSaved] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setKey(recoveryKey);
    setPassword("");
    setSaved(false);
    setError(null);
  }, [isOpen, recoveryKey]);

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      setKey(await api.replaceRecoveryKey(password));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  };

  const handleCopy = async () => {
    if (!key) return;
    try {
      await navigator.clipboard.writeText(key);
      toast.success("Recovery key copied");
    } catch {
      toast.info("Select the key and copy it manually");
    }
  };

  // Once a key is on screen, only "Done" closes the dialog: it won't be shown again.
  const showingKey = key !== null;
  const blockDismiss = (e: Event) => showingKey && e.preventDefault();

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && !showingKey && onClose()}>
      <DialogContent
        className="sm:max-w-md"
        showCloseButton={!showingKey}
        onEscapeKeyDown={blockDismiss}
        onPointerDownOutside={blockDismiss}
      >
        <DialogHeader>
          <DialogTitle>{showingKey ? "Save your recovery key" : "New recovery key"}</DialogTitle>
          <DialogDescription>
            {showingKey
              ? INTROS[reason]
              : "A new key replaces your current one, which stops working. Enter your password to continue."}
          </DialogDescription>
        </DialogHeader>

        {showingKey ? (
          <>
            <div className="space-y-3">
              <p
                className="rounded-lg border bg-muted/50 px-3 py-4 text-center font-mono text-base font-semibold tracking-wide break-all select-all"
                aria-label="Recovery key"
              >
                {key}
              </p>
              <Button variant="outline" className="w-full" onClick={handleCopy}>
                <CopyIcon data-icon="inline-start" />
                Copy Key
              </Button>
              <div className="flex items-center gap-2.5 pt-1">
                <Checkbox
                  id="recovery-key-saved"
                  checked={saved}
                  onCheckedChange={(checked) => setSaved(checked === true)}
                />
                <Label htmlFor="recovery-key-saved" className="font-normal">
                  I've saved it somewhere safe
                </Label>
              </div>
            </div>
            <DialogFooter>
              <Button onClick={onClose} disabled={!saved}>
                Done
              </Button>
            </DialogFooter>
          </>
        ) : (
          <form onSubmit={handleCreate}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="recovery-password">Password</FieldLabel>
                <Input
                  id="recovery-password"
                  type="password"
                  required
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  autoComplete="current-password"
                />
              </Field>
              <FieldError>{error}</FieldError>
            </FieldGroup>
            <DialogFooter className="mt-6">
              <Button type="button" variant="outline" onClick={onClose}>
                Cancel
              </Button>
              <Button type="submit" disabled={submitting}>
                {submitting && <Spinner data-icon="inline-start" />}
                Create New Key
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
};
