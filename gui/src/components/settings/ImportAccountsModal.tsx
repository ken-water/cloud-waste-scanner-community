import { type ChangeEvent, type Dispatch, type RefObject, type SetStateAction } from "react";
import { Loader2, RefreshCw, Upload, X } from "lucide-react";

interface CloudImportCandidate {
  id: string;
  provider: string;
  name: string;
  credentials: string;
  region?: string | null;
  source: string;
  import_kind: "aws_local" | "cloud_profile" | string;
}

interface ImportRenameMapping {
  provider: string;
  original: string;
  imported: string;
}

interface ImportAccountsModalProps {
  open: boolean;
  onClose: () => void;
  importFileRef: RefObject<HTMLInputElement | null>;
  onFileChange: (event: ChangeEvent<HTMLInputElement>) => void | Promise<void>;
  discoverImportableAccounts: () => void | Promise<void>;
  downloadImportTemplate: (format: "json" | "csv") => void | Promise<void>;
  selectAllImportCandidates: (checked: boolean) => void;
  handleImportSelectedAccounts: () => void | Promise<void>;
  discoveringImports: boolean;
  importingAccounts: boolean;
  importCandidates: CloudImportCandidate[];
  selectedImportIds: Record<string, boolean>;
  setSelectedImportIds: Dispatch<SetStateAction<Record<string, boolean>>>;
  providerDisplayName: (provider: string) => string;
  importResultSummary: string;
  importInvalidItems: string[];
  importExecutionFailures: string[];
  importRenameMappings: ImportRenameMapping[];
}

