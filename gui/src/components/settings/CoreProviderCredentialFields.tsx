import { Eye, EyeOff } from "lucide-react";

type FormSetter<T> = (value: T | ((prev: T) => T)) => void;

interface CoreProviderCredentialFieldsProps {
  selectedProvider: string;
  showCredentialSecrets: boolean;
  onToggleCredentialSecrets: () => void;
  awsForm: any;
  setAwsForm: FormSetter<any>;
  azureForm: any;
  setAzureForm: FormSetter<any>;
  gcpForm: any;
  setGcpForm: FormSetter<any>;
  aliForm: any;
  setAliForm: FormSetter<any>;
  doForm: any;
  setDoForm: FormSetter<any>;
  cfForm: any;
  setCfForm: FormSetter<any>;
  vultrForm: any;
  setVultrForm: FormSetter<any>;
  linodeForm: any;
  setLinodeForm: FormSetter<any>;
  hetzForm: any;
  setHetzForm: FormSetter<any>;
  scwForm: any;
  setScwForm: FormSetter<any>;
  exoForm: any;
  setExoForm: FormSetter<any>;
  lwForm: any;
  setLwForm: FormSetter<any>;
  upcForm: any;
  setUpcForm: FormSetter<any>;
  gcoreForm: any;
  setGcoreForm: FormSetter<any>;
  contaboForm: any;
  setContaboForm: FormSetter<any>;
  civoForm: any;
  setCivoForm: FormSetter<any>;
  equinixForm: any;
  setEquinixForm: FormSetter<any>;
  rackspaceForm: any;
  setRackspaceForm: FormSetter<any>;
  openstackForm: any;
  setOpenstackForm: FormSetter<any>;
}

