// The open group's expenses: the trash and the repeated ones.

import { toast } from "svelte-sonner";
import { api } from "@/services/api";
import { errorMessage } from "@/utils/errors";
import { expenseTitle } from "@/utils/formatters";
import { t } from "../i18n/index.svelte";
import { askConfirm } from "../state/confirm.svelte";
import { openGroup } from "../state/groups.svelte";

/** Moves an expense to the group's trash. It isn't asked first: it can be undone. */
export async function deleteExpense(expenseId: string) {
  const expense = openGroup.group?.expenses.find((e) => e.id === expenseId);
  if (!expense) return;
  try {
    await openGroup.change((groupId) => api.deleteExpense(groupId, expenseId));
    toast.success(t("trash.moved", expenseTitle(expense)), {
      duration: 8000,
      action: { label: t("trash.undo"), onClick: () => restoreExpense(expenseId) },
    });
  } catch (err) {
    toast.error(t("expenses.deleteFailed"), { description: errorMessage(err) });
  }
}

/** Puts a deleted expense back among the others. */
export async function restoreExpense(expenseId: string) {
  try {
    await openGroup.change((groupId) => api.restoreExpense(groupId, expenseId));
  } catch (err) {
    toast.error(t("trash.restoreFailed"), { description: errorMessage(err) });
  }
}

/** Removes a deleted expense from the trash, for every member and for good. */
export async function purgeExpense(expenseId: string) {
  const deleted = openGroup.group?.trash?.find((d) => d.expense.id === expenseId);
  if (!deleted) return;
  const confirmed = await askConfirm({
    title: t("trash.purgeTitle", expenseTitle(deleted.expense)),
    description: t("trash.purgeDescription"),
    confirmLabel: t("trash.purgeConfirm"),
    destructive: true,
  });
  if (!confirmed) return;
  try {
    await openGroup.change((groupId) => api.purgeExpense(groupId, expenseId));
  } catch (err) {
    toast.error(t("expenses.deleteFailed"), { description: errorMessage(err) });
  }
}

/** Stops an expense from coming back. The ones already added stay. */
export async function stopRecurring(recurringId: string) {
  const model = openGroup.group?.recurring?.find((r) => r.id === recurringId);
  if (!model) return;
  const confirmed = await askConfirm({
    title: t("recurring.stopTitle", model.title),
    description: t("recurring.stopDescription"),
    confirmLabel: t("recurring.stop"),
  });
  if (!confirmed) return;
  try {
    await openGroup.change((groupId) => api.stopRecurringExpense(groupId, recurringId));
  } catch (err) {
    toast.error(t("recurring.stopFailed"), { description: errorMessage(err) });
  }
}
