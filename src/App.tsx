import type React from "react";
import { useCallback, useEffect, useState } from "react";
import { Navbar } from "./components/common/Navbar";
import { GroupDashboard } from "./components/dashboard/GroupDashboard";
import { AddExpenseModal } from "./components/modals/AddExpenseModal";
import { AddMemberModal } from "./components/modals/AddMemberModal";
import { CreateGroupModal } from "./components/modals/CreateGroupModal";
import { ExpenseHistoryModal } from "./components/modals/ExpenseHistoryModal";
import { RecordReimbursementModal } from "./components/modals/RecordReimbursementModal";
import { BalancesTab } from "./components/workspace/BalancesTab";
import { ExpensesTab } from "./components/workspace/ExpensesTab";
import { GroupHeader } from "./components/workspace/GroupHeader";
import { SettleUpTab } from "./components/workspace/SettleUpTab";
import { api } from "./services/api";
import type {
  Expense,
  ExpenseSplit,
  Group,
  ParticipantBalance,
  SettlementTransfer,
  TabType,
} from "./types";

export const App: React.FC = () => {
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
      const [bal, set] = await Promise.all([api.getBalances(groupId), api.getSettlements(groupId)]);
      setBalances(bal);
      setSettlements(set);
    } catch (err) {
      console.error("Failed to load active group:", err);
      alert(`Failed to open group: ${err instanceof Error ? err.message : String(err)}`);
      setSelectedGroupId(null);
      setCurrentGroup(null);
    }
  }, []);

  useEffect(() => {
    refreshGroups();
  }, [refreshGroups]);

  useEffect(() => {
    if (selectedGroupId) {
      refreshActiveGroup(selectedGroupId);
    } else {
      setCurrentGroup(null);
      setBalances([]);
      setSettlements([]);
    }
  }, [selectedGroupId, refreshActiveGroup]);

  // Actions
  const handleSelectGroup = (groupId: string) => {
    setSelectedGroupId(groupId);
    setActiveTab("expenses");
  };

  const handleNavigateHome = () => {
    setSelectedGroupId(null);
    setCurrentGroup(null);
    refreshGroups();
  };

  const handleCreateGroup = async (name: string, currency: string, participants: string[]) => {
    try {
      const newGroup = await api.createGroup(name, currency, participants);
      await refreshGroups();
      setSelectedGroupId(newGroup.id);
    } catch (err) {
      alert(`Failed to create group: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleDeleteGroup = async () => {
    if (!currentGroup) return;
    if (confirm(`Are you sure you want to delete the group "${currentGroup.name}"?`)) {
      await api.deleteGroup(currentGroup.id);
      handleNavigateHome();
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
    splits: ExpenseSplit[]
  ) => {
    if (!currentGroup) return;
    const updated = await api.addExpense(currentGroup.id, title, amountCents, paidBy, splits);
    setCurrentGroup(updated);
    await refreshActiveGroup(updated.id);
    await refreshGroups();
  };

  const handleDeleteExpense = async (expenseId: string) => {
    if (!currentGroup) return;
    if (confirm("Are you sure you want to delete this record?")) {
      const updated = await api.deleteExpense(currentGroup.id, expenseId);
      setCurrentGroup(updated);
      await refreshActiveGroup(updated.id);
      await refreshGroups();
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
    splits: ExpenseSplit[]
  ) => {
    if (!currentGroup) return;
    const updated = await api.updateExpense(
      currentGroup.id,
      expenseId,
      title,
      amountCents,
      paidBy,
      splits
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

  return (
    <div className="min-h-screen bg-slate-950 text-slate-100 flex flex-col font-sans selection:bg-indigo-500 selection:text-white">
      <Navbar
        currentGroup={currentGroup}
        onNavigateHome={handleNavigateHome}
        onOpenCreateGroup={() => setIsCreateGroupOpen(true)}
      />

      <main className="flex-1 max-w-6xl w-full mx-auto px-4 py-6">
        {loading ? (
          <div className="flex items-center justify-center py-20 text-slate-500 text-sm">
            Loading groups...
          </div>
        ) : !currentGroup ? (
          <GroupDashboard
            groups={groups}
            onSelectGroup={handleSelectGroup}
            onOpenCreateGroup={() => setIsCreateGroupOpen(true)}
          />
        ) : (
          <div className="space-y-6 animate-in fade-in duration-200">
            {/* Back Button */}
            <button
              type="button"
              onClick={handleNavigateHome}
              className="inline-flex items-center gap-1.5 text-xs font-semibold text-slate-400 hover:text-indigo-400 transition cursor-pointer"
            >
              ← Back to All Groups
            </button>

            <GroupHeader
              group={currentGroup}
              onOpenAddMember={() => setIsAddMemberOpen(true)}
              onDeleteGroup={handleDeleteGroup}
            />

            {/* Tab Navigation */}
            <div className="border-b border-slate-800">
              <nav className="flex space-x-2">
                <button
                  type="button"
                  onClick={() => setActiveTab("expenses")}
                  className={`px-4 py-2.5 text-xs sm:text-sm font-semibold border-b-2 transition flex items-center gap-2 cursor-pointer ${
                    activeTab === "expenses"
                      ? "border-indigo-500 text-indigo-400"
                      : "border-transparent text-slate-400 hover:text-slate-200"
                  }`}
                >
                  <svg
                    className="w-4 h-4"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth="2"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M9 12h3.75M9 15h3.75M9 18h3.75m3 .75H18a2.25 2.25 0 0 0 2.25-2.25V6.108c0-1.135-.845-2.098-1.976-2.192a48.424 48.424 0 0 0-1.123-.08m-5.801 0c-.065.21-.1.433-.1.664 0 .414.336.75.75.75h4.5a.75.75 0 0 0 .75-.75 2.25 2.25 0 0 0-.1-.664m-5.8 0A2.251 2.251 0 0 1 13.5 2.25H15c1.012 0 1.867.668 2.15 1.586m-5.8 0c-.376.023-.75.05-1.124.08C9.095 4.01 8.25 4.973 8.25 6.108V8.25m0 0H4.875c-.621 0-1.125.504-1.125 1.125v11.25c0 .621.504 1.125 1.125 1.125h9.75c.621 0 1.125-.504 1.125-1.125V9.375c0-.621-.504-1.125-1.125-1.125H8.25ZM6.75 12h.008v.008H6.75V12Zm0 3h.008v.008H6.75V15Zm0 3h.008v.008H6.75V18Z"
                    />
                  </svg>
                  <span>Expenses</span>
                  <span className="px-1.5 py-0.5 rounded-full text-[10px] bg-slate-800 text-slate-300 font-mono">
                    {currentGroup.expenses.length}
                  </span>
                </button>

                <button
                  type="button"
                  onClick={() => setActiveTab("balances")}
                  className={`px-4 py-2.5 text-xs sm:text-sm font-semibold border-b-2 transition flex items-center gap-2 cursor-pointer ${
                    activeTab === "balances"
                      ? "border-indigo-500 text-indigo-400"
                      : "border-transparent text-slate-400 hover:text-slate-200"
                  }`}
                >
                  <svg
                    className="w-4 h-4"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth="2"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M3 13.125C3 12.504 3.504 12 4.125 12h2.25c.621 0 1.125.504 1.125 1.125v6.75C7.5 20.496 6.996 21 6.375 21h-2.25A1.125 1.125 0 0 1 3 19.875v-6.75ZM9.75 8.625c0-.621.504-1.125 1.125-1.125h2.25c.621 0 1.125.504 1.125 1.125v11.25c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V8.625ZM16.5 4.125c0-.621.504-1.125 1.125-1.125h2.25C20.496 3 21 3.504 21 4.125v15.75c0 .621-.504 1.125-1.125 1.125h-2.25a1.125 1.125 0 0 1-1.125-1.125V4.125Z"
                    />
                  </svg>
                  <span>Balances</span>
                </button>

                <button
                  type="button"
                  onClick={() => setActiveTab("settle")}
                  className={`px-4 py-2.5 text-xs sm:text-sm font-semibold border-b-2 transition flex items-center gap-2 cursor-pointer ${
                    activeTab === "settle"
                      ? "border-indigo-500 text-indigo-400"
                      : "border-transparent text-slate-400 hover:text-slate-200"
                  }`}
                >
                  <svg
                    className="w-4 h-4"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    strokeWidth="2"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      d="M7.5 21 3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5"
                    />
                  </svg>
                  <span>Settle Up</span>
                  {settlements.length > 0 && (
                    <span className="px-1.5 py-0.5 rounded-full text-[10px] bg-emerald-500/20 text-emerald-300 font-mono">
                      {settlements.length}
                    </span>
                  )}
                </button>
              </nav>
            </div>

            {/* Tab Panels */}
            {activeTab === "expenses" && (
              <ExpensesTab
                group={currentGroup}
                onOpenAddExpense={handleOpenAddExpense}
                onOpenReimburse={() => handleOpenReimburseModal()}
                onDeleteExpense={handleDeleteExpense}
                onEditExpense={handleEditExpense}
                onViewHistory={handleViewHistory}
              />
            )}

            {activeTab === "balances" && (
              <BalancesTab
                group={currentGroup}
                balances={balances}
                onReimburseParticipant={(pid, amount) =>
                  handleOpenReimburseModal(pid, undefined, amount)
                }
              />
            )}

            {activeTab === "settle" && (
              <SettleUpTab
                group={currentGroup}
                settlements={settlements}
                onOpenReimburse={() => handleOpenReimburseModal()}
                onMarkAsPaid={(fromId, toId, amount) =>
                  handleOpenReimburseModal(fromId, toId, amount)
                }
              />
            )}
          </div>
        )}
      </main>

      {/* Modals */}
      <CreateGroupModal
        isOpen={isCreateGroupOpen}
        onClose={() => setIsCreateGroupOpen(false)}
        onCreateGroup={handleCreateGroup}
      />

      {currentGroup && (
        <>
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
