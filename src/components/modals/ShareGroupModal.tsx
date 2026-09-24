import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
import type { Group, SyncInfo } from "@/types";
import { formatDateTime } from "@/utils/formatters";
import { CopyIcon, LockIcon, RefreshCwIcon, TriangleAlertIcon } from "lucide-react";
import type React from "react";
import { useState } from "react";
import { toast } from "sonner";

interface ShareGroupModalProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group;
  syncInfo: SyncInfo | null;
  onSyncNow: () => Promise<void>;
}

export const ShareGroupModal: React.FC<ShareGroupModalProps> = ({
  isOpen,
  onClose,
  group,
  syncInfo,
  onSyncNow,
}) => {
  const [busy, setBusy] = useState(false);

  const handleSyncNow = async () => {
    setBusy(true);
    try {
      await onSyncNow();
    } finally {
      setBusy(false);
    }
  };

  const handleCopy = async () => {
    if (!syncInfo?.invite_code) return;
    try {
      await navigator.clipboard.writeText(syncInfo.invite_code);
      toast.success("Invite code copied");
    } catch {
      document.getElementById("share-invite-code")?.focus();
      toast.info("Select the code and copy it manually");
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Invite to Group</DialogTitle>
          <DialogDescription>
            Other members join "{group.name}" with this invite code, from their own account.
            Everyone can edit, even offline; changes merge when devices reconnect.
          </DialogDescription>
        </DialogHeader>

        {!syncInfo?.invite_code ? (
          <div className="flex items-center justify-center gap-2 py-6 text-sm text-muted-foreground">
            <Spinner /> Loading…
          </div>
        ) : (
          <>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="share-invite-code">Invite code</FieldLabel>
                <Textarea
                  id="share-invite-code"
                  readOnly
                  rows={3}
                  value={syncInfo.invite_code}
                  onFocus={(e) => e.currentTarget.select()}
                  className="resize-none font-mono text-xs break-all"
                />
                <FieldDescription className="flex items-start gap-1.5">
                  <LockIcon className="mt-0.5 size-3.5 shrink-0" aria-hidden />
                  Anyone with this code can see and edit the group, so share it only with members.
                  Changes are end-to-end encrypted: the sync server cannot read them.
                </FieldDescription>
              </Field>

              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
                <dt className="text-muted-foreground">Server</dt>
                <dd className="truncate text-right font-mono">{syncInfo.server_url}</dd>
                <dt className="text-muted-foreground">Last synced</dt>
                <dd className="text-right">
                  {syncInfo.last_synced_at ? formatDateTime(syncInfo.last_synced_at) : "Never"}
                </dd>
              </dl>

              {syncInfo.last_error && (
                <Alert variant="destructive">
                  <TriangleAlertIcon />
                  <AlertDescription>Last sync failed: {syncInfo.last_error}</AlertDescription>
                </Alert>
              )}
            </FieldGroup>

            <DialogFooter>
              <Button variant="outline" onClick={handleSyncNow} disabled={busy}>
                {busy ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <RefreshCwIcon data-icon="inline-start" />
                )}
                Sync Now
              </Button>
              <Button onClick={handleCopy}>
                <CopyIcon data-icon="inline-start" />
                Copy Invite Code
              </Button>
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
};
