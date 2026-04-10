import { Eye, EyeOff } from "lucide-react";

type FormSetter<T> = (value: T | ((prev: T) => T)) => void;

interface ExtendedProviderCredentialFieldsProps {
  selectedProvider: string;
  showCredentialSecrets: boolean;
  onToggleCredentialSecrets: () => void;
  wasabiForm: any;
  setWasabiForm: FormSetter<any>;
  backblazeForm: any;
  setBackblazeForm: FormSetter<any>;
  idriveForm: any;
  setIdriveForm: FormSetter<any>;
  storjForm: any;
  setStorjForm: FormSetter<any>;
  dreamhostForm: any;
  setDreamhostForm: FormSetter<any>;
  cloudianForm: any;
  setCloudianForm: FormSetter<any>;
  s3compatibleForm: any;
  setS3compatibleForm: FormSetter<any>;
  minioForm: any;
  setMinioForm: FormSetter<any>;
  cephForm: any;
  setCephForm: FormSetter<any>;
  lyveForm: any;
  setLyveForm: FormSetter<any>;
  dellForm: any;
  setDellForm: FormSetter<any>;
  storagegridForm: any;
  setStoragegridForm: FormSetter<any>;
  scalityForm: any;
  setScalityForm: FormSetter<any>;
  hcpForm: any;
  setHcpForm: FormSetter<any>;
  qumuloForm: any;
  setQumuloForm: FormSetter<any>;
  nutanixForm: any;
  setNutanixForm: FormSetter<any>;
  flashbladeForm: any;
  setFlashbladeForm: FormSetter<any>;
  greenlakeForm: any;
  setGreenlakeForm: FormSetter<any>;
  ionosForm: any;
  setIonosForm: FormSetter<any>;
  oracleForm: any;
  setOracleForm: FormSetter<any>;
  ibmForm: any;
  setIbmForm: FormSetter<any>;
  ovhForm: any;
  setOvhForm: FormSetter<any>;
  huaweiForm: any;
  setHuaweiForm: FormSetter<any>;
  tencentForm: any;
  setTencentForm: FormSetter<any>;
  volcForm: any;
  setVolcForm: FormSetter<any>;
  baiduForm: any;
  setBaiduForm: FormSetter<any>;
  tianyiForm: any;
  setTianyiForm: FormSetter<any>;
}

