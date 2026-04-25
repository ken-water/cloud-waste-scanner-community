use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use crate::models::WastedResource;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KubeconfigContextRef {
    pub name: String,
    pub cluster: String,
    pub user: String,
    pub namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KubeconfigSummary {
    pub current_context: Option<String>,
    pub contexts: Vec<KubeconfigContextRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KubeconfigReadiness {
    pub status: String,
    pub provider_hint: String,
    pub active_context_source: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct K8sNodeSnapshot {
    pub name: String,
    pub provider_id: Option<String>,
    pub instance_type: Option<String>,
    pub allocatable_cpu_millicores: i64,
    pub allocatable_memory_mib: i64,
    pub requested_cpu_millicores: i64,
    pub requested_memory_mib: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct K8sPersistentVolumeSnapshot {
    pub name: String,
    pub capacity_gib: i64,
    pub phase: String,
    pub claim_namespace: Option<String>,
    pub claim_name: Option<String>,
    pub storage_class: Option<String>,
    pub volume_handle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct K8sPodSnapshot {
    pub namespace: String,
    pub name: String,
    pub node_name: Option<String>,
    pub phase: String,
    pub requested_cpu_millicores: i64,
    pub requested_memory_mib: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct K8sPersistentVolumeClaimSnapshot {
    pub namespace: String,
    pub name: String,
    pub phase: String,
    pub volume_name: Option<String>,
    pub requested_storage_gib: i64,
    pub storage_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct K8sServiceSnapshot {
    pub namespace: String,
    pub name: String,
    pub service_type: String,
    pub selector_count: usize,
    pub load_balancer_ingress_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct K8sWorkloadSnapshot {
    pub namespace: String,
    pub name: String,
    pub kind: String,
    pub replicas: i64,
    pub ready_replicas: i64,
    pub requested_cpu_millicores: i64,
    pub requested_memory_mib: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct RawKubeconfig {
    #[serde(rename = "current-context")]
    current_context: Option<String>,
    contexts: Option<Vec<RawNamedContext>>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawNamedContext {
    name: String,
    context: RawContext,
}

#[derive(Debug, Clone, Deserialize)]
struct RawContext {
    cluster: String,
    user: String,
    namespace: Option<String>,
}

pub fn parse_kubeconfig_summary(input: &str) -> Result<KubeconfigSummary, String> {
    let raw: RawKubeconfig =
        serde_yaml_ng::from_str(input).map_err(|e| format!("invalid kubeconfig yaml: {}", e))?;
    let contexts = raw
        .contexts
        .unwrap_or_default()
        .into_iter()
        .map(|c| KubeconfigContextRef {
            name: c.name,
            cluster: c.context.cluster,
            user: c.context.user,
            namespace: c.context.namespace,
        })
        .collect::<Vec<_>>();
    Ok(KubeconfigSummary {
        current_context: raw.current_context,
        contexts,
    })
}

pub fn load_kubeconfig_summary(path: &Path) -> Result<KubeconfigSummary, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read kubeconfig '{}': {}", path.display(), e))?;
    parse_kubeconfig_summary(&content)
}

pub fn resolve_active_context<'a>(
    summary: &'a KubeconfigSummary,
    preferred: Option<&str>,
) -> Option<&'a KubeconfigContextRef> {
    if let Some(target) = preferred {
        if let Some(found) = summary.contexts.iter().find(|ctx| ctx.name == target) {
            return Some(found);
        }
    }

    if let Some(current) = summary.current_context.as_deref() {
        if let Some(found) = summary.contexts.iter().find(|ctx| ctx.name == current) {
            return Some(found);
        }
    }

    summary.contexts.first()
}

pub fn resolve_active_context_source(
    summary: &KubeconfigSummary,
    preferred: Option<&str>,
) -> String {
    if let Some(target) = preferred {
        if summary.contexts.iter().any(|ctx| ctx.name == target) {
            return "preferred".to_string();
        }
    }

    if let Some(current) = summary.current_context.as_deref() {
        if summary.contexts.iter().any(|ctx| ctx.name == current) {
            return "current_context".to_string();
        }
    }

    if summary.contexts.is_empty() {
        "none".to_string()
    } else {
        "first_context".to_string()
    }
}

pub fn infer_cluster_provider(context: Option<&KubeconfigContextRef>) -> String {
    let Some(context) = context else {
        return "unknown".to_string();
    };
    let haystack = format!(
        "{} {} {}",
        context.name.to_ascii_lowercase(),
        context.cluster.to_ascii_lowercase(),
        context.user.to_ascii_lowercase()
    );
    if haystack.contains("eks") || haystack.contains("arn:aws:eks") {
        "eks".to_string()
    } else if haystack.contains("gke") || haystack.contains("container.googleapis.com") {
        "gke".to_string()
    } else if haystack.contains("aks") || haystack.contains("azmk8s") {
        "aks".to_string()
    } else {
        "generic".to_string()
    }
}

pub fn assess_kubeconfig_readiness(
    summary: &KubeconfigSummary,
    preferred: Option<&str>,
) -> KubeconfigReadiness {
    let active = resolve_active_context(summary, preferred);
    let provider_hint = infer_cluster_provider(active);
    let active_context_source = resolve_active_context_source(summary, preferred);
    let mut warnings = Vec::new();

    if summary.contexts.is_empty() {
        warnings.push("kubeconfig has no contexts".to_string());
    }
    if let Some(preferred) = preferred {
        if !summary.contexts.iter().any(|ctx| ctx.name == preferred) {
            warnings.push(format!("requested context '{}' was not found", preferred));
        }
    }
    if summary.current_context.is_none() {
        warnings.push("kubeconfig has no current-context".to_string());
    } else if let Some(current) = summary.current_context.as_deref() {
        if !summary.contexts.iter().any(|ctx| ctx.name == current) {
            warnings.push(format!("current-context '{}' was not found", current));
        }
    }

    let status = if active.is_some() { "ready" } else { "blocked" }.to_string();
    KubeconfigReadiness {
        status,
        provider_hint,
        active_context_source,
        warnings,
    }
}

pub fn node_request_utilization_ratio(node: &K8sNodeSnapshot) -> f64 {
    if node.allocatable_cpu_millicores <= 0 {
        return 0.0;
    }
    (node.requested_cpu_millicores.max(0) as f64 / node.allocatable_cpu_millicores as f64)
        .clamp(0.0, 1.0)
}

pub fn estimate_k8s_node_monthly_cost(instance_type: Option<&str>) -> f64 {
    let Some(instance_type) = instance_type else {
        return 80.0;
    };
    let normalized = instance_type.to_ascii_lowercase();
    if normalized.contains("4xlarge") || normalized.contains("n2-standard-16") {
        520.0
    } else if normalized.contains("2xlarge") || normalized.contains("n2-standard-8") {
        260.0
    } else if normalized.contains("xlarge") || normalized.contains("n2-standard-4") {
        130.0
    } else if normalized.contains("large") || normalized.contains("n2-standard-2") {
        70.0
    } else {
        80.0
    }
}

pub fn analyze_k8s_nodes(cluster_name: &str, nodes: &[K8sNodeSnapshot]) -> Vec<WastedResource> {
    nodes
        .iter()
        .filter_map(|node| {
            let utilization = node_request_utilization_ratio(node);
            let is_large = node.allocatable_cpu_millicores >= 4_000
                || node.allocatable_memory_mib >= 16 * 1024
                || node
                    .instance_type
                    .as_deref()
                    .map(|t| {
                        let t = t.to_ascii_lowercase();
                        t.contains("xlarge") || t.contains("standard-8") || t.contains("standard-16")
                    })
                    .unwrap_or(false);
            if utilization > 0.35 || !is_large {
                return None;
            }

            let monthly_cost = estimate_k8s_node_monthly_cost(node.instance_type.as_deref());
            Some(WastedResource {
                id: format!("{}/{}", cluster_name, node.name),
                resource_type: "K8s Node Rightsizing".to_string(),
                estimated_monthly_cost: monthly_cost * 0.45,
                details: format!(
                    "K8s node '{}' in cluster '{}' has {:.0}% requested CPU on a large allocatable baseline.",
                    node.name,
                    cluster_name,
                    utilization * 100.0
                ),
                provider: "kubernetes".to_string(),
                region: "cluster".to_string(),
                action_type: "RIGHTSIZE".to_string(),
            })
        })
        .collect()
}

pub fn analyze_k8s_persistent_volumes(
    cluster_name: &str,
    volumes: &[K8sPersistentVolumeSnapshot],
) -> Vec<WastedResource> {
    volumes
        .iter()
        .filter_map(|volume| {
            let phase = volume.phase.to_ascii_lowercase();
            let looks_orphaned = phase == "released"
                || phase == "failed"
                || (phase == "available" && volume.claim_name.is_none());
            if !looks_orphaned || volume.capacity_gib <= 0 {
                return None;
            }
            let estimated = (volume.capacity_gib as f64 * 0.08).max(1.0);
            Some(WastedResource {
                id: format!("{}/pv/{}", cluster_name, volume.name),
                resource_type: "K8s Orphan PV".to_string(),
                estimated_monthly_cost: estimated,
                details: format!(
                    "PersistentVolume '{}' is '{}' with {} GiB capacity. Validate retention, then delete or recycle backing storage.",
                    volume.name, volume.phase, volume.capacity_gib
                ),
                provider: "kubernetes".to_string(),
                region: "cluster".to_string(),
                action_type: "DELETE".to_string(),
            })
        })
        .collect()
}

pub fn parse_cpu_to_millicores(raw: &str) -> i64 {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return 0;
    }
    if let Some(value) = trimmed.strip_suffix('m') {
        return value.parse::<i64>().unwrap_or(0).max(0);
    }
    if let Some(value) = trimmed.strip_suffix('n') {
        return (value.parse::<f64>().unwrap_or(0.0) / 1_000_000.0).round() as i64;
    }
    if let Some(value) = trimmed.strip_suffix('u') {
        return (value.parse::<f64>().unwrap_or(0.0) / 1_000.0).round() as i64;
    }
    (trimmed.parse::<f64>().unwrap_or(0.0) * 1000.0).round() as i64
}

pub fn parse_memory_to_mib(raw: &str) -> i64 {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let units = [
        ("Ki", 1.0 / 1024.0),
        ("Mi", 1.0),
        ("Gi", 1024.0),
        ("Ti", 1024.0 * 1024.0),
        ("K", 1000.0 / 1024.0 / 1024.0),
        ("M", 1000.0 * 1000.0 / 1024.0 / 1024.0),
        ("G", 1000.0 * 1000.0 * 1000.0 / 1024.0 / 1024.0),
    ];
    for (suffix, multiplier) in units {
        if let Some(value) = trimmed.strip_suffix(suffix) {
            return (value.parse::<f64>().unwrap_or(0.0) * multiplier).round() as i64;
        }
    }
    (trimmed.parse::<f64>().unwrap_or(0.0) / 1024.0 / 1024.0).round() as i64
}

fn value_string<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = value;
    for part in path {
        current = current.get(*part)?;
    }
    current.as_str()
}

fn metadata_name(value: &Value) -> String {
    value_string(value, &["metadata", "name"])
        .unwrap_or("unknown")
        .to_string()
}

fn metadata_namespace(value: &Value) -> String {
    value_string(value, &["metadata", "namespace"])
        .unwrap_or("default")
        .to_string()
}

fn metadata_label(value: &Value, key: &str) -> Option<String> {
    value
        .get("metadata")?
        .get("labels")?
        .get(key)?
        .as_str()
        .map(|s| s.to_string())
}

pub fn parse_kubectl_pods_json(input: &str) -> Result<Vec<K8sPodSnapshot>, String> {
    let root: Value =
        serde_json::from_str(input).map_err(|e| format!("invalid kubectl pods json: {}", e))?;
    let items = root
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "kubectl pods json missing items array".to_string())?;
    let mut pods = Vec::new();
    for item in items {
        let mut cpu = 0;
        let mut memory = 0;
        if let Some(containers) = item
            .get("spec")
            .and_then(|v| v.get("containers"))
            .and_then(|v| v.as_array())
        {
            for container in containers {
                if let Some(raw) = value_string(container, &["resources", "requests", "cpu"]) {
                    cpu += parse_cpu_to_millicores(raw);
                }
                if let Some(raw) = value_string(container, &["resources", "requests", "memory"]) {
                    memory += parse_memory_to_mib(raw);
                }
            }
        }
        pods.push(K8sPodSnapshot {
            namespace: metadata_namespace(item),
            name: metadata_name(item),
            node_name: value_string(item, &["spec", "nodeName"]).map(|s| s.to_string()),
            phase: value_string(item, &["status", "phase"])
                .unwrap_or("Unknown")
                .to_string(),
            requested_cpu_millicores: cpu,
            requested_memory_mib: memory,
        });
    }
    Ok(pods)
}

pub fn parse_kubectl_pvcs_json(
    input: &str,
) -> Result<Vec<K8sPersistentVolumeClaimSnapshot>, String> {
    let root: Value =
        serde_json::from_str(input).map_err(|e| format!("invalid kubectl pvc json: {}", e))?;
    let items = root
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "kubectl pvc json missing items array".to_string())?;
    let mut claims = Vec::new();
    for item in items {
        claims.push(K8sPersistentVolumeClaimSnapshot {
            namespace: metadata_namespace(item),
            name: metadata_name(item),
            phase: value_string(item, &["status", "phase"])
                .unwrap_or("Unknown")
                .to_string(),
            volume_name: value_string(item, &["spec", "volumeName"]).map(|s| s.to_string()),
            requested_storage_gib: value_string(
                item,
                &["spec", "resources", "requests", "storage"],
            )
            .map(parse_memory_to_mib)
            .map(|mib| (mib as f64 / 1024.0).round() as i64)
            .unwrap_or(0),
            storage_class: value_string(item, &["spec", "storageClassName"]).map(|s| s.to_string()),
        });
    }
    Ok(claims)
}

pub fn parse_kubectl_services_json(input: &str) -> Result<Vec<K8sServiceSnapshot>, String> {
    let root: Value =
        serde_json::from_str(input).map_err(|e| format!("invalid kubectl services json: {}", e))?;
    let items = root
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "kubectl services json missing items array".to_string())?;
    let mut services = Vec::new();
    for item in items {
        let selector_count = item
            .get("spec")
            .and_then(|v| v.get("selector"))
            .and_then(|v| v.as_object())
            .map(|v| v.len())
            .unwrap_or(0);
        let load_balancer_ingress_count = item
            .get("status")
            .and_then(|v| v.get("loadBalancer"))
            .and_then(|v| v.get("ingress"))
            .and_then(|v| v.as_array())
            .map(|v| v.len())
            .unwrap_or(0);
        services.push(K8sServiceSnapshot {
            namespace: metadata_namespace(item),
            name: metadata_name(item),
            service_type: value_string(item, &["spec", "type"])
                .unwrap_or("ClusterIP")
                .to_string(),
            selector_count,
            load_balancer_ingress_count,
        });
    }
    Ok(services)
}

pub fn parse_kubectl_workloads_json(input: &str) -> Result<Vec<K8sWorkloadSnapshot>, String> {
    let root: Value = serde_json::from_str(input)
        .map_err(|e| format!("invalid kubectl workloads json: {}", e))?;
    let items = root
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "kubectl workloads json missing items array".to_string())?;
    let mut workloads = Vec::new();
    for item in items {
        let mut cpu = 0;
        let mut memory = 0;
        if let Some(containers) = item
            .get("spec")
            .and_then(|v| v.get("template"))
            .and_then(|v| v.get("spec"))
            .and_then(|v| v.get("containers"))
            .and_then(|v| v.as_array())
        {
            for container in containers {
                if let Some(raw) = value_string(container, &["resources", "requests", "cpu"]) {
                    cpu += parse_cpu_to_millicores(raw);
                }
                if let Some(raw) = value_string(container, &["resources", "requests", "memory"]) {
                    memory += parse_memory_to_mib(raw);
                }
            }
        }
        workloads.push(K8sWorkloadSnapshot {
            namespace: metadata_namespace(item),
            name: metadata_name(item),
            kind: value_string(item, &["kind"])
                .unwrap_or("Workload")
                .to_string(),
            replicas: item
                .get("spec")
                .and_then(|v| v.get("replicas"))
                .and_then(|v| v.as_i64())
                .unwrap_or(1),
            ready_replicas: item
                .get("status")
                .and_then(|v| v.get("readyReplicas"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            requested_cpu_millicores: cpu,
            requested_memory_mib: memory,
        });
    }
    Ok(workloads)
}

pub fn parse_kubectl_nodes_json(
    input: &str,
    pods: &[K8sPodSnapshot],
) -> Result<Vec<K8sNodeSnapshot>, String> {
    let root: Value =
        serde_json::from_str(input).map_err(|e| format!("invalid kubectl nodes json: {}", e))?;
    let items = root
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "kubectl nodes json missing items array".to_string())?;
    let mut pod_requests: HashMap<String, (i64, i64)> = HashMap::new();
    for pod in pods {
        if !pod.phase.eq_ignore_ascii_case("running") {
            continue;
        }
        if let Some(node_name) = pod.node_name.as_deref() {
            let entry = pod_requests.entry(node_name.to_string()).or_insert((0, 0));
            entry.0 += pod.requested_cpu_millicores;
            entry.1 += pod.requested_memory_mib;
        }
    }

    let mut nodes = Vec::new();
    for item in items {
        let name = metadata_name(item);
        let requests = pod_requests.get(&name).copied().unwrap_or((0, 0));
        let instance_type = metadata_label(item, "node.kubernetes.io/instance-type")
            .or_else(|| metadata_label(item, "beta.kubernetes.io/instance-type"));
        nodes.push(K8sNodeSnapshot {
            name,
            provider_id: value_string(item, &["spec", "providerID"]).map(|s| s.to_string()),
            instance_type,
            allocatable_cpu_millicores: value_string(item, &["status", "allocatable", "cpu"])
                .map(parse_cpu_to_millicores)
                .unwrap_or(0),
            allocatable_memory_mib: value_string(item, &["status", "allocatable", "memory"])
                .map(parse_memory_to_mib)
                .unwrap_or(0),
            requested_cpu_millicores: requests.0,
            requested_memory_mib: requests.1,
        });
    }
    Ok(nodes)
}

pub fn parse_kubectl_persistent_volumes_json(
    input: &str,
) -> Result<Vec<K8sPersistentVolumeSnapshot>, String> {
    let root: Value =
        serde_json::from_str(input).map_err(|e| format!("invalid kubectl pv json: {}", e))?;
    let items = root
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "kubectl pv json missing items array".to_string())?;
    let mut volumes = Vec::new();
    for item in items {
        volumes.push(K8sPersistentVolumeSnapshot {
            name: metadata_name(item),
            capacity_gib: value_string(item, &["spec", "capacity", "storage"])
                .or_else(|| value_string(item, &["status", "capacity", "storage"]))
                .map(parse_memory_to_mib)
                .map(|mib| (mib as f64 / 1024.0).round() as i64)
                .unwrap_or(0),
            phase: value_string(item, &["status", "phase"])
                .unwrap_or("Unknown")
                .to_string(),
            claim_namespace: value_string(item, &["spec", "claimRef", "namespace"])
                .map(|s| s.to_string()),
            claim_name: value_string(item, &["spec", "claimRef", "name"]).map(|s| s.to_string()),
            storage_class: value_string(item, &["spec", "storageClassName"]).map(|s| s.to_string()),
            volume_handle: value_string(item, &["spec", "csi", "volumeHandle"])
                .or_else(|| value_string(item, &["spec", "awsElasticBlockStore", "volumeID"]))
                .or_else(|| value_string(item, &["spec", "gcePersistentDisk", "pdName"]))
                .or_else(|| value_string(item, &["spec", "azureDisk", "diskName"]))
                .map(|s| s.to_string()),
        });
    }
    Ok(volumes)
}

pub fn analyze_k8s_snapshot(
    cluster_name: &str,
    nodes: &[K8sNodeSnapshot],
    volumes: &[K8sPersistentVolumeSnapshot],
) -> Vec<WastedResource> {
    let mut findings = analyze_k8s_nodes(cluster_name, nodes);
    findings.extend(analyze_k8s_persistent_volumes(cluster_name, volumes));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kubeconfig_summary_extracts_contexts() {
        let raw = r#"
apiVersion: v1
kind: Config
current-context: prod-eks
contexts:
  - name: prod-eks
    context:
      cluster: arn:aws:eks:ap-southeast-1:111122223333:cluster/prod
      user: arn:aws:eks:ap-southeast-1:111122223333:cluster/prod
      namespace: finance
  - name: stage-gke
    context:
      cluster: gke_project_asia_stage
      user: gke_user_stage
"#;
        let summary = parse_kubeconfig_summary(raw).expect("parse should succeed");
        assert_eq!(summary.current_context.as_deref(), Some("prod-eks"));
        assert_eq!(summary.contexts.len(), 2);
        assert_eq!(summary.contexts[0].name, "prod-eks");
        assert_eq!(summary.contexts[0].namespace.as_deref(), Some("finance"));
        assert_eq!(summary.contexts[1].name, "stage-gke");
        assert_eq!(summary.contexts[1].namespace, None);
    }

    #[test]
    fn resolve_active_context_prefers_requested_then_current_then_first() {
        let summary = KubeconfigSummary {
            current_context: Some("prod".to_string()),
            contexts: vec![
                KubeconfigContextRef {
                    name: "dev".to_string(),
                    cluster: "c1".to_string(),
                    user: "u1".to_string(),
                    namespace: None,
                },
                KubeconfigContextRef {
                    name: "prod".to_string(),
                    cluster: "c2".to_string(),
                    user: "u2".to_string(),
                    namespace: Some("ops".to_string()),
                },
            ],
        };
        let preferred = resolve_active_context(&summary, Some("dev"))
            .expect("preferred context should resolve");
        assert_eq!(preferred.name, "dev");

        let fallback_current = resolve_active_context(&summary, Some("not-found"))
            .expect("current context should resolve");
        assert_eq!(fallback_current.name, "prod");

        let no_current = KubeconfigSummary {
            current_context: None,
            contexts: summary.contexts.clone(),
        };
        let fallback_first =
            resolve_active_context(&no_current, None).expect("first context should resolve");
        assert_eq!(fallback_first.name, "dev");
    }

    #[test]
    fn assess_kubeconfig_readiness_reports_provider_and_warnings() {
        let summary = KubeconfigSummary {
            current_context: Some("missing".to_string()),
            contexts: vec![KubeconfigContextRef {
                name: "prod-eks".to_string(),
                cluster: "arn:aws:eks:us-east-1:123456789012:cluster/prod".to_string(),
                user: "aws".to_string(),
                namespace: None,
            }],
        };

        let readiness = assess_kubeconfig_readiness(&summary, Some("prod-eks"));
        assert_eq!(readiness.status, "ready");
        assert_eq!(readiness.provider_hint, "eks");
        assert_eq!(readiness.active_context_source, "preferred");
        assert!(readiness
            .warnings
            .iter()
            .any(|warning| warning.contains("current-context 'missing'")));

        let blocked = assess_kubeconfig_readiness(
            &KubeconfigSummary {
                current_context: None,
                contexts: Vec::new(),
            },
            None,
        );
        assert_eq!(blocked.status, "blocked");
        assert_eq!(blocked.active_context_source, "none");
    }

    #[test]
    fn analyze_k8s_nodes_flags_large_low_request_nodes() {
        let findings = analyze_k8s_nodes(
            "prod",
            &[K8sNodeSnapshot {
                name: "pool-a-1".to_string(),
                provider_id: Some("aws:///i-123".to_string()),
                instance_type: Some("m6i.2xlarge".to_string()),
                allocatable_cpu_millicores: 7_500,
                allocatable_memory_mib: 30 * 1024,
                requested_cpu_millicores: 900,
                requested_memory_mib: 4 * 1024,
            }],
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].resource_type, "K8s Node Rightsizing");
        assert_eq!(findings[0].action_type, "RIGHTSIZE");
    }

    #[test]
    fn analyze_k8s_persistent_volumes_flags_released_or_available_without_claim() {
        let findings = analyze_k8s_persistent_volumes(
            "prod",
            &[
                K8sPersistentVolumeSnapshot {
                    name: "pv-released".to_string(),
                    capacity_gib: 100,
                    phase: "Released".to_string(),
                    claim_namespace: Some("app".to_string()),
                    claim_name: Some("data".to_string()),
                    storage_class: Some("gp3".to_string()),
                    volume_handle: Some("vol-123".to_string()),
                },
                K8sPersistentVolumeSnapshot {
                    name: "pv-bound".to_string(),
                    capacity_gib: 100,
                    phase: "Bound".to_string(),
                    claim_namespace: Some("app".to_string()),
                    claim_name: Some("live".to_string()),
                    storage_class: Some("gp3".to_string()),
                    volume_handle: Some("vol-456".to_string()),
                },
            ],
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].id, "prod/pv/pv-released");
        assert_eq!(findings[0].action_type, "DELETE");
    }

    #[test]
    fn parse_kubectl_json_builds_node_and_pv_snapshots() {
        let pods = parse_kubectl_pods_json(
            r#"{
  "items": [
    {
      "metadata": {"name": "api-1", "namespace": "prod"},
      "spec": {
        "nodeName": "node-a",
        "containers": [
          {"resources": {"requests": {"cpu": "500m", "memory": "512Mi"}}},
          {"resources": {"requests": {"cpu": "1", "memory": "1Gi"}}}
        ]
      },
      "status": {"phase": "Running"}
    }
  ]
}"#,
        )
        .expect("pods parse");
        assert_eq!(pods[0].requested_cpu_millicores, 1500);
        assert_eq!(pods[0].requested_memory_mib, 1536);

        let nodes = parse_kubectl_nodes_json(
            r#"{
  "items": [
    {
      "metadata": {
        "name": "node-a",
        "labels": {"node.kubernetes.io/instance-type": "m6i.2xlarge"}
      },
      "spec": {"providerID": "aws:///us-east-1a/i-123"},
      "status": {"allocatable": {"cpu": "7500m", "memory": "30Gi"}}
    }
  ]
}"#,
            &pods,
        )
        .expect("nodes parse");
        assert_eq!(nodes[0].requested_cpu_millicores, 1500);
        assert_eq!(nodes[0].allocatable_memory_mib, 30 * 1024);

        let volumes = parse_kubectl_persistent_volumes_json(
            r#"{
  "items": [
    {
      "metadata": {"name": "pv-old"},
      "spec": {
        "capacity": {"storage": "100Gi"},
        "claimRef": {"namespace": "prod", "name": "data"},
        "storageClassName": "gp3",
        "csi": {"volumeHandle": "vol-123"}
      },
      "status": {"phase": "Released"}
    }
  ]
}"#,
        )
        .expect("pv parse");
        assert_eq!(volumes[0].capacity_gib, 100);
        assert_eq!(volumes[0].volume_handle.as_deref(), Some("vol-123"));
    }

    #[test]
    fn analyze_k8s_snapshot_combines_node_and_volume_findings() {
        let findings = analyze_k8s_snapshot(
            "prod",
            &[K8sNodeSnapshot {
                name: "node-a".to_string(),
                provider_id: None,
                instance_type: Some("m6i.2xlarge".to_string()),
                allocatable_cpu_millicores: 7_500,
                allocatable_memory_mib: 30 * 1024,
                requested_cpu_millicores: 500,
                requested_memory_mib: 1024,
            }],
            &[K8sPersistentVolumeSnapshot {
                name: "pv-released".to_string(),
                capacity_gib: 50,
                phase: "Released".to_string(),
                claim_namespace: Some("prod".to_string()),
                claim_name: Some("data".to_string()),
                storage_class: Some("gp3".to_string()),
                volume_handle: Some("vol-123".to_string()),
            }],
        );
        assert_eq!(findings.len(), 2);
        assert!(findings
            .iter()
            .any(|finding| finding.resource_type == "K8s Node Rightsizing"));
        assert!(findings
            .iter()
            .any(|finding| finding.resource_type == "K8s Orphan PV"));
    }

    #[test]
    fn parse_kubectl_json_builds_pvc_service_and_workload_snapshots() {
        let pvcs = parse_kubectl_pvcs_json(
            r#"{"items":[{"metadata":{"namespace":"prod","name":"data"},"spec":{"volumeName":"pv-1","storageClassName":"gp3","resources":{"requests":{"storage":"25Gi"}}},"status":{"phase":"Bound"}}]}"#,
        )
        .expect("pvc parse");
        assert_eq!(pvcs[0].requested_storage_gib, 25);
        assert_eq!(pvcs[0].volume_name.as_deref(), Some("pv-1"));

        let services = parse_kubectl_services_json(
            r#"{"items":[{"metadata":{"namespace":"prod","name":"api"},"spec":{"type":"LoadBalancer","selector":{"app":"api"}},"status":{"loadBalancer":{"ingress":[{"hostname":"lb.example.com"}]}}}]}"#,
        )
        .expect("services parse");
        assert_eq!(services[0].service_type, "LoadBalancer");
        assert_eq!(services[0].selector_count, 1);
        assert_eq!(services[0].load_balancer_ingress_count, 1);

        let workloads = parse_kubectl_workloads_json(
            r#"{"items":[{"kind":"Deployment","metadata":{"namespace":"prod","name":"api"},"spec":{"replicas":3,"template":{"spec":{"containers":[{"resources":{"requests":{"cpu":"250m","memory":"256Mi"}}}]}}},"status":{"readyReplicas":2}}]}"#,
        )
        .expect("workloads parse");
        assert_eq!(workloads[0].kind, "Deployment");
        assert_eq!(workloads[0].replicas, 3);
        assert_eq!(workloads[0].requested_cpu_millicores, 250);
    }
}
