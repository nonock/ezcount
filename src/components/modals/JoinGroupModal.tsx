import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import { useNativeFeatures } from "@/services/native";
import { errorMessage } from "@/utils/errors";
import { ScanQrCodeIcon } from "lucide-react";
import type React from "react";
import { useEffect, useState } from "react";

interface JoinGroupModalProps {
  isOpen: boolean;
  onClose: () => void;
  onJoinGroup: (inviteCode: string) => Promise<void>;
  /** Pre-fills the invite, say from a link the app was opened with. */
  initialCode?: string;
  initialError?: string | null;
  /** Scans an invite's QR code instead; offered where the device has a camera scanner. */
  onScan: () => void;
}

export const JoinGroupModal: React.FC<JoinGroupModalProps> = ({
  isOpen,
  onClose,
  onJoinGroup,
  initialCode = "",
  initialError = null,
  onScan,
}) => {
  const [code, setCode] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const native = useNativeFeatures();

  useEffect(() => {
    if (!isOpen) return;
    setCode(initialCode);
    setError(initialError);
  }, [isOpen, initialCode, initialError]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!code.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      await onJoinGroup(code.trim());
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Join a Group</DialogTitle>
          <DialogDescription>
            Open the invite link a member sent you, or paste it here. To get one, a member opens the
            group and taps Invite.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <FieldGroup>
            {native.scan && (
              <Button type="button" variant="outline" size="lg" onClick={onScan}>
                <ScanQrCodeIcon data-icon="inline-start" />
                Scan QR Code
              </Button>
            )}
            <Field>
              <FieldLabel htmlFor="input-invite-code">Invite link</FieldLabel>
              <Textarea
                id="input-invite-code"
                required
                rows={3}
                value={code}
                onChange={(e) => setCode(e.target.value)}
                placeholder="https://…/join#…"
                spellCheck={false}
                autoCapitalize="off"
                autoCorrect="off"
                className="resize-none font-mono text-xs break-all"
              />
            </Field>
            <FieldError>{error}</FieldError>
          </FieldGroup>
          <DialogFooter className="mt-6">
            <DialogClose asChild>
              <Button type="button" variant="outline">
                Cancel
              </Button>
            </DialogClose>
            <Button type="submit" disabled={submitting}>
              {submitting && <Spinner data-icon="inline-start" />}
              Join Group
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
};
