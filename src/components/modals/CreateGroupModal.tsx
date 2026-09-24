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
import { errorMessage } from "@/utils/errors";
import { PlusIcon, XIcon } from "lucide-react";
import type React from "react";
import { useState } from "react";

interface CreateGroupModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreateGroup: (name: string, currency: string, participants: string[]) => Promise<void>;
}

interface ParticipantField {
  id: string;
  name: string;
}

const initialParticipants = (): ParticipantField[] => [
  { id: "p-init-1", name: "" },
  { id: "p-init-2", name: "" },
  { id: "p-init-3", name: "" },
];

export const CreateGroupModal: React.FC<CreateGroupModalProps> = ({
  isOpen,
  onClose,
  onCreateGroup,
}) => {
  const [name, setName] = useState("");
  const [currency, setCurrency] = useState("EUR");
  const [participants, setParticipants] = useState<ParticipantField[]>(initialParticipants);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleAddParticipantField = () => {
    setParticipants([...participants, { id: `p-${Date.now()}-${Math.random()}`, name: "" }]);
  };

  const handleParticipantChange = (id: string, value: string) => {
    setParticipants(participants.map((p) => (p.id === id ? { ...p, name: value } : p)));
  };

  const handleRemoveParticipantField = (id: string) => {
    if (participants.length <= 1) return;
    setParticipants(participants.filter((p) => p.id !== id));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedName = name.trim();
    const validParticipants = participants.map((p) => p.name.trim()).filter((p) => p.length > 0);

    if (!trimmedName) {
      setError("Please enter a group name.");
      return;
    }
    if (validParticipants.length === 0) {
      setError("Please enter at least one participant.");
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      await onCreateGroup(trimmedName, currency, validParticipants);
      setName("");
      setCurrency("EUR");
      setParticipants(initialParticipants());
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
                  <SelectItem value="EUR">EUR (€) — Euro</SelectItem>
                  <SelectItem value="USD">USD ($) — US Dollar</SelectItem>
                  <SelectItem value="GBP">GBP (£) — British Pound</SelectItem>
                  <SelectItem value="CHF">CHF — Swiss Franc</SelectItem>
                  <SelectItem value="CAD">CAD (CA$) — Canadian Dollar</SelectItem>
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
                      placeholder={`Participant ${idx + 1}`}
                      aria-label={`Participant ${idx + 1}`}
                    />
                    {participants.length > 1 && (
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