export function ImportAccountsModal({
  open,
  onClose,
  importFileRef,
  onFileChange,
  discoverImportableAccounts,
  downloadImportTemplate,
  selectAllImportCandidates,
  handleImportSelectedAccounts,
  discoveringImports,
  importingAccounts,
  importCandidates,
  selectedImportIds,
  setSelectedImportIds,
  providerDisplayName,
  importResultSummary,
  importInvalidItems,
  importExecutionFailures,
  importRenameMappings,
}: ImportAccountsModalProps) {
  if (!open) {
    return null;
  }

  return (
    <>
      <input
        ref={importFileRef as RefObject<HTMLInputElement>}
        type="file"
        accept=".json,.csv,application/json,text/csv"
        className="hidden"
        onChange={onFileChange}
      />

      <div className="fixed inset-0 bg-black/50 backdrop-blur-sm flex items-center justify-center z-50 p-4">
        <div className="bg-white dark:bg-slate-800 rounded-xl p-6 w-full max-w-4xl shadow-2xl animate-in zoom-in-95 border border-slate-200 dark:border-slate-700">
          <div className="flex justify-between items-center mb-4">
            <div>
              <h3 className="text-2xl font-bold text-slate-900 dark:text-white">Import Cloud Accounts</h3>
              <p className="text-sm text-slate-500 dark:text-slate-400 mt-1">
                Discover from local environment/CLI (including AWS SSO), or load from a JSON/CSV file.
              </p>
            </div>
            <button onClick={onClose} className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200">
              <X className="w-5 h-5" />
            </button>
          </div>

          <div className="flex flex-wrap gap-2 mb-4">
            <button
              onClick={discoverImportableAccounts}
              disabled={discoveringImports || importingAccounts}
              className="inline-flex items-center px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-600 text-sm font-semibold text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {discoveringImports ? <Loader2 className="w-4 h-4 mr-2 animate-spin" /> : <RefreshCw className="w-4 h-4 mr-2" />}
              Discover Local Accounts
            </button>
            <button
              onClick={() => importFileRef.current?.click()}
              disabled={discoveringImports || importingAccounts}
              className="inline-flex items-center px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-600 text-sm font-semibold text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Upload className="w-4 h-4 mr-2" />
              Import JSON / CSV
            </button>
            <button
              onClick={() => downloadImportTemplate("json")}
              disabled={discoveringImports || importingAccounts}
              className="inline-flex items-center px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-600 text-sm font-semibold text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              JSON Template
            </button>
            <button
              onClick={() => downloadImportTemplate("csv")}
              disabled={discoveringImports || importingAccounts}
              className="inline-flex items-center px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-600 text-sm font-semibold text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              CSV Template
            </button>
            <button
              onClick={() => selectAllImportCandidates(true)}
              disabled={importCandidates.length === 0 || importingAccounts}
              className="inline-flex items-center px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-600 text-sm font-semibold text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Select All
            </button>
            <button
              onClick={() => selectAllImportCandidates(false)}
              disabled={importCandidates.length === 0 || importingAccounts}
              className="inline-flex items-center px-3 py-2 rounded-lg border border-slate-200 dark:border-slate-600 text-sm font-semibold text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Clear Selection
            </button>
          </div>

          <div className="mb-4 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-900/40 p-3">
            <p className="text-xs font-bold uppercase tracking-wide text-slate-600 dark:text-slate-300">
              Duplicate Name Handling
            </p>
            <p className="mt-1 text-xs text-slate-600 dark:text-slate-300">
              If an imported account name already exists, the app keeps the existing account and renames the incoming one with suffixes like <code>-2</code>, <code>-3</code>. After import, a full rename mapping list is shown below.
            </p>
          </div>

          <div className="border border-slate-200 dark:border-slate-700 rounded-lg overflow-hidden">
            <div className="grid grid-cols-[44px_1.1fr_1fr_1.3fr_120px] bg-slate-100 dark:bg-slate-900/60 text-xs font-semibold uppercase tracking-wide text-slate-500 dark:text-slate-400 px-2 py-2">
              <span></span>
              <span>Account Name</span>
              <span>Provider</span>
              <span>Source</span>
              <span>Mode</span>
            </div>
            <div className="max-h-[320px] overflow-y-auto divide-y divide-slate-100 dark:divide-slate-700">
              {importCandidates.length === 0 ? (
                <div className="py-10 text-center text-sm text-slate-500 dark:text-slate-400">
                  {discoveringImports ? "Discovering importable accounts..." : "No importable accounts loaded yet."}
                </div>
              ) : (
                importCandidates.map((candidate) => (
                  <label
                    key={candidate.id}
                    className="grid grid-cols-[44px_1.1fr_1fr_1.3fr_120px] items-center px-2 py-2 text-sm text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-700/40 cursor-pointer"
                  >
                    <input
                      type="checkbox"
                      checked={Boolean(selectedImportIds[candidate.id])}
                      onChange={(event) =>
                        setSelectedImportIds((prev) => ({
                          ...prev,
                          [candidate.id]: event.target.checked,
                        }))
                      }
                      className="w-4 h-4 accent-indigo-600"
                    />
                    <span className="font-semibold truncate pr-2" title={candidate.name}>{candidate.name}</span>
                    <span className="truncate pr-2" title={providerDisplayName(candidate.provider)}>
                      {providerDisplayName(candidate.provider)}
                    </span>
                    <span className="truncate pr-2 text-slate-500 dark:text-slate-400" title={candidate.source}>
                      {candidate.source}
                    </span>
                    <span className="text-xs font-bold uppercase tracking-wide text-indigo-600 dark:text-indigo-300">
                      {candidate.import_kind === "aws_local" ? "AWS Local" : "Cloud"}
                    </span>
                  </label>
                ))
              )}
            </div>
          </div>

          {(importResultSummary || importInvalidItems.length > 0 || importExecutionFailures.length > 0 || importRenameMappings.length > 0) && (
            <div className="mt-4 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-900/40 p-3">
              {importResultSummary && (
                <p className="text-sm font-semibold text-slate-700 dark:text-slate-200">{importResultSummary}</p>
              )}
              {importRenameMappings.length > 0 && (
                <div className="mt-2">
                  <p className="text-xs font-bold uppercase tracking-wide text-blue-700 dark:text-blue-400">
                    Renamed Accounts ({importRenameMappings.length})
                  </p>
                  <div className="mt-1 max-h-28 overflow-y-auto text-xs text-slate-600 dark:text-slate-300 space-y-1 pr-1">
                    {importRenameMappings.map((item, idx) => (
                      <p key={`rename-${idx}`}>
                        [{providerDisplayName(item.provider)}] {item.original} {"->"} {item.imported}
                      </p>
                    ))}
                  </div>
                </div>
              )}
              {importInvalidItems.length > 0 && (
                <div className="mt-2">
                  <p className="text-xs font-bold uppercase tracking-wide text-amber-700 dark:text-amber-400">
                    Invalid Source Rows ({importInvalidItems.length})
                  </p>
                  <div className="mt-1 max-h-28 overflow-y-auto text-xs text-slate-600 dark:text-slate-300 space-y-1 pr-1">
                    {importInvalidItems.map((item, idx) => (
                      <p key={`invalid-${idx}`}>{item}</p>
                    ))}
                  </div>
                </div>
              )}
              {importExecutionFailures.length > 0 && (
                <div className="mt-2">
                  <p className="text-xs font-bold uppercase tracking-wide text-red-700 dark:text-red-400">
                    Import Failures ({importExecutionFailures.length})
                  </p>
                  <div className="mt-1 max-h-28 overflow-y-auto text-xs text-slate-600 dark:text-slate-300 space-y-1 pr-1">
                    {importExecutionFailures.map((item, idx) => (
                      <p key={`exec-fail-${idx}`}>{item}</p>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}

          <p className="mt-3 text-xs text-slate-500 dark:text-slate-400">
            AWS imports are saved to local AWS profiles so scan/test behavior stays consistent.
          </p>

          <div className="mt-6 flex justify-end gap-3">
            <button
              onClick={onClose}
              disabled={importingAccounts}
              className="px-4 py-3 text-slate-500 dark:text-slate-300 hover:text-slate-700 dark:hover:text-white transition-colors font-medium disabled:opacity-50 disabled:cursor-not-allowed"
            >
              Cancel
            </button>
            <button
              onClick={handleImportSelectedAccounts}
              disabled={importingAccounts || importCandidates.length === 0}
              className="inline-flex items-center bg-indigo-600 hover:bg-indigo-700 text-white px-5 py-3 rounded-lg font-bold transition-all shadow-lg shadow-indigo-500/20 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {importingAccounts ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <Upload className="w-5 h-5 mr-2" />}
              {importingAccounts ? "Importing..." : "Import Selected"}
            </button>
          </div>
        </div>
      </div>
    </>
  );
}
