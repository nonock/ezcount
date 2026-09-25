import { useConfirm } from "@/components/common/ConfirmDialog";
import { Splash } from "@/components/common/Splash";
import { Alert, AlertAction, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { errorMessage } from "@/utils/errors";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowLeftIcon,
  ArrowLeftRightIcon,
  ReceiptTextIcon,
  ScaleIcon,
  TriangleAlertIcon,
} from "lucide-react";
import type React from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { AuthScreen, type NewRecoveryKey } from "./components/auth/AuthScreen";
import { Navbar } from "./components/common/Navbar";
import { QrScanOverlay } from "./components/common/QrScanOverlay";
import { GroupDashboard } from "./components/dashboard/GroupDashboard";
import { AddExpenseModal } from "./components/modals/AddExpenseModal";
import { AddMemberModal } from "./components/modals/AddMemberModal";
import { ChangePasswordDialog } from "./components/modals/ChangePasswordDialog";
import { CreateGroupModal } from "./components/modals/CreateGroupModal";
import { ExpenseHistoryModal } from "./components/modals/ExpenseHistoryModal";
import { JoinGroupModal } from "./components/modals/JoinGroupModal";
import { RecordReimbursementModal } from "./components/modals/RecordReimbursementModal";
import { RecoveryKeyDialog } from "./components/modals/RecoveryKeyDialog";
import { ShareGroupModal } from "./components/modals/ShareGroupModal";
import { WhoAreYouModal } from "./components/modals/WhoAreYouModal";
import { BalancesTab } from "./components/workspace/BalancesTab";
import { ExpensesTab } from "./components/workspace/ExpensesTab";
import { GroupHeader } from "./components/workspace/GroupHeader";
import { SettleUpTab } from "./components/workspace/SettleUpTab";
import { api } from "./services/api";
import { ScanCancelled, cancelScan, onInviteLink, scanQrCode } from "./services/native";
import type {
  AccountInfo,
  Expense,
  ExpenseSplit,
  Group,
  ParticipantBalance,
  SettlementTransfer,
  SyncInfo,
  TabType,
} from "./types";

interface SyncUpdatedEvent {
  group_id: string;
  changed: boolean;
}

