export interface ProviderOption {
  value: string;
  label: string;
}

export const CLOUD_PROVIDER_OPTIONS: ProviderOption[] = [
  { value: "alibaba", label: "Alibaba Cloud" },
  { value: "aws", label: "AWS" },
  { value: "azure", label: "Azure" },
  { value: "baidu", label: "Baidu AI Cloud" },
  { value: "cloudflare", label: "Cloudflare" },
  { value: "digitalocean", label: "DigitalOcean" },
  { value: "gcp", label: "Google Cloud (GCP)" },
  { value: "huawei", label: "Huawei Cloud" },
  { value: "ibm", label: "IBM Cloud" },
  { value: "linode", label: "Linode" },
  { value: "akamai", label: "Akamai Connected Cloud" },
  { value: "hetzner", label: "Hetzner" },
  { value: "scaleway", label: "Scaleway" },
  { value: "exoscale", label: "Exoscale" },
  { value: "leaseweb", label: "Leaseweb" },
  { value: "upcloud", label: "UpCloud" },
  { value: "gcore", label: "Gcore" },
  { value: "contabo", label: "Contabo" },
  { value: "civo", label: "Civo" },
  { value: "equinix", label: "Equinix Metal" },
  { value: "rackspace", label: "Rackspace" },
  { value: "openstack", label: "OpenStack" },
  { value: "wasabi", label: "Wasabi" },
  { value: "backblaze", label: "Backblaze B2" },
  { value: "idrive", label: "IDrive e2" },
  { value: "storj", label: "Storj DCS" },
  { value: "dreamhost", label: "DreamHost DreamObjects" },
  { value: "cloudian", label: "Cloudian HyperStore (S3)" },
  { value: "s3compatible", label: "Generic S3-Compatible" },
  { value: "minio", label: "MinIO (S3-Compatible)" },
  { value: "ceph", label: "Ceph RGW (S3-Compatible)" },
  { value: "lyve", label: "Seagate Lyve Cloud" },
  { value: "dell", label: "Dell EMC ECS (S3)" },
  { value: "storagegrid", label: "NetApp StorageGRID (S3)" },
  { value: "scality", label: "Scality (S3)" },
  { value: "hcp", label: "Hitachi HCP (S3)" },
  { value: "qumulo", label: "Qumulo (S3)" },
  { value: "nutanix", label: "Nutanix Objects (S3)" },
  { value: "flashblade", label: "Pure Storage FlashBlade (S3)" },
  { value: "greenlake", label: "HPE GreenLake (S3)" },
  { value: "ionos", label: "IONOS Cloud" },
  { value: "oracle", label: "Oracle Cloud" },
  { value: "ovh", label: "OVHcloud" },
  { value: "tencent", label: "Tencent Cloud" },
  { value: "tianyi", label: "Tianyi Cloud" },
  { value: "volcengine", label: "Volcengine" },
  { value: "vultr", label: "Vultr" },
];

export const CLOUD_PROVIDER_FILTER_OPTIONS: ProviderOption[] = [
  { value: "All", label: "All Providers" },
  ...CLOUD_PROVIDER_OPTIONS,
];

function normalizeProvider(input: string): string {
  return input.toLowerCase().replace(/[^a-z0-9]/g, "");
}

export function resolveProviderValue(input: string): string {
  const normalizedInput = normalizeProvider(input);

  const match = CLOUD_PROVIDER_OPTIONS.find((provider) => {
    const valueKey = normalizeProvider(provider.value);
    const labelKey = normalizeProvider(provider.label);
    return (
      normalizedInput === valueKey ||
      normalizedInput === labelKey ||
      normalizedInput.includes(valueKey) ||
      valueKey.includes(normalizedInput) ||
      normalizedInput.includes(labelKey) ||
      labelKey.includes(normalizedInput)
    );
  });

  return match ? match.value : input;
}

export function matchesProviderFilter(selectedProvider: string, actualProvider: string): boolean {
  if (selectedProvider === "All") {
    return true;
  }

  const normalizedActual = normalizeProvider(actualProvider);
  const selected = CLOUD_PROVIDER_OPTIONS.find((provider) => provider.value === selectedProvider);

  if (!selected) {
    return normalizeProvider(selectedProvider) === normalizedActual;
  }

  const valueKey = normalizeProvider(selected.value);
  const labelKey = normalizeProvider(selected.label);

  return (
    normalizedActual === valueKey ||
    normalizedActual === labelKey ||
    normalizedActual.includes(valueKey) ||
    valueKey.includes(normalizedActual) ||
    normalizedActual.includes(labelKey) ||
    labelKey.includes(normalizedActual)
  );
}
