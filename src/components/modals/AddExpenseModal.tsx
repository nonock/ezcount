import { DatePicker } from "@/components/common/DatePicker";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
  InputGroupText,
} from "@/components/ui/input-group";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";
import type { Expense, ExpenseSplit, Group } from "@/types";
import { errorMessage } from "@/utils/errors";
import { formatDateInput, formatMoney } from "@/utils/formatters";
import { MinusIcon, PlusIcon } from "lucide-react";
import type React from "react";
import { useEffect, useMemo, useRef, useState } from "react";

interface AddExpenseModalProps {
  isOpen: boolean;
  onClose: () => void;
  group: Group;
  onAddExpense: (
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[],
    createdAt?: string | null
  ) => Promise<void>;
  editingExpense?: Expense | null;
  onUpdateExpense?: (
    expenseId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[],
    createdAt?: string | null
  ) => Promise<void>;
  currentUserId?: string | null;
}

interface SplitItemState {
  included: boolean;
  shares: number;
}

export const AddExpenseModal: React.FC<AddExpenseModalProps> = ({
  isOpen,
  onClose,
  group,
  onAddExpense,
  editingExpense,
  onUpdateExpense,
  currentUserId,
}) => {
  const [title, setTitle] = useState("");
  const [amountStr, setAmountStr] = useState("");
  const [paidBy, setPaidBy] = useState("");
  const [expenseDate, setExpenseDate] = useState<string>(formatDateInput());
  const [splitsState, setSplitsState] = useState<Record<string, SplitItemState>>({});
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Removed members stay selectable on expenses they are already part of.
  const participants = useMemo(() => {
    const involved = new Set(
      editingExpense
        ? [editingExpense.paid_by, ...editingExpense.splits.map((s) => s.participant_id)]
        : []
    );
    return group.participants.filter((p) => !p.removed || involved.has(p.id));
  }, [group.participants, editingExpense]);

  // Initialize form state once per opening (or when switching to another expense), so a
  // background sync refreshing the group doesn't wipe what the user is typing.
  const initializedFor = useRef<string | null>(null);
  useEffect(() => {
    if (!isOpen) {
      initializedFor.current = null;
      return;
    }
    const key = editingExpense?.id ?? "new";
    if (initializedFor.current === key) return;
    initializedFor.current = key;
    setError(null);

    if (editingExpense) {
      setTitle(editingExpense.title);
      setAmountStr((editingExpense.amount_cents / 100).toFixed(2));
      setPaidBy(editingExpense.paid_by);
      setExpenseDate(formatDateInput(editingExpense.created_at));

      const initialSplits: Record<string, SplitItemState> = {};
      for (const p of participants) {
        const match = editingExpense.splits.find((s) => s.participant_id === p.id);
        initialSplits[p.id] = {
          included: !!match,
          shares: match ? match.shares : 1,
        };
      }
      setSplitsState(initialSplits);
    } else {
      setTitle("");
      setAmountStr("");
      setExpenseDate(formatDateInput());
      const defaultPayer =
        currentUserId && participants.some((p) => p.id === currentUserId)
          ? currentUserId
          : participants[0]?.id || "";
      setPaidBy(defaultPayer);
      const initialSplits: Record<string, SplitItemState> = {};
      for (const p of participants) {
        initialSplits[p.id] = { included: true, shares: 1 };
      }
      setSplitsState(initialSplits);
    }
  }, [isOpen, editingExpense, participants, currentUserId]);

  const handleToggleParticipant = (id: string) => {
    setSplitsState((prev) => ({
      ...prev,
      [id]: {
        included: !prev[id]?.included,
        shares: prev[id]?.shares || 1,
      },
    }));
  };

  const handleUpdateShares = (id: string, newShares: number) => {
    if (newShares < 1) return;
    setSplitsState((prev) => ({
      ...prev,
      [id]: {
        ...prev[id],
        shares: newShares,
      },
    }));
  };

  const includedParticipants = participants.filter((p) => splitsState[p.id]?.included);
  const totalShares = includedParticipants.reduce(
    (sum, p) => sum + (splitsState[p.id]?.shares || 1),
    0
  );
  const allIncluded = includedParticipants.length === participants.length;

  const handleToggleAll = () => {
    setSplitsState((prev) => {
      const next: Record<string, SplitItemState> = {};
      for (const p of participants) {
        next[p.id] = {
          included: !allIncluded,
          shares: prev[p.id]?.shares || 1,
        };
      }
      return next;
    });
  };

  const amountDecimal = Number.parseFloat(amountStr);
  const amountCents = !Number.isNaN(amountDecimal) ? Math.round(amountDecimal * 100) : 0;
  const perShareCents = totalShares > 0 ? Math.floor(amountCents / totalShares) : 0;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedTitle = title.trim();

    if (!trimmedTitle) {
      setError("Please enter a description.");
      return;
    }
    if (amountCents <= 0) {
      setError("Please enter an amount greater than zero.");
      return;
    }
    if (includedParticipants.length === 0) {
      setError("Select at least one person to split the bill with.");
      return;
    }

    const splits: ExpenseSplit[] = includedParticipants.map((p) => ({
      participant_id: p.id,
      shares: splitsState[p.id]?.shares || 1,
    }));

    let createdAtIso: string | null = null;
    if (expenseDate) {
      const [year, month, day] = expenseDate.split("-").map(Number);
      const d = new Date();
      d.setFullYear(year, month - 1, day);
      createdAtIso = d.toISOString();
    }

    setSubmitting(true);
    setError(null);
    try {
      if (editingExpense && onUpdateExpense) {
        await onUpdateExpense(
          editingExpense.id,
          trimmedTitle,
          amountCents,
          paidBy,
          splits,
          createdAtIso
        );
      } else {
        await onAddExpense(trimmedTitle, amountCents, paidBy, splits, createdAtIso);
      }
      onClose();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  };

  const isEditing = Boolean(editingExpense);

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[calc(100dvh-2rem)] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{isEditing ? "Edit Expense" : "Add New Expense"}</DialogTitle>
          <DialogDescription>
            {isEditing
              ? "Changes are recorded in the expense's history."
              : "Who paid, how much, and who it was for."}
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <Field>
              <FieldLabel htmlFor="input-expense-title">Description</FieldLabel>
              <Input
                id="input-expense-title"
                required
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="e.g. Groceries, Dinner, Taxi"
              />
            </Field>

            <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
              <Field>
                <FieldLabel htmlFor="input-expense-amount">Amount</FieldLabel>
                <InputGroup>
                  <InputGroupInput
                    id="input-expense-amount"
                    type="number"
                    inputMode="decimal"
                    step="0.01"
                    min="0.01"
                    required
                    value={amountStr}
                    onChange={(e) => setAmountStr(e.target.value)}
                    placeholder="0.00"
                    className="tabular-nums"
                  />
                  <InputGroupAddon align="inline-end">
                    <InputGroupText>{group.currency}</InputGroupText>
                  </InputGroupAddon>
                </InputGroup>
              </Field>

              <Field>
                <FieldLabel id="label-expense-date" htmlFor="input-expense-date">
                  Date
                </FieldLabel>
                <DatePicker
                  id="input-expense-date"
                  labelId="label-expense-date"
                  value={expenseDate}
                  onChange={setExpenseDate}
                />
              </Field>

              <Field className="col-span-2 sm:col-span-1">
                <FieldLabel htmlFor="select-expense-payer">Paid by</FieldLabel>
                <Select value={paidBy} onValueChange={setPaidBy}>
                  <SelectTrigger id="select-expense-payer" className="w-full">
                    <SelectValue placeholder="Choose…" />
                  </SelectTrigger>
                  <SelectContent position="popper">
                    {participants.map((p) => (
                      <SelectItem key={p.id} value={p.id}>
                        {p.name}
                        {p.id === currentUserId ? " (You)" : ""}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>
            </div>

            <FieldSet>
              <div className="flex items-center justify-between">
                <FieldLegend variant="label" className="mb-0">
                  Split between
                </FieldLegend>
                <Button type="button" variant="link" size="sm" onClick={handleToggleAll}>
                  {allIncluded ? "Deselect all" : "Select all"}
                </Button>
              </div>
              <FieldDescription>
                {includedParticipants.length}/{participants.length} people, {totalShares}{" "}
                {totalShares === 1 ? "part" : "parts"}
                {amountCents > 0 && totalShares > 0 && (
                  <> · about {formatMoney(perShareCents, group.currency)} per part</>
                )}
              </FieldDescription>

              <ul className="divide-y rounded-lg border">
                {participants.map((p) => {
                  const state = splitsState[p.id] || { included: false, shares: 1 };
                  const owed =
                    totalShares > 0 && amountCents > 0
                      ? Math.round((amountCents * state.shares) / totalShares)
                      : 0;
                  const checkboxId = `split-${p.id}`;

                  return (
                    <li
                      key={p.id}
                      className={cn(
                        "flex min-h-11 items-center justify-between gap-2 px-3 py-1.5",
                        !state.included && "text-muted-foreground"
                      )}
                    >
                      <div className="flex items-center gap-2.5">
                        <Checkbox
                          id={checkboxId}
                          checked={state.included}
                          onCheckedChange={() => handleToggleParticipant(p.id)}
                        />
                        <Label htmlFor={checkboxId} className="font-normal">
                          {p.name}
                        </Label>
                      </div>

                      {state.included && (
                        <div className="flex items-center gap-2">
                          {owed > 0 && (
                            <span className="text-sm tabular-nums">
                              {formatMoney(owed, group.currency)}
                            </span>
                          )}
                          <div className="flex items-center rounded-md border">
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon-sm"
                              onClick={() => handleUpdateShares(p.id, state.shares - 1)}
                              disabled={state.shares <= 1}
                              aria-label={`Fewer parts for ${p.name}`}
                            >
                              <MinusIcon />
                            </Button>
                            <span className="min-w-14 text-center text-xs" aria-live="polite">
                              {state.shares} {state.shares === 1 ? "part" : "parts"}
                            </span>
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon-sm"
                              onClick={() => handleUpdateShares(p.id, state.shares + 1)}
                              aria-label={`More parts for ${p.name}`}
                            >
                              <PlusIcon />
                            </Button>
                          </div>
                        </div>
                      )}
                    </li>
                  );
                })}
              </ul>
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
              {isEditing ? "Save Changes" : "Save Expense"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
};