export const App: React.FC = () => {
  const askConfirm = useConfirm();
  // undefined while loading, null when logged out.
  const [account, setAccount] = useState<AccountInfo | null | undefined>(undefined);
  const [groups, setGroups] = useState<Group[]>([]);
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);
  const [currentGroup, setCurrentGroup] = useState<Group | null>(null);
  const [activeTab, setActiveTab] = useState<TabType>("expenses");
  const [balances, setBalances] = useState<ParticipantBalance[]>([]);
  const [settlements, setSettlements] = useState<SettlementTransfer[]>([]);
  const [loading, setLoading] = useState(true);

  // Modal open states
  const [isCreateGroupOpen, setIsCreateGroupOpen] = useState(false);
  const [isAddMemberOpen, setIsAddMemberOpen] = useState(false);
  const [isAddExpenseOpen, setIsAddExpenseOpen] = useState(false);
  const [editingExpense, setEditingExpense] = useState<Expense | null>(null);
  const [historyExpense, setHistoryExpense] = useState<Expense | null>(null);
  const [isReimburseOpen, setIsReimburseOpen] = useState(false);
  const [isShareOpen, setIsShareOpen] = useState(false);
  const [isJoinOpen, setIsJoinOpen] = useState(false);
  // What the join dialog opens with: an invite from a link or a scan, and why it failed.
  const [joinPrefill, setJoinPrefill] = useState<{ code: string; error: string | null }>({
    code: "",
    error: null,
  });
  // An invite link the app was opened with, kept until the user is logged in.
  const [pendingInvite, setPendingInvite] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  // A recovery key from sign-up or recovery, shown once over the app; or the dialog making a
  // new one from the account menu.
  const [recoveryKey, setRecoveryKey] = useState<NewRecoveryKey | null>(null);
  const [isNewRecoveryKeyOpen, setIsNewRecoveryKeyOpen] = useState(false);
  const [isChangePasswordOpen, setIsChangePasswordOpen] = useState(false);
  const [isWhoOpen, setIsWhoOpen] = useState(false);
  // Groups where the user closed the "Who are you?" prompt without answering, this session.
  const [identitySkipped, setIdentitySkipped] = useState<Set<string>>(() => new Set());
  const [syncInfo, setSyncInfo] = useState<SyncInfo | null>(null);
  const [storageWarnings, setStorageWarnings] = useState<string[]>([]);
  // Read by the sync event listener, which is registered once.
  const selectedGroupRef = useRef<string | null>(null);
  const [reimbursePrefill, setReimbursePrefill] = useState<{
    fromId?: string;
    toId?: string;
    amount?: string;
  }>({});

  const refreshGroups = useCallback(async () => {
    try {
      const list = await api.getGroups();
      setGroups(list);
    } catch (err) {
      console.error("Failed to load groups:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshActiveGroup = useCallback(async (groupId: string) => {
    try {
      const g = await api.getGroup(groupId);
      setCurrentGroup(g);
      const [bal, set, sync] = await Promise.all([
        api.getBalances(groupId),
        api.getSettlements(groupId),
        api.getSyncInfo(groupId),
      ]);
      setBalances(bal);
      setSettlements(set);
      setSyncInfo(sync);
    } catch (err) {
      console.error("Failed to load active group:", err);
      toast.error("Could not open the group", { description: errorMessage(err) });
      setSelectedGroupId(null);
      setCurrentGroup(null);
    }
  }, []);

  const refreshAccount = useCallback(async () => {
    try {
      setAccount(await api.getAccount());
    } catch (err) {
      console.error("Failed to load the account:", err);
      setAccount(null);
    }
  }, []);

  useEffect(() => {
    refreshAccount();
    api
      .getStorageWarnings()
      .then(setStorageWarnings)
      .catch((err) => console.error("Failed to load storage warnings:", err));
  }, [refreshAccount]);

  useEffect(() => onInviteLink(setPendingInvite), []);

  const loggedIn = Boolean(account);
  useEffect(() => {
    if (!loggedIn || !pendingInvite) return;
    setJoinPrefill({ code: pendingInvite, error: null });
    setIsJoinOpen(true);
    setPendingInvite(null);
  }, [loggedIn, pendingInvite]);

  useEffect(() => {
    if (loggedIn) refreshGroups();
  }, [loggedIn, refreshGroups]);

  const currentUserId = (currentGroup && account?.identities[currentGroup.id]) || null;

  useEffect(() => {
    selectedGroupRef.current = selectedGroupId;
    if (selectedGroupId) {
      refreshActiveGroup(selectedGroupId);
    } else {
      setCurrentGroup(null);
      setBalances([]);
      setSettlements([]);
      setSyncInfo(null);
    }
  }, [selectedGroupId, refreshActiveGroup]);

  // Background sync reports every attempt; reload when another device changed something.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    listen<SyncUpdatedEvent>("sync-updated", ({ payload }) => {
      if (payload.changed) refreshGroups();
      if (payload.group_id !== selectedGroupRef.current) return;
      if (payload.changed) {
        refreshActiveGroup(payload.group_id);
      } else {
        api
          .getSyncInfo(payload.group_id)
          .then(setSyncInfo)
          .catch(() => {});
      }
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((err) => console.error("Could not subscribe to sync updates:", err));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refreshGroups, refreshActiveGroup]);

  // Another device of this account joined or left a group, or changed who the user is.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    listen("account-updated", async () => {
      await refreshAccount();
      const list = await api.getGroups();
      setGroups(list);
      const selected = selectedGroupRef.current;
      if (selected && !list.some((g) => g.id === selected)) {
        setSelectedGroupId(null);
        toast.info("This group was removed from your account on another device");
      }
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((err) => console.error("Could not subscribe to account updates:", err));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refreshAccount]);

  const isShared = Boolean(syncInfo?.enabled);

  const handleSyncNow = useCallback(async () => {
    const groupId = selectedGroupRef.current;
    if (!groupId) return;
    setSyncInfo(await api.syncNow(groupId));
    await refreshActiveGroup(groupId);
    await refreshGroups();
  }, [refreshActiveGroup, refreshGroups]);

  // Catch up as soon as the app comes back to the foreground (e.g. reopening it on a phone).
  useEffect(() => {
    if (!isShared) return;
    const onVisible = () => {
      if (document.visibilityState === "visible") {
        handleSyncNow().catch((err) => console.error("Sync failed:", err));
      }
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  }, [isShared, handleSyncNow]);

  // Ask once per group who the user is; the answer is kept in their account.
  const needsIdentity = Boolean(
    currentGroup && account && !currentUserId && !identitySkipped.has(currentGroup.id)
  );
  useEffect(() => {
    if (needsIdentity) setIsWhoOpen(true);
  }, [needsIdentity]);

  // Actions
  const handleCloseWho = () => {
    setIsWhoOpen(false);
    if (currentGroup && !currentUserId) {
      const groupId = currentGroup.id;
      setIdentitySkipped((skipped) => new Set(skipped).add(groupId));
    }
  };

  const handleChooseIdentity = async (participantId: string) => {
    if (!currentGroup) return;
    setAccount(await api.setIdentity(currentGroup.id, participantId));
  };

  const handleAddSelf = async (name: string) => {
    if (!currentGroup) return;
    const updated = await api.addSelf(currentGroup.id, name);
    setCurrentGroup(updated);
    await refreshAccount();
    await refreshActiveGroup(updated.id);
    await refreshGroups();
  };

  const handleLogOut = async () => {
    if (!account) return;
    const confirmed = await askConfirm({
      title: `Log out of ${account.username}?`,
      description:
        "Your groups are removed from this device. They stay in your account: log in again to get them back.",
      confirmLabel: "Log Out",
    });
    if (!confirmed) return;
    try {
      await api.logOut(false);
    } catch (err) {
      const force = await askConfirm({
        title: "Log out anyway?",
        description: errorMessage(err),
        confirmLabel: "Log Out Anyway",
        destructive: true,
      });
      if (!force) return;
      try {
        await api.logOut(true);
      } catch (forceErr) {
        toast.error("Could not log out", { description: errorMessage(forceErr) });
        return;
      }
    }
    setSelectedGroupId(null);
    setCurrentGroup(null);
    setGroups([]);
    setIdentitySkipped(new Set());
    setAccount(null);
  };

  const handleSelectGroup = (groupId: string) => {
    setSelectedGroupId(groupId);
    setActiveTab("expenses");
  };

  const handleNavigateHome = () => {
    setSelectedGroupId(null);
    setCurrentGroup(null);
    refreshGroups();
  };

  // Errors propagate to the dialog, which shows them inline.
  const handleCreateGroup = async (name: string, currency: string, participants: string[]) => {
    const newGroup = await api.createGroup(name, currency, participants);
    // The backend recorded the creator as the first participant.
    await refreshAccount();
    await refreshGroups();
    setSelectedGroupId(newGroup.id);
  };

  const handleLeaveGroup = async () => {
    if (!currentGroup) return;
    const confirmed = await askConfirm({
      title: `Leave "${currentGroup.name}"?`,
      description:
        "It's removed from your account on all your devices. Other members keep the group, and you can rejoin with an invite link.",
      confirmLabel: "Leave Group",
      destructive: true,
    });
    if (!confirmed) return;
    try {
      await api.leaveGroup(currentGroup.id);
      handleNavigateHome();
    } catch (err) {
      toast.error("Could not leave the group", { description: errorMessage(err) });
    }
  };

  const handleRemoveMember = async (participantId: string) => {
    if (!currentGroup) return;
    const participant = currentGroup.participants.find((p) => p.id === participantId);
    if (!participant) return;
    const confirmed = await askConfirm({
      title: `Remove ${participant.name}?`,
      description:
        "Their past expenses and balance are kept, but they can't be added to new expenses.",
      confirmLabel: "Remove",
      destructive: true,
    });
    if (!confirmed) return;
    try {
      await api.removeParticipant(currentGroup.id, participantId);
      await refreshActiveGroup(currentGroup.id);
      await refreshGroups();
    } catch (err) {
      toast.error("Could not remove the member", { description: errorMessage(err) });
    }
  };

  const handleJoinGroup = async (inviteCode: string) => {
    const group = await api.joinGroup(inviteCode);
    await refreshGroups();
    setSelectedGroupId(group.id);
    setActiveTab("expenses");
    toast.success(`Joined "${group.name}"`);
  };

  const openJoin = (code = "", error: string | null = null) => {
    setJoinPrefill({ code, error });
    setIsJoinOpen(true);
  };

  // Scanning is a deliberate act, so a scanned invite is joined right away. Problems, or
  // cancelling, lead back to the join dialog.
  const handleScan = async () => {
    setIsJoinOpen(false);
    setScanning(true);
    let code: string;
    try {
      code = await scanQrCode();
    } catch (err) {
      setScanning(false);
      openJoin("", err instanceof ScanCancelled ? null : errorMessage(err));
      return;
    }
    setScanning(false);
    try {
      await handleJoinGroup(code);
    } catch (err) {
      openJoin(code, errorMessage(err));
    }
  };

  const handleAddMember = async (name: string) => {
    if (!currentGroup) return;
    const updated = await api.addParticipant(currentGroup.id, name);
    setCurrentGroup(updated);
    await refreshActiveGroup(updated.id);
    await refreshGroups();
  };

  const handleAddExpense = async (
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[],
    createdAt?: string | null
  ) => {
    if (!currentGroup) return;
    const updated = await api.addExpense(
      currentGroup.id,
      title,
      amountCents,
      paidBy,
      splits,
      createdAt
    );
    setCurrentGroup(updated);
    await refreshActiveGroup(updated.id);
    await refreshGroups();
  };

  const handleDeleteExpense = async (expenseId: string) => {
    if (!currentGroup) return;
    const expense = currentGroup.expenses.find((e) => e.id === expenseId);
    const confirmed = await askConfirm({
      title: `Delete "${expense?.title ?? "this record"}"?`,
      description: "Balances are recalculated without it. This can't be undone.",
      confirmLabel: "Delete",
      destructive: true,
    });
    if (!confirmed) return;
    try {
      const updated = await api.deleteExpense(currentGroup.id, expenseId);
      setCurrentGroup(updated);
      await refreshActiveGroup(updated.id);
      await refreshGroups();
    } catch (err) {
      toast.error("Could not delete the record", { description: errorMessage(err) });
    }
  };

  const handleOpenAddExpense = () => {
    setEditingExpense(null);
    setIsAddExpenseOpen(true);
  };

  const handleEditExpense = (expense: Expense) => {
    setEditingExpense(expense);
    setIsAddExpenseOpen(true);
  };

  const handleUpdateExpense = async (
    expenseId: string,
    title: string,
    amountCents: number,
    paidBy: string,
    splits: ExpenseSplit[],
    createdAt?: string | null
  ) => {
    if (!currentGroup) return;
    const updated = await api.updateExpense(
      currentGroup.id,
      expenseId,
      title,
      amountCents,
      paidBy,
      splits,
      createdAt
    );
    setCurrentGroup(updated);
    await refreshActiveGroup(updated.id);
    await refreshGroups();
  };

  const handleViewHistory = (expense: Expense) => {
    setHistoryExpense(expense);
  };

  const handleRecordReimbursement = async (
    fromId: string,
    toId: string,
    amountCents: number,
    notes?: string
  ) => {
    if (!currentGroup) return;
    const updated = await api.recordReimbursement(
      currentGroup.id,
      fromId,
      toId,
      amountCents,
      notes
    );
    setCurrentGroup(updated);
    await refreshActiveGroup(updated.id);
    await refreshGroups();
  };

  const handleOpenReimburseModal = (fromId?: string, toId?: string, amount?: string) => {
    setReimbursePrefill({ fromId, toId, amount });
    setIsReimburseOpen(true);
  };

  if (account === undefined) {
    return <Splash />;
  }

  if (account === null) {
    return (
      <AuthScreen
        onAuthenticated={(acc, key) => {
          setLoading(true);
          setAccount(acc);
          setRecoveryKey(key ?? null);
        }}
      />
    );
  }

  return (
    <div className="flex min-h-screen flex-col">
      <Navbar
        currentGroup={currentGroup}
        onNavigateHome={handleNavigateHome}
        onOpenCreateGroup={() => setIsCreateGroupOpen(true)}
        username={account.username}
        serverUrl={account.server_url}
        onChangePassword={() => setIsChangePasswordOpen(true)}
        onNewRecoveryKey={() => setIsNewRecoveryKeyOpen(true)}
        onLogOut={handleLogOut}
      />

      <main className="mx-auto w-full max-w-5xl flex-1 space-y-6 px-4 py-6 pb-[max(1.5rem,env(safe-area-inset-bottom))]">
        {storageWarnings.length > 0 && (
          <Alert>
            <TriangleAlertIcon />
            <AlertTitle>Some saved data could not be loaded</AlertTitle>
            <AlertDescription>
              <ul className="list-disc pl-4">
                {storageWarnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
              <p>Nothing was deleted. The data is still on disk.</p>
            </AlertDescription>
            <AlertAction>
              <Button variant="ghost" size="sm" onClick={() => setStorageWarnings([])}>
                Dismiss
              </Button>
            </AlertAction>
          </Alert>
        )}

        {loading ? (
          <div className="flex items-center justify-center gap-2 py-20 text-sm text-muted-foreground">
            <Spinner /> Loading groups…
          </div>
        ) : !currentGroup ? (
          <GroupDashboard
            groups={groups}
            onSelectGroup={handleSelectGroup}
            onOpenCreateGroup={() => setIsCreateGroupOpen(true)}
            onOpenJoinGroup={() => openJoin()}
          />
        ) : (
          <div className="space-y-4">
            <Button variant="ghost" size="sm" onClick={handleNavigateHome} className="-ml-2">
              <ArrowLeftIcon data-icon="inline-start" />
              Back to All Groups
            </Button>

            <GroupHeader
              group={currentGroup}
              balances={balances}
              currentUserId={currentUserId}
              onChangeIdentity={() => setIsWhoOpen(true)}
              onOpenAddMember={() => setIsAddMemberOpen(true)}
              onRemoveMember={handleRemoveMember}
              onOpenShare={() => setIsShareOpen(true)}
              onLeaveGroup={handleLeaveGroup}
              syncInfo={syncInfo}
            />

            <Tabs value={activeTab} onValueChange={(value) => setActiveTab(value as TabType)}>
              <TabsList className="w-full sm:w-fit">
                <TabsTrigger value="expenses">
                  <ReceiptTextIcon />
                  Expenses
                  <Badge variant="secondary" className="tabular-nums">
                    {currentGroup.expenses.length}
                  </Badge>
                </TabsTrigger>
                <TabsTrigger value="balances">
                  <ScaleIcon />
                  Balances
                </TabsTrigger>
                <TabsTrigger value="settle">
                  <ArrowLeftRightIcon />
                  Settle Up
                  {settlements.length > 0 && (
                    <Badge variant="secondary" className="tabular-nums">
                      {settlements.length}
                    </Badge>
                  )}
                </TabsTrigger>
              </TabsList>

              <TabsContent value="expenses" className="pt-4">
                <ExpensesTab
                  group={currentGroup}
                  hasOutstandingDebt={currentGroup.expenses.length > 0 && settlements.length > 0}
                  onOpenAddExpense={handleOpenAddExpense}
                  onOpenReimburse={() => handleOpenReimburseModal()}
                  onDeleteExpense={handleDeleteExpense}
                  onEditExpense={handleEditExpense}
                  onViewHistory={handleViewHistory}
                />
              </TabsContent>
              <TabsContent value="balances" className="pt-4">
                <BalancesTab
                  group={currentGroup}
                  balances={balances}
                  onReimburseParticipant={(pid, amount) =>
                    handleOpenReimburseModal(pid, undefined, amount)
                  }
                />
              </TabsContent>
              <TabsContent value="settle" className="pt-4">
                <SettleUpTab
                  group={currentGroup}
                  settlements={settlements}
                  onOpenReimburse={() => handleOpenReimburseModal()}
                  onMarkAsPaid={(fromId, toId, amount) =>
                    handleOpenReimburseModal(fromId, toId, amount)
                  }
                />
              </TabsContent>
            </Tabs>
          </div>
        )}
      </main>

      <CreateGroupModal
        isOpen={isCreateGroupOpen}
        onClose={() => setIsCreateGroupOpen(false)}
        onCreateGroup={handleCreateGroup}
        ownName={account.username}
      />

      <JoinGroupModal
        isOpen={isJoinOpen}
        onClose={() => setIsJoinOpen(false)}
        onJoinGroup={handleJoinGroup}
        initialCode={joinPrefill.code}
        initialError={joinPrefill.error}
        onScan={handleScan}
      />

      <RecoveryKeyDialog
        isOpen={recoveryKey !== null}
        onClose={() => setRecoveryKey(null)}
        reason={recoveryKey?.reason ?? "signup"}
        recoveryKey={recoveryKey?.key}
      />

      <RecoveryKeyDialog
        isOpen={isNewRecoveryKeyOpen}
        onClose={() => setIsNewRecoveryKeyOpen(false)}
        reason="replace"
      />

      <ChangePasswordDialog
        isOpen={isChangePasswordOpen}
        onClose={() => setIsChangePasswordOpen(false)}
        username={account.username}
      />

      {scanning && (
        <QrScanOverlay
          onCancel={() =>
            cancelScan().catch((err) => console.error("Could not stop scanning:", err))
          }
        />
      )}

      {currentGroup && (
        <>
          <ShareGroupModal
            isOpen={isShareOpen}
            onClose={() => setIsShareOpen(false)}
            group={currentGroup}
            syncInfo={syncInfo}
            onSyncNow={handleSyncNow}
          />

          <WhoAreYouModal
            isOpen={isWhoOpen}
            onClose={handleCloseWho}
            group={currentGroup}
            currentUserId={currentUserId}
            defaultName={account.username}
            onChoose={handleChooseIdentity}
            onAddSelf={handleAddSelf}
          />

          <AddMemberModal
            isOpen={isAddMemberOpen}
            onClose={() => setIsAddMemberOpen(false)}
            onAddMember={handleAddMember}
          />

          <AddExpenseModal
            isOpen={isAddExpenseOpen}
            onClose={() => {
              setIsAddExpenseOpen(false);
              setEditingExpense(null);
            }}
            group={currentGroup}
            currentUserId={currentUserId}
            onAddExpense={handleAddExpense}
            editingExpense={editingExpense}
            onUpdateExpense={handleUpdateExpense}
          />

          <RecordReimbursementModal
            isOpen={isReimburseOpen}
            onClose={() => setIsReimburseOpen(false)}
            group={currentGroup}
            initialFromId={reimbursePrefill.fromId}
            initialToId={reimbursePrefill.toId}
            initialAmount={reimbursePrefill.amount}
            onRecordReimbursement={handleRecordReimbursement}
          />

          <ExpenseHistoryModal
            isOpen={Boolean(historyExpense)}
            onClose={() => setHistoryExpense(null)}
            expense={historyExpense}
            currency={currentGroup.currency}
            participants={currentGroup.participants}
          />
        </>
      )}
    </div>
  );
};
