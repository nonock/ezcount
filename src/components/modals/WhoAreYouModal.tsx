import { Avatar, AvatarFallback } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
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
import { Spinner } from "@/components/ui/spinner";
import type { Group } from "@/types";
import { errorMessage } from "@/utils/errors";
import { CheckIcon, UserPlusIcon } from "lucide-react";
import type React from "react";
import { useEffect, useState } from "react";

interface WhoAreYouModalProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group;
  currentUserId: string | null;
  /** Suggested name when adding yourself. */
  defaultName: string;
  onChoose: (participantId: string) => Promise<void>;
  onAddSelf: (name: string) => Promise<void>;
}

/** Asks which participant the user is. The answer is saved in their account. */
export const WhoAreYouModal: React.FC<WhoAreYouModalProps> = ({
  isOpen,
  onClose,
  group,
  currentUserId,
  defaultName,
  onChoose,
  onAddSelf,
}) => {
  const [addingSelf, setAddingSelf] = useState(false);
  const [name, setName] = useState("");
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setAddingSelf(false);
    setName(defaultName);
    setBusyId(null);
    setError(null);
  }, [isOpen, defaultName]);

  const run = async (id: string, action: () => Promise<void>) => {
    setBusyId(id);
    setError(null);
    try {
      await action();
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusyId(null);
    }
  };

  const handleAddSelf = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = name.trim();
    if (trimmed) run("new", () => onAddSelf(trimmed));
  };

  const people = group.participants.filter((p) => !p.removed);

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>Who are you in "{group.name}"?</DialogTitle>
          <DialogDescription>
            Your balance and "Paid by you" follow this choice on all your devices.
          </DialogDescription>
        </DialogHeader>

        {!addingSelf ? (
          <>
            <ul className="space-y-1.5" aria-label="Members">
              {people.map((p) => (
                <li key={p.id}>
                  <Button
                    variant={p.id === currentUserId ? "secondary" : "outline"}
                    className="h-11 w-full justify-start gap-3"
                    disabled={busyId !== null}
                    onClick={() => run(p.id, () => onChoose(p.id))}
                  >
                    <Avatar size="sm" aria-hidden>
                      <AvatarFallback>{p.name.slice(0, 1).toUpperCase()}</AvatarFallback>
                    </Avatar>
                    <span className="truncate">{p.name}</span>
                    {busyId === p.id ? (
                      <Spinner className="ml-auto" />
                    ) : (
                      p.id === currentUserId && <CheckIcon className="ml-auto" aria-label="You" />
                    )}
                  </Button>
                </li>
              ))}
            </ul>
            <FieldError>{error}</FieldError>
            <DialogFooter>
              <Button variant="ghost" onClick={() => setAddingSelf(true)}>
                <UserPlusIcon data-icon="inline-start" />
                I'm not in the list
              </Button>
            </DialogFooter>
          </>
        ) : (
          <form onSubmit={handleAddSelf}>
            <FieldGroup>
              <Field>
                <FieldLabel htmlFor="input-self-name">Your name in this group</FieldLabel>
                <Input
                  id="input-self-name"
                  required
                  autoFocus
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                />
              </Field>
              <FieldError>{error}</FieldError>
            </FieldGroup>
            <DialogFooter className="mt-6">
              <Button type="button" variant="outline" onClick={() => setAddingSelf(false)}>
                Back
              </Button>
              <Button type="submit" disabled={busyId !== null}>
                {busyId === "new" && <Spinner data-icon="inline-start" />}
                Add Me
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
};
