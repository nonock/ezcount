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
import {
  Field,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { CURRENCIES } from "@/utils/currencies";
import { errorMessage } from "@/utils/errors";
import { PlusIcon, XIcon } from "lucide-react";
import type React from "react";
import { useEffect, useState } from "react";

interface CreateGroupModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** The first participant is the user; the backend records them as "you". */
  onCreateGroup: (name: string, currency: string, participants: string[]) => Promise<void>;
  /** Suggested name for the user. */
  ownName: string;
}

interface ParticipantField {
  id: string;
  name: string;
}

const SELF_ID = "p-self";

const initialParticipants = (ownName = ""): ParticipantField[] => [
  { id: SELF_ID, name: ownName },
  { id: "p-init-2", name: "" },
  { id: "p-init-3", name: "" },
];

export const CreateGroupModal: React.FC<CreateGroupModalProps> = ({
  isOpen,
  onClose,
  onCreateGroup,
  ownName,
}) => {
  const [name, setName] = useState("");
  const [currency, setCurrency] = useState("EUR");
  const [participants, setParticipants] = useState<ParticipantField[]>(initialParticipants);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Suggest the user's name, without overwriting what they typed.
  useEffect(() => {
    if (!isOpen) return;
    setParticipants((current) =>
      current.map((p) => (p.id === SELF_ID && !p.name.trim() ? { ...p, name: ownName } : p))
    );
  }, [isOpen, ownName]);

  const handleAddParticipantField = () => {
    setParticipants([...participants, { id: `p-${Date.now()}-${Math.random()}`, name: "" }]);
  };

  const handleParticipantChange = (id: string, value: string) => {
    setParticipants(participants.map((p) => (p.id === id ? { ...p, name: value } : p)));
  };

  const handleRemoveParticipantField = (id: string) => {
    if (id === SELF_ID) return;
    setParticipants(participants.filter((p) => p.id !== id));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedName = name.trim();
    // The user's own name stays first.
    const validParticipants = participants.map((p) => p.name.trim()).filter((p) => p.length > 0);

    if (!trimmedName) {
      setError("Please enter a group name.");
      return;
    }
    if (!participants[0]?.name.trim()) {
      setError("Please enter your name.");
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      await onCreateGroup(trimmedName, currency, validParticipants);
      setName("");
      setCurrency("EUR");
      setParticipants(initialParticipants(ownName));
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Create New Group</DialogTitle>
          <DialogDescription>
            A trip, a flatshare, a dinner… You can add more people later.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="input-group-name">Group name</FieldLabel>
              <Input
                id="input-group-name"
                required
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. Summer Vacation, Roommates"
              />
            </Field>

            <Field>
              <FieldLabel htmlFor="select-group-currency">Currency</FieldLabel>
              <Select value={currency} onValueChange={setCurrency}>
                <SelectTrigger id="select-group-currency" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent position="popper">
                  {CURRENCIES.map((c) => (
                    <SelectItem key={c.code} value={c.code}>
                      {c.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </Field>

            <FieldSet>
              <FieldLegend variant="label">Participants</FieldLegend>
              <div className="space-y-2">
                {participants.map((p, idx) => (
                  <div key={p.id} className="flex items-center gap-2">
                    <Input
                      value={p.name}
                      onChange={(e) => handleParticipantChange(p.id, e.target.value)}
                      placeholder={p.id === SELF_ID ? "Your name" : `Participant ${idx + 1}`}
                      aria-label={p.id === SELF_ID ? "Your name" : `Participant ${idx + 1}`}
                    />
                    {p.id === SELF_ID ? (
                      <span className="w-9 shrink-0 text-center text-xs text-muted-foreground">
                        You
                      </span>
                    ) : (
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        onClick={() => handleRemoveParticipantField(p.id)}
                        aria-label={`Remove participant ${idx + 1}`}
                      >
                        <XIcon />
                      </Button>
                    )}
                  </div>
                ))}
              </div>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="self-start"
                onClick={handleAddParticipantField}
              >
                <PlusIcon data-icon="inline-start" />
                Add person
              </Button>
            </FieldSet>

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
              Create Group
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
};
