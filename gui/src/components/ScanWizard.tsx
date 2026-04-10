import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { 
  Cloud, Check, ChevronRight, Loader2, Play, 
  AlertTriangle, Clock, ShieldCheck, Zap, Gauge 
} from "lucide-react";
import { Modal } from "./Modal";

interface CloudProfile {
  id: string;
  provider: string;
  name: string;
  credentials?: string;
  proxy_profile_id?: string | null;
  timeout_seconds?: number;
  policy_custom?: string;
}

interface ScanWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onScanComplete: (results: any[]) => void;
  demoMode?: boolean;
}

type Step = "select" | "configure" | "scanning" | "complete";

export function ScanWizard({ isOpen, onClose, onScanComplete, demoMode = false }: ScanWizardProps) {
  const [step, setStep] = useState<Step>("select");
  const [profiles, setProfiles] = useState<CloudProfile[]>([]);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  
  // Config
  const [timeout, setTimeout] = useState(10);
  const [policyMode, setPolicyMode] = useState<"conservative" | "standard" | "aggressive">("standard");
  
  // Scan State
  const [scanProgress, setScanProgress] = useState({ current: 0, total: 0, message: "Initializing..." });
  const [error, setError] = useState<string | null>(null);
  const isQuotaProtectedError = Boolean(error && error.toLowerCase().includes("not counted toward your scan quota"));

  useEffect(() => {
    if (isOpen) {
      loadProfiles();
      setStep("select");
      setSelectedIds([]);
      setError(null);
    }
  }, [isOpen]);

  // Load saved config when an account is selected (use first one as template)
  useEffect(() => {
    if (selectedIds.length > 0) {
      const p = profiles.find(x => x.id === selectedIds[0]);
      if (p) {
        if (p.timeout_seconds) setTimeout(p.timeout_seconds);
        // Heuristic to determine policy mode from custom JSON
        if (p.policy_custom) {
          try {
            const pol = JSON.parse(p.policy_custom);
            if (pol.cpu_percent >= 5) setPolicyMode("aggressive");
            else if (pol.cpu_percent < 2) setPolicyMode("conservative");
            else setPolicyMode("standard");
          } catch(e) {}
        }
      }
    }
  }, [selectedIds]);

  useEffect(() => {
    const unlisten = listen("scan-progress", (event: any) => {
      setScanProgress(event.payload);
    });
    return () => { unlisten.then(f => f()); };
  }, []);

  function goToSelect() {
    setError(null);
    setStep("select");
  }

  function goToConfigure() {
    setError(null);
    setStep("configure");
  }

  async function loadProfiles() {
    if (demoMode) {
        setProfiles([{ id: "demo", provider: "AWS", name: "Demo Environment" }]);
        setSelectedIds(["demo"]);
        return;
    }

    try {
      const dbProfiles = await invoke<CloudProfile[]>("list_cloud_profiles");
      
      // AWS Local
      let localAws: any[] = [];
      try { localAws = await invoke("list_aws_profiles"); } catch (e) {}
      
      const combined: CloudProfile[] = [
        ...dbProfiles,
        ...localAws.map((p: any) => ({
          id: `aws_local:${p.name}`,
          provider: 'AWS (Local)',
          name: p.name
        }))
      ];
      setProfiles(combined);
      
      // Auto-select if only one
      if (combined.length === 1) setSelectedIds([combined[0].id]);
    } catch (e) {
      console.error(e);
    }
  }

  async function handleStartScan() {
    if (selectedIds.length === 0) return;
    setStep("scanning");
    setError(null);
    setScanProgress({ current: 0, total: 10, message: "Starting scan..." });

    // Update DB config for ALL selected profiles (except local AWS)
    for (const pid of selectedIds) {
        if (!pid.startsWith("aws_local:") && pid !== "demo") {
            const profile = profiles.find(p => p.id === pid);
            // We rely on backend update_cloud_profile to handle config update.
            // Note: Since we don't have credentials in the partial profile list here if list_cloud_profiles doesn't return them,
            // we might have an issue. However, standard list_cloud_profiles implementation usually returns credentials.
            // Let's assume it does.
            if (profile) {
                 let cpu = 2.0; let days = 7;
                 if (policyMode === "conservative") { cpu = 1.0; days = 14; }
                 if (policyMode === "aggressive") { cpu = 10.0; days = 3; }
                 const policyJson = JSON.stringify({ cpu_percent: cpu, network_mb: 5.0, lookback_days: days });
                 
                 try {
                     // We need to pass credentials to update.
                     // The CloudProfile interface should have credentials if the backend returns it.
                     // Let's assume the backend returns it (it does in `db.rs`).
                     // But we didn't add it to the interface at the top. Let's add it now.
                     // Actually, I can't easily add it to the interface without potentially breaking other things if I'm wrong.
                     // But I can cast to `any` to access it if it exists at runtime.
                     await invoke("update_cloud_profile", {
                         id: profile.id,
                         provider: profile.provider,
                         name: profile.name,
                         credentials: profile.credentials || "",
                         timeout: timeout,
                         policy: policyJson,
                         proxy_profile_id: profile.proxy_profile_id ?? null,
                     });
                 } catch (e) {
                     console.warn(`Failed to save config for ${pid}`, e);
                 }
            }
        }
    }

    try {
      const results = await invoke<any[]>("run_scan", { 
        licenseKey: null,
        awsProfile: null, 
        awsRegion: null,
        selectedAccounts: demoMode ? null : selectedIds,
        demoMode: demoMode
      });
      
      onScanComplete(results);
      onClose(); 
    } catch (e) {
      setError(String(e));
      setStep("configure"); 
    }
  }

  return (
    <Modal isOpen={isOpen} onClose={onClose} title="" footer={null}>
      <div className="p-1">
        {/* Header Steps */}
        <div className="flex items-center justify-between mb-8 px-2">
            <div className={`flex flex-col items-center ${step === 'select' ? 'text-indigo-600 dark:text-indigo-400' : 'text-slate-400'}`}>
                <div className={`w-10 h-10 rounded-full flex items-center justify-center mb-1 text-lg font-bold border-2 ${step === 'select' || step === 'configure' || step === 'scanning' ? 'border-indigo-600 bg-indigo-50 text-indigo-600 dark:text-indigo-400' : 'border-slate-200'}`}>1</div>
                <span className="text-[10px] font-bold uppercase">Target</span>
            </div>
            <div className="flex-1 h-[2px] bg-slate-100 mx-2">
                <div className={`h-full bg-indigo-600 transition-all ${step === 'configure' || step === 'scanning' ? 'w-full' : 'w-0'}`}></div>
            </div>
            <div className={`flex flex-col items-center ${step === 'configure' ? 'text-indigo-600 dark:text-indigo-400' : 'text-slate-400'}`}>
                <div className={`w-10 h-10 rounded-full flex items-center justify-center mb-1 text-lg font-bold border-2 ${step === 'configure' || step === 'scanning' ? 'border-indigo-600 bg-indigo-50 text-indigo-600 dark:text-indigo-400' : 'border-slate-200'}`}>2</div>
                <span className="text-[10px] font-bold uppercase">Strategy</span>
            </div>
            <div className="flex-1 h-[2px] bg-slate-100 mx-2">
                <div className={`h-full bg-indigo-600 transition-all ${step === 'scanning' ? 'w-full' : 'w-0'}`}></div>
            </div>
            <div className={`flex flex-col items-center ${step === 'scanning' ? 'text-indigo-600 dark:text-indigo-400' : 'text-slate-400'}`}>
                <div className={`w-10 h-10 rounded-full flex items-center justify-center mb-1 text-lg font-bold border-2 ${step === 'scanning' ? 'border-indigo-600 bg-indigo-50 text-indigo-600 dark:text-indigo-400' : 'border-slate-200'}`}>3</div>
                <span className="text-[10px] font-bold uppercase">Diagnose</span>
            </div>
        </div>

        {/* Step 1: Select Account */}
        {step === "select" && (
            <div className="space-y-4 animate-in slide-in-from-right-4 fade-in">
                <div className="flex justify-between items-center">
                    <h3 className="text-xl font-bold text-slate-900 dark:text-white">Select Accounts to Scan</h3>
                    {!demoMode && profiles.length > 0 && (
                        <button 
                            onClick={() => {
                                setError(null);
                                if (selectedIds.length === profiles.length) setSelectedIds([]);
                                else setSelectedIds(profiles.map(p => p.id));
                            }}
                            className="text-base font-bold text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 transition-colors"
                        >
                            {selectedIds.length === profiles.length ? "Deselect All" : "Select All"}
                        </button>
                    )}
                </div>
                <div className="space-y-2 max-h-[300px] overflow-y-auto">
                    {profiles.map(p => (
                        <div 
                            key={p.id}
                            onClick={() => {
                                setError(null);
                                if (selectedIds.includes(p.id)) setSelectedIds(selectedIds.filter(id => id !== p.id));
                                else setSelectedIds([...selectedIds, p.id]);
                            }}
                            className={`p-4 rounded-xl border-2 cursor-pointer transition-all flex items-center justify-between group ${selectedIds.includes(p.id) ? 'border-indigo-600 bg-indigo-50/50 dark:bg-indigo-900/20' : 'border-slate-100 dark:border-slate-700 hover:border-indigo-200 dark:hover:border-slate-600'}`}
                        >
                            <div className="flex items-center gap-3">
                                <div className={`p-3 rounded-lg ${selectedIds.includes(p.id) ? 'bg-indigo-100 text-indigo-600 dark:text-indigo-400' : 'bg-slate-100 text-slate-500'}`}>
                                    <Cloud className="w-5 h-5" />
                                </div>
                                <div>
                                    <p className="font-bold text-slate-900 dark:text-white">{p.name}</p>
                                    <p className="text-base text-slate-500">{p.provider}</p>
                                </div>
                            </div>
                            {selectedIds.includes(p.id) && <Check className="w-5 h-5 text-indigo-600 dark:text-indigo-400" />}
                        </div>
                    ))}
                    {profiles.length === 0 && (
                        <div className="text-center py-8 text-slate-400">No accounts found. Please add one in Configuration.</div>
                    )}
                </div>
                <button 
                    onClick={goToConfigure}
                    disabled={selectedIds.length === 0}
                    className="w-full py-3 bg-indigo-600 text-white rounded-xl font-bold flex items-center justify-center hover:bg-indigo-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed mt-4"
                >
                    Next Step {selectedIds.length > 0 && `(${selectedIds.length})`} <ChevronRight className="w-5 h-5 ml-1" />
                </button>
            </div>
        )}

        {/* Step 2: Configure Strategy */}
        {step === "configure" && (
            <div className="space-y-6 animate-in slide-in-from-right-4 fade-in">
                <div>
                    <h3 className="text-xl font-bold text-slate-900 dark:text-white mb-1">Scan Strategy</h3>
                    <p className="text-lg text-slate-500 mb-4">
                        Define flagging aggression. 
                        {selectedIds.length > 1 && <span className="text-indigo-600 dark:text-indigo-400 font-bold ml-1">Applying to {selectedIds.length} accounts.</span>}
                    </p>
                    
                    <div className="grid grid-cols-3 gap-3">
                        <div 
                            onClick={() => setPolicyMode("conservative")}
                            className={`p-3 rounded-lg border-2 cursor-pointer transition-all text-center ${policyMode === 'conservative' ? 'border-green-500 bg-green-50 dark:bg-green-900/20' : 'border-slate-100 dark:border-slate-700 opacity-60 hover:opacity-100'}`}
                        >
                            <ShieldCheck className={`w-6 h-6 mx-auto mb-2 ${policyMode === 'conservative' ? 'text-green-600 dark:text-green-400' : 'text-slate-400'}`} />
                            <div className="text-base font-bold text-slate-900 dark:text-white">Conservative</div>
                            <div className="text-[9px] text-slate-500 mt-1">CPU &lt; 1%</div>
                        </div>
                        <div 
                            onClick={() => setPolicyMode("standard")}
                            className={`p-3 rounded-lg border-2 cursor-pointer transition-all text-center ${policyMode === 'standard' ? 'border-indigo-600 bg-indigo-50 dark:bg-indigo-900/20' : 'border-slate-100 dark:border-slate-700 opacity-60 hover:opacity-100'}`}
                        >
                            <Gauge className={`w-6 h-6 mx-auto mb-2 ${policyMode === 'standard' ? 'text-indigo-600 dark:text-indigo-400' : 'text-slate-400'}`} />
                            <div className="text-base font-bold text-slate-900 dark:text-white">Standard</div>
                            <div className="text-[9px] text-slate-500 mt-1">CPU &lt; 2%</div>
                        </div>
                        <div 
                            onClick={() => setPolicyMode("aggressive")}
                            className={`p-3 rounded-lg border-2 cursor-pointer transition-all text-center ${policyMode === 'aggressive' ? 'border-red-500 bg-red-50 dark:bg-red-900/20' : 'border-slate-100 dark:border-slate-700 opacity-60 hover:opacity-100'}`}
                        >
                            <Zap className={`w-6 h-6 mx-auto mb-2 ${policyMode === 'aggressive' ? 'text-red-600 dark:text-red-400' : 'text-slate-400'}`} />
                            <div className="text-base font-bold text-slate-900 dark:text-white">Aggressive</div>
                            <div className="text-[9px] text-slate-500 mt-1">CPU &lt; 10%</div>
                        </div>
                    </div>
                </div>

                <div>
                    <div className="flex justify-between mb-2">
                        <h3 className="text-lg font-bold text-slate-900 dark:text-white flex items-center gap-2">
                            <Clock className="w-5 h-5" /> Connection Timeout
                        </h3>
                        <span className="text-base font-mono font-bold bg-slate-100 dark:bg-slate-700 px-2 py-0.5 rounded text-slate-600 dark:text-slate-300">{timeout}s</span>
                    </div>
                    <input 
                        type="range" 
                        min="5" max="120" step="5"
                        value={timeout}
                        onChange={(e) => setTimeout(parseInt(e.target.value))}
                        className="w-full h-2 bg-slate-200 rounded-lg appearance-none cursor-pointer accent-indigo-600"
                    />
                    <p className="text-base text-slate-400 mt-2">
                        {timeout > 30 ? "Extended timeout for large accounts." : "Standard timeout for quick scans."}
                    </p>
                </div>

                {error && (
                    <div className="bg-red-50 dark:bg-red-900/20 border border-red-100 dark:border-red-800 text-red-700 dark:text-red-300 p-3 rounded-lg text-sm">
                        <div className="flex items-start">
                            <AlertTriangle className="w-5 h-5 mr-2 mt-0.5 flex-shrink-0" />
                            <div>
                                <p className="font-semibold">Scan did not complete</p>
                                <p className="mt-1">{error}</p>
                                {isQuotaProtectedError && (
                                    <p className="mt-2 text-emerald-700 dark:text-emerald-300 font-semibold">
                                        This attempt was protected and not deducted from your scan quota.
                                    </p>
                                )}
                            </div>
                        </div>
                    </div>
                )}

                <div className="bg-slate-50 dark:bg-slate-800/70 border border-slate-200 dark:border-slate-700 rounded-lg p-3">
                    <p className="text-sm text-slate-700 dark:text-slate-300">
                        Scan fairness guarantee: if selected accounts return no cloud data due to connectivity or credential configuration issues, this attempt is not counted toward your scan quota.
                    </p>
                </div>

                <div className="flex gap-3 pt-4">
                    <button onClick={goToSelect} className="px-4 py-3 text-slate-500 font-bold hover:text-slate-700 dark:text-slate-200 transition-colors">Back</button>
                    <button 
                        onClick={handleStartScan}
                        className="flex-1 py-3 bg-indigo-600 text-white rounded-xl font-bold flex items-center justify-center hover:bg-indigo-700 transition-all shadow-lg shadow-indigo-500/20"
                    >
                        <Play className="w-5 h-5 mr-2" /> Start Scan
                    </button>
                </div>
            </div>
        )}

        {/* Step 3: Scanning */}
        {step === "scanning" && (
            <div className="flex flex-col items-center justify-center py-8 space-y-6 animate-in zoom-in-95 duration-300">
                 <div className="w-full max-w-xs space-y-2">
                    <div className="flex justify-between text-base font-bold text-slate-500 dark:text-slate-400 uppercase">
                        <span>Progress</span>
                        <span>{Math.round((scanProgress.current / Math.max(scanProgress.total, 1)) * 100)}%</span>
                    </div>
                    <div className="w-full bg-slate-100 dark:bg-slate-700 rounded-full h-3 overflow-hidden border border-slate-200 dark:border-slate-600">
                        <div 
                            className="bg-indigo-600 h-full rounded-full transition-all duration-300 ease-out" 
                            style={{ width: `${(scanProgress.current / Math.max(scanProgress.total, 1)) * 100}%` }}
                        ></div>
                    </div>
                 </div>
                 
                 <div className="relative">
                     <div className="relative bg-white dark:bg-slate-800 p-4 rounded-full shadow-sm border border-slate-100 dark:border-slate-700">
                        <Loader2 className="w-10 h-10 text-indigo-600 dark:text-indigo-400 animate-spin" />
                     </div>
                 </div>

                 <div className="text-center space-y-2 max-w-sm mx-auto">
                     <h3 className="text-xl font-bold text-slate-900 dark:text-white animate-pulse">{scanProgress.message}</h3>
                     <p className="text-base text-slate-400 dark:text-slate-500">
                         Please wait while we analyze your cloud resources...
                     </p>
                 </div>
            </div>
        )}
      </div>
    </Modal>
  );
}
