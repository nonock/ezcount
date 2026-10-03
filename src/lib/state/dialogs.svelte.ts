// Which dialog is open, and what it opens with.

import { SvelteSet } from "svelte/reactivity";
import type { Expense, Participant } from "@/types";

class Dialogs {
  /** The camera is scanning an invite's QR code. */
  scanning = $state(false);
  /** Groups where the user closed "Who are you?" without answering, this session. */
  identitySkipped = new SvelteSet<string>();
  createGroup = $state(false);
  /** Pre-filled with an invite from a link or a scan, and why joining with it failed. */
  join = $state({ open: false, code: "", error: null as string | null });
  addMember = $state(false);
  // The member stays set after closing, so the dialog doesn't change while it animates out.
  renameMember = $state({ open: false, member: null as Participant | null });
  editGroup = $state(false);
  /** Adds an expense, or edits `editing`. */
  expense = $state({ open: false, editing: null as Expense | null });
  history = $state<Expense | null>(null);
  reimburse = $state({ open: false, fromId: "", toId: "", amount: "" });
  share = $state(false);
  who = $state(false);
  /** The account: who is logged in, and what can be done with it. */
  account = $state(false);
  changePassword = $state(false);
  newRecoveryKey = $state(false);
  /** Shows a code that logs another device in. */
  linkDevice = $state(false);

  openJoin(code = "", error: string | null = null) {
    this.join = { open: true, code, error };
  }

  openExpense(editing: Expense | null = null) {
    this.expense = { open: true, editing };
  }

  openReimburse(prefill: { fromId?: string; toId?: string; amount?: string } = {}) {
    this.reimburse = {
      open: true,
      fromId: prefill.fromId ?? "",
      toId: prefill.toId ?? "",
      amount: prefill.amount ?? "",
    };
  }
}

export const dialogs = new Dialogs();