export function CoreProviderCredentialFields({
  selectedProvider,
  showCredentialSecrets,
  onToggleCredentialSecrets,
  awsForm,
  setAwsForm,
  azureForm,
  setAzureForm,
  gcpForm,
  setGcpForm,
  aliForm,
  setAliForm,
  doForm,
  setDoForm,
  cfForm,
  setCfForm,
  vultrForm,
  setVultrForm,
  linodeForm,
  setLinodeForm,
  hetzForm,
  setHetzForm,
  scwForm,
  setScwForm,
  exoForm,
  setExoForm,
  lwForm,
  setLwForm,
  upcForm,
  setUpcForm,
  gcoreForm,
  setGcoreForm,
  contaboForm,
  setContaboForm,
  civoForm,
  setCivoForm,
  equinixForm,
  setEquinixForm,
  rackspaceForm,
  setRackspaceForm,
  openstackForm,
  setOpenstackForm,
}: CoreProviderCredentialFieldsProps) {
  const secretToggleButton = (
    <button
      type="button"
      onClick={onToggleCredentialSecrets}
      className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-500 transition-colors hover:text-slate-700 dark:text-slate-300 dark:hover:text-white"
      title={showCredentialSecrets ? "Hide secret value" : "Show secret value"}
      aria-label={showCredentialSecrets ? "Hide secret value" : "Show secret value"}
    >
      {showCredentialSecrets ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
    </button>
  );

  return (
    <>
      {selectedProvider === "aws" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={awsForm.name} onChange={e => setAwsForm({ ...awsForm, name: e.target.value })} placeholder="Profile Name (e.g. prod)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={awsForm.region} onChange={e => setAwsForm({ ...awsForm, region: e.target.value })} placeholder="Default Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={awsForm.key} onChange={e => setAwsForm({ ...awsForm, key: e.target.value })} placeholder="Access Key ID" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={awsForm.secret} onChange={e => setAwsForm({ ...awsForm, secret: e.target.value })} placeholder="Secret Access Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "azure" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={azureForm.name} onChange={e => setAzureForm({ ...azureForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={azureForm.subscription_id} onChange={e => setAzureForm({ ...azureForm, subscription_id: e.target.value })} placeholder="Subscription ID" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={azureForm.tenant_id} onChange={e => setAzureForm({ ...azureForm, tenant_id: e.target.value })} placeholder="Tenant ID" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={azureForm.client_id} onChange={e => setAzureForm({ ...azureForm, client_id: e.target.value })} placeholder="Client ID" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={azureForm.client_secret} onChange={e => setAzureForm({ ...azureForm, client_secret: e.target.value })} placeholder="Client Secret" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "gcp" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={gcpForm.name} onChange={e => setGcpForm({ ...gcpForm, name: e.target.value })} placeholder="Account Name" />
          <textarea rows={5} className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white font-mono text-base outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={gcpForm.json_key} onChange={e => setGcpForm({ ...gcpForm, json_key: e.target.value })} placeholder="Paste JSON Key Content Here..." />
        </>
      )}
      {selectedProvider === "alibaba" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={aliForm.name} onChange={e => setAliForm({ ...aliForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={aliForm.region} onChange={e => setAliForm({ ...aliForm, region: e.target.value })} placeholder="Region ID (e.g. cn-hangzhou)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={aliForm.key} onChange={e => setAliForm({ ...aliForm, key: e.target.value })} placeholder="Access Key ID" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={aliForm.secret} onChange={e => setAliForm({ ...aliForm, secret: e.target.value })} placeholder="Access Key Secret" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "digitalocean" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={doForm.name} onChange={e => setDoForm({ ...doForm, name: e.target.value })} placeholder="Account Name" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={doForm.token} onChange={e => setDoForm({ ...doForm, token: e.target.value })} placeholder="Personal Access Token" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "cloudflare" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cfForm.name} onChange={e => setCfForm({ ...cfForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cfForm.account_id} onChange={e => setCfForm({ ...cfForm, account_id: e.target.value })} placeholder="Account ID" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={cfForm.token} onChange={e => setCfForm({ ...cfForm, token: e.target.value })} placeholder="API Token (Read Privileges)" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "vultr" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={vultrForm.name} onChange={e => setVultrForm({ ...vultrForm, name: e.target.value })} placeholder="Account Name" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={vultrForm.api_key} onChange={e => setVultrForm({ ...vultrForm, api_key: e.target.value })} placeholder="Vultr API Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {(selectedProvider === "linode" || selectedProvider === "akamai") && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={linodeForm.name} onChange={e => setLinodeForm({ ...linodeForm, name: e.target.value })} placeholder="Account Name" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={linodeForm.token} onChange={e => setLinodeForm({ ...linodeForm, token: e.target.value })} placeholder={selectedProvider === "akamai" ? "Akamai API Token" : "Personal Access Token"} />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "hetzner" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={hetzForm.name} onChange={e => setHetzForm({ ...hetzForm, name: e.target.value })} placeholder="Account Name" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={hetzForm.token} onChange={e => setHetzForm({ ...hetzForm, token: e.target.value })} placeholder="Hetzner API Token" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "scaleway" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={scwForm.name} onChange={e => setScwForm({ ...scwForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={scwForm.zones} onChange={e => setScwForm({ ...scwForm, zones: e.target.value })} placeholder="Zones (comma-separated, e.g. fr-par-1,nl-ams-1)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={scwForm.token} onChange={e => setScwForm({ ...scwForm, token: e.target.value })} placeholder="Scaleway API Token" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "exoscale" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={exoForm.name} onChange={e => setExoForm({ ...exoForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={exoForm.endpoint} onChange={e => setExoForm({ ...exoForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: api.exoscale.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={exoForm.api_key} onChange={e => setExoForm({ ...exoForm, api_key: e.target.value })} placeholder="API Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={exoForm.secret_key} onChange={e => setExoForm({ ...exoForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "leaseweb" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={lwForm.name} onChange={e => setLwForm({ ...lwForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={lwForm.endpoint} onChange={e => setLwForm({ ...lwForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: api.leaseweb.com)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={lwForm.token} onChange={e => setLwForm({ ...lwForm, token: e.target.value })} placeholder="Leaseweb API Token" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "upcloud" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={upcForm.name} onChange={e => setUpcForm({ ...upcForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={upcForm.endpoint} onChange={e => setUpcForm({ ...upcForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: api.upcloud.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={upcForm.username} onChange={e => setUpcForm({ ...upcForm, username: e.target.value })} placeholder="UpCloud Username" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={upcForm.password} onChange={e => setUpcForm({ ...upcForm, password: e.target.value })} placeholder="UpCloud Password" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "gcore" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={gcoreForm.name} onChange={e => setGcoreForm({ ...gcoreForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={gcoreForm.endpoint} onChange={e => setGcoreForm({ ...gcoreForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: api.gcore.com)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={gcoreForm.token} onChange={e => setGcoreForm({ ...gcoreForm, token: e.target.value })} placeholder="Gcore API Token" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "contabo" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={contaboForm.name} onChange={e => setContaboForm({ ...contaboForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={contaboForm.endpoint} onChange={e => setContaboForm({ ...contaboForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: api.contabo.com)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={contaboForm.token} onChange={e => setContaboForm({ ...contaboForm, token: e.target.value })} placeholder="API Token (optional)" />
            {secretToggleButton}
          </div>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={contaboForm.client_id} onChange={e => setContaboForm({ ...contaboForm, client_id: e.target.value })} placeholder="Client ID (optional)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={contaboForm.client_secret} onChange={e => setContaboForm({ ...contaboForm, client_secret: e.target.value })} placeholder="Client Secret (optional)" />
            {secretToggleButton}
          </div>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={contaboForm.username} onChange={e => setContaboForm({ ...contaboForm, username: e.target.value })} placeholder="Username (optional)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={contaboForm.password} onChange={e => setContaboForm({ ...contaboForm, password: e.target.value })} placeholder="Password (optional)" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "civo" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={civoForm.name} onChange={e => setCivoForm({ ...civoForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={civoForm.endpoint} onChange={e => setCivoForm({ ...civoForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: api.civo.com)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={civoForm.token} onChange={e => setCivoForm({ ...civoForm, token: e.target.value })} placeholder="Civo API Token" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "equinix" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={equinixForm.name} onChange={e => setEquinixForm({ ...equinixForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={equinixForm.endpoint} onChange={e => setEquinixForm({ ...equinixForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: api.equinix.com/metal/v1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={equinixForm.project_id} onChange={e => setEquinixForm({ ...equinixForm, project_id: e.target.value })} placeholder="Project ID (optional)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={equinixForm.token} onChange={e => setEquinixForm({ ...equinixForm, token: e.target.value })} placeholder="Equinix Metal API Token" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "rackspace" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={rackspaceForm.name} onChange={e => setRackspaceForm({ ...rackspaceForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={rackspaceForm.endpoint} onChange={e => setRackspaceForm({ ...rackspaceForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: dfw.servers.api.rackspacecloud.com/v2)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={rackspaceForm.project_id} onChange={e => setRackspaceForm({ ...rackspaceForm, project_id: e.target.value })} placeholder="Project ID (optional)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={rackspaceForm.token} onChange={e => setRackspaceForm({ ...rackspaceForm, token: e.target.value })} placeholder="Rackspace API Token" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "openstack" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={openstackForm.name} onChange={e => setOpenstackForm({ ...openstackForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={openstackForm.endpoint} onChange={e => setOpenstackForm({ ...openstackForm, endpoint: e.target.value })} placeholder="Endpoint (required, e.g. your-openstack-api/v2)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={openstackForm.project_id} onChange={e => setOpenstackForm({ ...openstackForm, project_id: e.target.value })} placeholder="Project/Tenant ID (optional)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={openstackForm.token} onChange={e => setOpenstackForm({ ...openstackForm, token: e.target.value })} placeholder="OpenStack API Token" />
            {secretToggleButton}
          </div>
        </>
      )}
    </>
  );
}
