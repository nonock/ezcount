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
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import type { Participant } from "@/types";
import { errorMessage } from "@/utils/errors";
import type React from "react";
import { useEffect, useState } from "react";

interface MemberModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** The member to rename; adds a new one when absent. */
  member?: Participant | null;
  onSubmit: (name: string) => Promise<void>;
}

/** Adds a member to the group, or renames one. */
export const MemberModal: React.FC<MemberModalProps> = ({ isOpen, onClose, member, onSubmit }) => {
  const [name, setName] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setName(member?.name ?? "");
    setError(null);
  }, [isOpen, member]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) return;

    setSubmitting(true);
    setError(null);
    try {
      await onSubmit(trimmed);
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{member ? `Rename ${member.name}` : "Add Group Member"}</DialogTitle>
          <DialogDescription>
            {member
              ? "Their expenses and payments show the new name too."
              : "They can be included in expenses right away."}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="input-member-name">Name</FieldLabel>
              <Input
                id="input-member-name"
                required
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. David"
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
              {member ? "Rename" : "Add to Group"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
};