export function ExtendedProviderCredentialFields(props: ExtendedProviderCredentialFieldsProps) {
  const {
    selectedProvider,
    showCredentialSecrets,
    onToggleCredentialSecrets,
    wasabiForm, setWasabiForm,
    backblazeForm, setBackblazeForm,
    idriveForm, setIdriveForm,
    storjForm, setStorjForm,
    dreamhostForm, setDreamhostForm,
    cloudianForm, setCloudianForm,
    s3compatibleForm, setS3compatibleForm,
    minioForm, setMinioForm,
    cephForm, setCephForm,
    lyveForm, setLyveForm,
    dellForm, setDellForm,
    storagegridForm, setStoragegridForm,
    scalityForm, setScalityForm,
    hcpForm, setHcpForm,
    qumuloForm, setQumuloForm,
    nutanixForm, setNutanixForm,
    flashbladeForm, setFlashbladeForm,
    greenlakeForm, setGreenlakeForm,
    ionosForm, setIonosForm,
    oracleForm, setOracleForm,
    ibmForm, setIbmForm,
    ovhForm, setOvhForm,
    huaweiForm, setHuaweiForm,
    tencentForm, setTencentForm,
    volcForm, setVolcForm,
    baiduForm, setBaiduForm,
    tianyiForm, setTianyiForm,
  } = props;

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
      {selectedProvider === "wasabi" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={wasabiForm.name} onChange={e => setWasabiForm({ ...wasabiForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={wasabiForm.region} onChange={e => setWasabiForm({ ...wasabiForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={wasabiForm.endpoint} onChange={e => setWasabiForm({ ...wasabiForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: s3.<region>.wasabisys.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={wasabiForm.access_key} onChange={e => setWasabiForm({ ...wasabiForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={wasabiForm.secret_key} onChange={e => setWasabiForm({ ...wasabiForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "backblaze" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={backblazeForm.name} onChange={e => setBackblazeForm({ ...backblazeForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={backblazeForm.region} onChange={e => setBackblazeForm({ ...backblazeForm, region: e.target.value })} placeholder="Region (e.g. us-west-004)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={backblazeForm.endpoint} onChange={e => setBackblazeForm({ ...backblazeForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: s3.<region>.backblazeb2.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={backblazeForm.access_key} onChange={e => setBackblazeForm({ ...backblazeForm, access_key: e.target.value })} placeholder="Application Key ID" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={backblazeForm.secret_key} onChange={e => setBackblazeForm({ ...backblazeForm, secret_key: e.target.value })} placeholder="Application Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "idrive" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={idriveForm.name} onChange={e => setIdriveForm({ ...idriveForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={idriveForm.region} onChange={e => setIdriveForm({ ...idriveForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={idriveForm.endpoint} onChange={e => setIdriveForm({ ...idriveForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: s3.<region>.idrivee2.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={idriveForm.access_key} onChange={e => setIdriveForm({ ...idriveForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={idriveForm.secret_key} onChange={e => setIdriveForm({ ...idriveForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "storj" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={storjForm.name} onChange={e => setStorjForm({ ...storjForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={storjForm.region} onChange={e => setStorjForm({ ...storjForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={storjForm.endpoint} onChange={e => setStorjForm({ ...storjForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: gateway.storjshare.io)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={storjForm.access_key} onChange={e => setStorjForm({ ...storjForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={storjForm.secret_key} onChange={e => setStorjForm({ ...storjForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "dreamhost" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={dreamhostForm.name} onChange={e => setDreamhostForm({ ...dreamhostForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={dreamhostForm.region} onChange={e => setDreamhostForm({ ...dreamhostForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={dreamhostForm.endpoint} onChange={e => setDreamhostForm({ ...dreamhostForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: objects-<region>.dream.io)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={dreamhostForm.access_key} onChange={e => setDreamhostForm({ ...dreamhostForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={dreamhostForm.secret_key} onChange={e => setDreamhostForm({ ...dreamhostForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "cloudian" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cloudianForm.name} onChange={e => setCloudianForm({ ...cloudianForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cloudianForm.region} onChange={e => setCloudianForm({ ...cloudianForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cloudianForm.endpoint} onChange={e => setCloudianForm({ ...cloudianForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: s3.<region>.cloudian.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cloudianForm.access_key} onChange={e => setCloudianForm({ ...cloudianForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={cloudianForm.secret_key} onChange={e => setCloudianForm({ ...cloudianForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "s3compatible" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={s3compatibleForm.name} onChange={e => setS3compatibleForm({ ...s3compatibleForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={s3compatibleForm.region} onChange={e => setS3compatibleForm({ ...s3compatibleForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={s3compatibleForm.endpoint} onChange={e => setS3compatibleForm({ ...s3compatibleForm, endpoint: e.target.value })} placeholder="S3 Endpoint (required, e.g. s3.example.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={s3compatibleForm.access_key} onChange={e => setS3compatibleForm({ ...s3compatibleForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={s3compatibleForm.secret_key} onChange={e => setS3compatibleForm({ ...s3compatibleForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "minio" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={minioForm.name} onChange={e => setMinioForm({ ...minioForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={minioForm.region} onChange={e => setMinioForm({ ...minioForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={minioForm.endpoint} onChange={e => setMinioForm({ ...minioForm, endpoint: e.target.value })} placeholder="Endpoint (default: http://localhost:9000)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={minioForm.access_key} onChange={e => setMinioForm({ ...minioForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={minioForm.secret_key} onChange={e => setMinioForm({ ...minioForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "ceph" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cephForm.name} onChange={e => setCephForm({ ...cephForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cephForm.region} onChange={e => setCephForm({ ...cephForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cephForm.endpoint} onChange={e => setCephForm({ ...cephForm, endpoint: e.target.value })} placeholder="Endpoint (default: http://localhost:7480)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={cephForm.access_key} onChange={e => setCephForm({ ...cephForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={cephForm.secret_key} onChange={e => setCephForm({ ...cephForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "lyve" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={lyveForm.name} onChange={e => setLyveForm({ ...lyveForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={lyveForm.region} onChange={e => setLyveForm({ ...lyveForm, region: e.target.value })} placeholder="Region (e.g. us-west-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={lyveForm.endpoint} onChange={e => setLyveForm({ ...lyveForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: s3.<region>.lyvecloud.seagate.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={lyveForm.access_key} onChange={e => setLyveForm({ ...lyveForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={lyveForm.secret_key} onChange={e => setLyveForm({ ...lyveForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "dell" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={dellForm.name} onChange={e => setDellForm({ ...dellForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={dellForm.region} onChange={e => setDellForm({ ...dellForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={dellForm.endpoint} onChange={e => setDellForm({ ...dellForm, endpoint: e.target.value })} placeholder="Endpoint (required, e.g. object.ecs.example.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={dellForm.access_key} onChange={e => setDellForm({ ...dellForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={dellForm.secret_key} onChange={e => setDellForm({ ...dellForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "storagegrid" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={storagegridForm.name} onChange={e => setStoragegridForm({ ...storagegridForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={storagegridForm.region} onChange={e => setStoragegridForm({ ...storagegridForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={storagegridForm.endpoint} onChange={e => setStoragegridForm({ ...storagegridForm, endpoint: e.target.value })} placeholder="Endpoint (required, e.g. s3.storagegrid.example.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={storagegridForm.access_key} onChange={e => setStoragegridForm({ ...storagegridForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={storagegridForm.secret_key} onChange={e => setStoragegridForm({ ...storagegridForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "scality" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={scalityForm.name} onChange={e => setScalityForm({ ...scalityForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={scalityForm.region} onChange={e => setScalityForm({ ...scalityForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={scalityForm.endpoint} onChange={e => setScalityForm({ ...scalityForm, endpoint: e.target.value })} placeholder="Endpoint (required, e.g. s3.scality.example.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={scalityForm.access_key} onChange={e => setScalityForm({ ...scalityForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={scalityForm.secret_key} onChange={e => setScalityForm({ ...scalityForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "hcp" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={hcpForm.name} onChange={e => setHcpForm({ ...hcpForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={hcpForm.region} onChange={e => setHcpForm({ ...hcpForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={hcpForm.endpoint} onChange={e => setHcpForm({ ...hcpForm, endpoint: e.target.value })} placeholder="Endpoint (required, e.g. hcp.example.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={hcpForm.access_key} onChange={e => setHcpForm({ ...hcpForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={hcpForm.secret_key} onChange={e => setHcpForm({ ...hcpForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "qumulo" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={qumuloForm.name} onChange={e => setQumuloForm({ ...qumuloForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={qumuloForm.region} onChange={e => setQumuloForm({ ...qumuloForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={qumuloForm.endpoint} onChange={e => setQumuloForm({ ...qumuloForm, endpoint: e.target.value })} placeholder="Endpoint (required, e.g. s3.qumulo.example.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={qumuloForm.access_key} onChange={e => setQumuloForm({ ...qumuloForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={qumuloForm.secret_key} onChange={e => setQumuloForm({ ...qumuloForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "nutanix" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={nutanixForm.name} onChange={e => setNutanixForm({ ...nutanixForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={nutanixForm.region} onChange={e => setNutanixForm({ ...nutanixForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={nutanixForm.endpoint} onChange={e => setNutanixForm({ ...nutanixForm, endpoint: e.target.value })} placeholder="Endpoint (required, e.g. objects.nutanix.example.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={nutanixForm.access_key} onChange={e => setNutanixForm({ ...nutanixForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={nutanixForm.secret_key} onChange={e => setNutanixForm({ ...nutanixForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "flashblade" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={flashbladeForm.name} onChange={e => setFlashbladeForm({ ...flashbladeForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={flashbladeForm.region} onChange={e => setFlashbladeForm({ ...flashbladeForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={flashbladeForm.endpoint} onChange={e => setFlashbladeForm({ ...flashbladeForm, endpoint: e.target.value })} placeholder="Endpoint (required, e.g. s3.flashblade.example.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={flashbladeForm.access_key} onChange={e => setFlashbladeForm({ ...flashbladeForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={flashbladeForm.secret_key} onChange={e => setFlashbladeForm({ ...flashbladeForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "greenlake" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={greenlakeForm.name} onChange={e => setGreenlakeForm({ ...greenlakeForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={greenlakeForm.region} onChange={e => setGreenlakeForm({ ...greenlakeForm, region: e.target.value })} placeholder="Region (e.g. us-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={greenlakeForm.endpoint} onChange={e => setGreenlakeForm({ ...greenlakeForm, endpoint: e.target.value })} placeholder="Endpoint (required, e.g. s3.greenlake.example.com)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={greenlakeForm.access_key} onChange={e => setGreenlakeForm({ ...greenlakeForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={greenlakeForm.secret_key} onChange={e => setGreenlakeForm({ ...greenlakeForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "ionos" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ionosForm.name} onChange={e => setIonosForm({ ...ionosForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ionosForm.endpoint} onChange={e => setIonosForm({ ...ionosForm, endpoint: e.target.value })} placeholder="Endpoint (optional, default: api.ionos.com)" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={ionosForm.token} onChange={e => setIonosForm({ ...ionosForm, token: e.target.value })} placeholder="IONOS API Token" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "oracle" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={oracleForm.name} onChange={e => setOracleForm({ ...oracleForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={oracleForm.tenancy_id} onChange={e => setOracleForm({ ...oracleForm, tenancy_id: e.target.value })} placeholder="Tenancy OCID" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={oracleForm.user_id} onChange={e => setOracleForm({ ...oracleForm, user_id: e.target.value })} placeholder="User OCID" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={oracleForm.fingerprint} onChange={e => setOracleForm({ ...oracleForm, fingerprint: e.target.value })} placeholder="Fingerprint" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={oracleForm.region} onChange={e => setOracleForm({ ...oracleForm, region: e.target.value })} placeholder="Region" />
          <textarea rows={3} className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white font-mono text-base outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={oracleForm.private_key} onChange={e => setOracleForm({ ...oracleForm, private_key: e.target.value })} placeholder="Private Key (PEM)..." />
        </>
      )}
      {selectedProvider === "ibm" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ibmForm.name} onChange={e => setIbmForm({ ...ibmForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ibmForm.api_key} onChange={e => setIbmForm({ ...ibmForm, api_key: e.target.value })} placeholder="IBM Cloud API Key" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ibmForm.region} onChange={e => setIbmForm({ ...ibmForm, region: e.target.value })} placeholder="Region (e.g. us-south)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ibmForm.cos_endpoint} onChange={e => setIbmForm({ ...ibmForm, cos_endpoint: e.target.value })} placeholder="COS Endpoint (optional, e.g. s3.us-south.cloud-object-storage.appdomain.cloud)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ibmForm.cos_service_instance_id} onChange={e => setIbmForm({ ...ibmForm, cos_service_instance_id: e.target.value })} placeholder="COS Service Instance ID (optional)" />
        </>
      )}
      {selectedProvider === "ovh" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ovhForm.name} onChange={e => setOvhForm({ ...ovhForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ovhForm.endpoint} onChange={e => setOvhForm({ ...ovhForm, endpoint: e.target.value })} placeholder="Endpoint (eu/us/ca or full URL)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ovhForm.project_id} onChange={e => setOvhForm({ ...ovhForm, project_id: e.target.value })} placeholder="Project ID (optional)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={ovhForm.application_key} onChange={e => setOvhForm({ ...ovhForm, application_key: e.target.value })} placeholder="Application Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={ovhForm.application_secret} onChange={e => setOvhForm({ ...ovhForm, application_secret: e.target.value })} placeholder="Application Secret" />
            {secretToggleButton}
          </div>
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={ovhForm.consumer_key} onChange={e => setOvhForm({ ...ovhForm, consumer_key: e.target.value })} placeholder="Consumer Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "huawei" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={huaweiForm.name} onChange={e => setHuaweiForm({ ...huaweiForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={huaweiForm.region} onChange={e => setHuaweiForm({ ...huaweiForm, region: e.target.value })} placeholder="Region (e.g. cn-north-4)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={huaweiForm.project_id} onChange={e => setHuaweiForm({ ...huaweiForm, project_id: e.target.value })} placeholder="Project ID" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={huaweiForm.access_key} onChange={e => setHuaweiForm({ ...huaweiForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={huaweiForm.secret_key} onChange={e => setHuaweiForm({ ...huaweiForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "tencent" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={tencentForm.name} onChange={e => setTencentForm({ ...tencentForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={tencentForm.region} onChange={e => setTencentForm({ ...tencentForm, region: e.target.value })} placeholder="Region (e.g. ap-guangzhou)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={tencentForm.secret_id} onChange={e => setTencentForm({ ...tencentForm, secret_id: e.target.value })} placeholder="Secret ID" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={tencentForm.secret_key} onChange={e => setTencentForm({ ...tencentForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "volcengine" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={volcForm.name} onChange={e => setVolcForm({ ...volcForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={volcForm.region} onChange={e => setVolcForm({ ...volcForm, region: e.target.value })} placeholder="Region (e.g. cn-beijing)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={volcForm.access_key} onChange={e => setVolcForm({ ...volcForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={volcForm.secret_key} onChange={e => setVolcForm({ ...volcForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "baidu" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={baiduForm.name} onChange={e => setBaiduForm({ ...baiduForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={baiduForm.region} onChange={e => setBaiduForm({ ...baiduForm, region: e.target.value })} placeholder="Region (e.g. bj)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={baiduForm.access_key} onChange={e => setBaiduForm({ ...baiduForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={baiduForm.secret_key} onChange={e => setBaiduForm({ ...baiduForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
      {selectedProvider === "tianyi" && (
        <>
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={tianyiForm.name} onChange={e => setTianyiForm({ ...tianyiForm, name: e.target.value })} placeholder="Account Name" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={tianyiForm.region} onChange={e => setTianyiForm({ ...tianyiForm, region: e.target.value })} placeholder="Region (e.g. cn-east-1)" />
          <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400" value={tianyiForm.access_key} onChange={e => setTianyiForm({ ...tianyiForm, access_key: e.target.value })} placeholder="Access Key" />
          <div className="relative">
            <input className="w-full p-4 border border-slate-200 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-slate-900 dark:text-white outline-none focus:ring-2 focus:ring-indigo-500 placeholder-slate-400 pr-12" type={showCredentialSecrets ? "text" : "password"} value={tianyiForm.secret_key} onChange={e => setTianyiForm({ ...tianyiForm, secret_key: e.target.value })} placeholder="Secret Key" />
            {secretToggleButton}
          </div>
        </>
      )}
    </>
  );
}
