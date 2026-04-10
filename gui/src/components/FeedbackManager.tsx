import { useState, useEffect } from "react";
import { Send, Loader2, CheckCircle, Clock, Check, RefreshCw, MessageSquare, LifeBuoy } from "lucide-react";
import { PageHeader } from "./layout/PageHeader";
import { PageShell } from "./layout/PageShell";
import { MetricCard } from "./ui/MetricCard";

interface FeedbackItem {
  id: number;
  type: string;
  message: string;
  date: number;
  status: string;
}

export function FeedbackManager() {
  const [form, setForm] = useState({ type: "bug", message: "", email: "" });
  const [sending, setSending] = useState(false);
  const [history, setHistory] = useState<FeedbackItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [typeFilter, setTypeFilter] = useState("all");
  const [statusFilter, setStatusFilter] = useState("all");
  const [submitFeedback, setSubmitFeedback] = useState<{
    type: "success" | "error";
    text: string;
  } | null>(null);

  useEffect(() => {
    loadHistory();
  }, []);

  function loadHistory() {
    const saved = localStorage.getItem("cws_feedback_history");
    if (saved) {
        setHistory(JSON.parse(saved));
    }
  }

  async function checkStatuses(items: FeedbackItem[]) {
      setLoading(true);
      setHistory(items);
      localStorage.setItem("cws_feedback_history", JSON.stringify(items));
      window.setTimeout(() => setLoading(false), 250);
  }

  async function handleSubmit() {
      if (!form.message) return;
      setSending(true);
      setSubmitFeedback(null);
      try {
          const newItem: FeedbackItem = {
              id: Date.now(),
              type: form.type,
              message: form.message,
              date: Date.now(),
              status: 'received'
          };
          const newHistory = [newItem, ...history];
          setHistory(newHistory);
          localStorage.setItem("cws_feedback_history", JSON.stringify(newHistory));
          setForm({ ...form, message: "" }); // Keep email
          setSubmitFeedback({ type: "success", text: "Feedback saved locally." });
      } catch (e) {
          setSubmitFeedback({ type: "error", text: "Failed to save feedback locally." });
      } finally {
          setSending(false);
      }
  }

  function getStatusBadge(status: string) {
      const s = status.toLowerCase();
      if (s === 'received' || s === 'new') return <span className="flex items-center text-slate-500 bg-slate-100 px-2 py-1 rounded text-xs font-bold uppercase"><Clock className="w-3 h-3 mr-1"/> Queued</span>;
      if (s === 'planned' || s === 'in_progress') return <span className="flex items-center text-blue-600 dark:text-blue-400 bg-blue-50 px-2 py-1 rounded text-xs font-bold uppercase"><Loader2 className="w-3 h-3 mr-1 animate-spin"/> In Progress</span>;
      if (s === 'released' || s === 'done') return <span className="flex items-center text-green-600 dark:text-green-400 bg-green-50 px-2 py-1 rounded text-xs font-bold uppercase"><CheckCircle className="w-3 h-3 mr-1"/> Released</span>;
      return <span className="flex items-center text-slate-400 bg-slate-50 px-2 py-1 rounded text-xs font-bold uppercase">{s}</span>;
  }

  const queuedCount = history.filter((item) => {
    const status = item.status.toLowerCase();
    return status === "received" || status === "new";
  }).length;
  const activeCount = history.filter((item) => {
    const status = item.status.toLowerCase();
    return status === "planned" || status === "in_progress";
  }).length;
  const releasedCount = history.filter((item) => {
    const status = item.status.toLowerCase();
    return status === "released" || status === "done";
  }).length;
  const filteredHistory = history.filter((item) => {
    const typeOk = typeFilter === "all" || item.type === typeFilter;
    const status = item.status.toLowerCase();
    const normalizedStatus =
      status === "received" || status === "new"
        ? "queued"
        : status === "planned" || status === "in_progress"
          ? "active"
          : status === "released" || status === "done"
            ? "released"
            : "other";
    const statusOk = statusFilter === "all" || normalizedStatus === statusFilter;
    return typeOk && statusOk;
  });

  return (
    <PageShell maxWidthClassName="max-w-6xl" className="space-y-8 animate-in fade-in slide-in-from-bottom-4 duration-300 transition-colors dark:text-slate-100">
        <PageHeader
            title="Feedback"
            subtitle="Submit product issues and workflow requests, then keep a local operating record of what is queued, in progress, or released."
            icon={<LifeBuoy className="w-6 h-6" />}
        />

        <div className="mt-8 grid gap-4 md:grid-cols-3">
            <MetricCard
                label="Queued"
                value={queuedCount}
                hint="New items waiting for triage or confirmation."
                icon={<Clock className="h-5 w-5" />}
            />
            <MetricCard
                label="In Progress"
                value={activeCount}
                hint="Requests already moving through the delivery pipeline."
                icon={<Loader2 className="h-5 w-5" />}
            />
            <MetricCard
                label="Released"
                value={releasedCount}
                hint="Local requests that already landed in shipped product behavior."
                icon={<CheckCircle className="h-5 w-5" />}
            />
        </div>

        <div className="mt-6 rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
            <div className="grid gap-4 md:grid-cols-3">
                <div>
                    <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Best Use</p>
                    <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                        Use this page for product gaps, UI friction, workflow ideas, and missing operator controls.
                    </p>
                </div>
                <div>
                    <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Do Not Use</p>
                    <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                        For startup failures, proxy routing errors, and runtime exits, go to System Logs first.
                    </p>
                </div>
                <div>
                    <p className="text-xs font-semibold uppercase tracking-[0.22em] text-slate-500 dark:text-slate-400">Operator Outcome</p>
                    <p className="mt-2 text-sm leading-6 text-slate-700 dark:text-slate-200">
                        Each submission stays visible locally so operators can avoid filing the same issue twice.
                    </p>
                </div>
            </div>
        </div>

        <div className="mt-8 rounded-2xl border border-slate-200 bg-white p-5 shadow-sm dark:border-slate-700 dark:bg-slate-800">
            <div className="flex items-start gap-3">
                <div className="mt-0.5 flex h-10 w-10 items-center justify-center rounded-2xl bg-indigo-50 text-indigo-600 dark:bg-indigo-500/15 dark:text-indigo-300">
                    <MessageSquare className="h-5 w-5" />
                </div>
                <div>
                    <h2 className="text-lg font-semibold text-slate-900 dark:text-white">Support lane</h2>
                    <p className="mt-1 text-sm leading-6 text-slate-500 dark:text-slate-400">
                        Use this screen for product gaps and operator friction. Use <span className="font-medium text-slate-700 dark:text-slate-200">System Logs</span> when startup, proxy, or runtime behavior needs investigation.
                    </p>
                </div>
            </div>
        </div>

        <div className="mt-8 flex flex-col gap-8 pb-12">
            {/* Submit Form */}
            <div className="w-full">
                <div className="bg-white dark:bg-slate-800 p-6 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm">
                    <h2 className="font-bold text-lg mb-4 text-slate-900 dark:text-white">New Submission</h2>
                    {submitFeedback && (
                        <div
                            className={`mb-4 rounded-lg border px-3 py-2 text-sm font-medium ${
                                submitFeedback.type === "error"
                                    ? "border-rose-200 dark:border-rose-800 bg-rose-50 dark:bg-rose-900/20 text-rose-700 dark:text-rose-300"
                                    : "border-emerald-200 dark:border-emerald-800 bg-emerald-50 dark:bg-emerald-900/20 text-emerald-700 dark:text-emerald-300"
                            }`}
                        >
                            {submitFeedback.text}
                        </div>
                    )}
                    
                    <div className="space-y-4">
                        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                            <div>
                                <label className="text-xs font-bold text-slate-400 dark:text-slate-500 uppercase mb-1.5 block">Type</label>
                                <div className="relative">
                                    <select 
                                        value={form.type}
                                        onChange={e => setForm({...form, type: e.target.value})}
                                        className="w-full p-2.5 border border-slate-200 dark:border-slate-600 rounded-lg bg-slate-50 dark:bg-slate-900 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 appearance-none text-sm font-medium"
                                    >
                                        <option value="bug">Bug Report</option>
                                        <option value="feature">Feature Request</option>
                                        <option value="other">Other</option>
                                    </select>
                                </div>
                            </div>
                            <div>
                                <label className="text-xs font-bold text-slate-400 dark:text-slate-500 uppercase mb-1.5 block">Email (Optional)</label>
                                <input 
                                    type="email"
                                    value={form.email}
                                    onChange={e => setForm({...form, email: e.target.value})}
                                    placeholder="For updates"
                                    className="w-full p-2.5 border border-slate-200 dark:border-slate-600 rounded-lg bg-slate-50 dark:bg-slate-900 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 text-sm placeholder-slate-400"
                                />
                            </div>
                        </div>

                        <div>
                            <label className="text-xs font-bold text-slate-400 dark:text-slate-500 uppercase mb-1.5 block">Message</label>
                            <textarea 
                                rows={4}
                                value={form.message}
                                onChange={e => setForm({...form, message: e.target.value})}
                                placeholder="Describe your issue or idea..."
                                className="w-full p-3 border border-slate-200 dark:border-slate-600 rounded-lg bg-slate-50 dark:bg-slate-900 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 resize-none text-sm placeholder-slate-400"
                            />
                        </div>

                        <button 
                            onClick={handleSubmit} 
                            disabled={sending || !form.message}
                            className="w-full md:w-auto px-8 bg-indigo-600 hover:bg-indigo-700 text-white py-2.5 rounded-lg font-bold flex items-center justify-center transition-all disabled:opacity-50 shadow-lg shadow-indigo-500/20"
                        >
                            {sending ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <Send className="w-4 h-4 mr-2" />}
                            {sending ? "Sending..." : "Submit Feedback"}
                        </button>
                    </div>
                </div>
            </div>

            {/* History List */}
            <div className="w-full flex-1">
                <div className="bg-white dark:bg-slate-800 rounded-xl border border-slate-200 dark:border-slate-700 shadow-sm overflow-hidden flex flex-col h-full min-h-[400px]">
                    <div className="p-4 border-b border-slate-100 dark:border-slate-700 bg-slate-50 dark:bg-slate-900/50 flex justify-between items-center">
                        <div>
                            <h2 className="font-bold text-lg text-slate-900 dark:text-white">My Submissions</h2>
                            <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">
                                {filteredHistory.length} visible records after local filters
                            </p>
                        </div>
                        <div className="flex items-center gap-2">
                            <select
                                value={typeFilter}
                                onChange={(e) => setTypeFilter(e.target.value)}
                                className="rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs font-semibold text-slate-700 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
                            >
                                <option value="all">All Types</option>
                                <option value="bug">Bug</option>
                                <option value="feature">Feature</option>
                                <option value="other">Other</option>
                            </select>
                            <select
                                value={statusFilter}
                                onChange={(e) => setStatusFilter(e.target.value)}
                                className="rounded-lg border border-slate-200 bg-white px-3 py-2 text-xs font-semibold text-slate-700 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
                            >
                                <option value="all">All Status</option>
                                <option value="queued">Queued</option>
                                <option value="active">In Progress</option>
                                <option value="released">Released</option>
                            </select>
                            <button onClick={() => checkStatuses(history)} disabled={loading} className="text-slate-400 hover:text-indigo-600 dark:text-indigo-400 transition-colors p-2 rounded-full hover:bg-slate-200 dark:hover:bg-slate-700">
                                <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
                            </button>
                        </div>
                    </div>
                    
                    <div className="flex-1 overflow-y-auto p-4 space-y-3">
                        {filteredHistory.length > 0 ? (
                            filteredHistory.map(item => (
                                <div key={item.id} className="p-4 border border-slate-100 dark:border-slate-700 rounded-xl bg-white dark:bg-slate-800 hover:shadow-md transition-shadow group">
                                    <div className="flex justify-between items-start mb-2">
                                        <div className="flex items-center gap-2">
                                            <span className={`text-[10px] font-bold uppercase px-2 py-0.5 rounded border ${
                                                item.type === 'bug' ? 'bg-red-50 text-red-600 dark:text-red-400 border-red-100 dark:bg-red-900/20 dark:border-red-800' : 'bg-amber-50 text-amber-600 dark:text-amber-400 border-amber-100 dark:bg-amber-900/20 dark:border-amber-800'
                                            }`}>{item.type}</span>
                                            <span className="text-xs text-slate-400 font-mono">#{10240 + item.id}</span>
                                        </div>
                                        {getStatusBadge(item.status)}
                                    </div>
                                    <p className="text-slate-700 dark:text-slate-200 dark:text-slate-300 text-sm whitespace-pre-wrap">{item.message}</p>
                                    <div className="mt-3 pt-3 border-t border-slate-50 dark:border-slate-700/50 flex justify-between items-center text-xs text-slate-400">
                                        <span>{new Date(item.date).toLocaleDateString()}</span>
                                        {item.status === 'released' && <span className="text-green-600 dark:text-green-400 font-bold flex items-center"><Check className="w-3 h-3 mr-1"/> Implemented</span>}
                                    </div>
                                </div>
                            ))
                        ) : (
                            <div className="flex flex-col items-center justify-center h-64 text-slate-400">
                                <div className="p-4 bg-slate-50 dark:bg-slate-700 rounded-full mb-4">
                                    <MessageSquare className="w-8 h-8 opacity-50" />
                                </div>
                                <p>No feedback records match the current filters.</p>
                                <p className="text-xs mt-1">Adjust the filters or create a new submission.</p>
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    </PageShell>
  );
}
