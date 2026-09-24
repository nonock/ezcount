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
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from "@/components/ui/input-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import type { Group } from "@/types";
import { errorMessage } from "@/utils/errors";
import type React from "react";
import { useEffect, useRef, useState } from "react";

interface RecordReimbursementModalProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group;
  initialFromId?: string;
  initialToId?: string;
  initialAmount?: string;
  onRecordReimbursement: (
    fromId: string,
    toId: string,
    amountCents: number,
    notes?: string
  ) => Promise<void>;
}

export const RecordReimbursementModal: React.FC<RecordReimbursementModalProps> = ({
  isOpen,
  onClose,
  group,
  initialFromId,
  initialToId,
  initialAmount,
  onRecordReimbursement,
}) => {
  const [fromId, setFromId] = useState("");
  const [toId, setToId] = useState("");
  const [amountStr, setAmountStr] = useState("");
  const [notes, setNotes] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Initialize once per opening so a background sync refreshing the group keeps the form intact.
  const initialized = useRef(false);
  useEffect(() => {
    if (!isOpen) {
      initialized.current = false;
      return;
    }
    if (initialized.current || group.participants.length === 0) return;
    initialized.current = true;

    const active = group.participants.filter((p) => !p.removed);
    const pool = active.length > 0 ? active : group.participants;
    const p1 = pool[0].id;
    const p2 = pool.length > 1 ? pool[1].id : p1;

    setFromId(initialFromId || p1);
    setToId(initialToId || (initialFromId === p1 ? p2 : p1));
    setAmountStr(initialAmount || "");
    setNotes("");
    setError(null);
  }, [isOpen, group, initialFromId, initialToId, initialAmount]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (fromId === toId) {
      setError("The sender and recipient cannot be the same person.");
      return;
    }

    const amountDecimal = Number.parseFloat(amountStr);
    const amountCents = !Number.isNaN(amountDecimal) ? Math.round(amountDecimal * 100) : 0;
    if (amountCents <= 0) {
      setError("Please enter an amount greater than zero.");
      return;
    }

    setSubmitting(true);
    setError(null);
    try {
      await onRecordReimbursement(fromId, toId, amountCents, notes.trim() || undefined);
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  };

  const options = (
    <SelectContent position="popper">
      {group.participants.map((p) => (
        <SelectItem key={p.id} value={p.id}>
          {p.name}
          {p.removed ? " (removed)" : ""}
        </SelectItem>
      ))}
    </SelectContent>
  );

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Record Reimbursement</DialogTitle>
          <DialogDescription>Record money paid back between two members.</DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Field>
                <FieldLabel htmlFor="select-reimburse-from">From (sender)</FieldLabel>
                <Select value={fromId} onValueChange={setFromId}>
                  <SelectTrigger id="select-reimburse-from" className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  {options}
                </Select>
              </Field>
              <Field>
                <FieldLabel htmlFor="select-reimburse-to">To (recipient)</FieldLabel>
                <Select value={toId} onValueChange={setToId}>
                  <SelectTrigger id="select-reimburse-to" className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  {options}
                </Select>
              </Field>
            </div>

            <Field>
              <FieldLabel htmlFor="input-reimburse-amount">Amount</FieldLabel>
              <InputGroup>
                <InputGroupInput
                  id="input-reimburse-amount"
                  type="number"
                  inputMode="decimal"
                  step="0.01"
                  min="0.01"
                  placeholder="0.00"
                  required
                  value={amountStr}
                  onChange={(e) => setAmountStr(e.target.value)}
                  className="tabular-nums"
                />
                <InputGroupAddon align="inline-end">
                  <InputGroupText>{group.currency}</InputGroupText>
                </InputGroupAddon>
              </InputGroup>
            </Field>

            <Field>
              <FieldLabel htmlFor="input-reimburse-notes">Note (optional)</FieldLabel>
              <Input
                id="input-reimburse-notes"
                placeholder="e.g. Bank transfer, Cash"
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
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
              Confirm Payment
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
};
