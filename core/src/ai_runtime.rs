use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::WastedResource;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiDeviceSnapshot {
    pub device_id: String,
    pub name: String,
    pub utilization_gpu_pct: f64,
    pub memory_used_mib: i64,
    pub memory_total_mib: i64,
    pub power_draw_w: f64,
    pub temperature_c: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiRecommendation {
    pub id: String,
    pub title: String,
    pub rationale: String,
    pub expected_monthly_savings: f64,
    pub confidence: f64,
}

fn parse_f64(value: &str) -> f64 {
    value.trim().parse::<f64>().unwrap_or(0.0)
}

fn parse_i64(value: &str) -> i64 {
    value.trim().parse::<i64>().unwrap_or(0)
}

pub fn parse_nvidia_smi_csv(input: &str) -> Vec<AiDeviceSnapshot> {
    input
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let cols = line.split(',').map(|c| c.trim()).collect::<Vec<_>>();
            if cols.len() < 7 {
                return None;
            }
            Some(AiDeviceSnapshot {
                device_id: cols[0].to_string(),
                name: cols[1].to_string(),
                utilization_gpu_pct: parse_f64(cols[2]),
                memory_used_mib: parse_i64(cols[3]),
                memory_total_mib: parse_i64(cols[4]),
                power_draw_w: parse_f64(cols[5]),
                temperature_c: parse_f64(cols[6]),
            })
        })
        .collect()
}

pub fn parse_rocm_smi_json(input: &str) -> Vec<AiDeviceSnapshot> {
    let parsed: Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut devices = Vec::new();
    let Some(obj) = parsed.as_object() else {
        return devices;
    };
    for (key, value) in obj {
        let card = value.as_object();
        let name = card
            .and_then(|m| m.get("Card series"))
            .and_then(|v| v.as_str())
            .unwrap_or("AMD GPU")
            .to_string();
        let util = card
            .and_then(|m| m.get("GPU use (%)"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim_end_matches('%'))
            .map(parse_f64)
            .unwrap_or(0.0);
        let mem_used = card
            .and_then(|m| m.get("GPU memory use (%)"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim_end_matches('%'))
            .map(parse_f64)
            .unwrap_or(0.0);
        let temp = card
            .and_then(|m| m.get("Temperature (Sensor edge) (C)"))
            .and_then(|v| v.as_str())
            .map(parse_f64)
            .unwrap_or(0.0);
        let power = card
            .and_then(|m| m.get("Average Graphics Package Power (W)"))
            .and_then(|v| v.as_str())
            .map(parse_f64)
            .unwrap_or(0.0);

        let memory_total_mib = 16 * 1024;
        let memory_used_mib = ((mem_used / 100.0) * memory_total_mib as f64).round() as i64;
        devices.push(AiDeviceSnapshot {
            device_id: key.to_string(),
            name,
            utilization_gpu_pct: util,
            memory_used_mib,
            memory_total_mib,
            power_draw_w: power,
            temperature_c: temp,
        });
    }
    devices
}

fn estimate_gpu_monthly_cost(name: &str) -> f64 {
    let n = name.to_ascii_lowercase();
    if n.contains("h100") {
        2500.0
    } else if n.contains("a100") {
        1800.0
    } else if n.contains("a10") || n.contains("a30") {
        900.0
    } else if n.contains("t4") || n.contains("l4") {
        320.0
    } else {
        600.0
    }
}

pub fn analyze_ai_devices(devices: &[AiDeviceSnapshot]) -> Vec<WastedResource> {
    let mut findings = Vec::new();
    for d in devices {
        let mem_ratio = if d.memory_total_mib <= 0 {
            0.0
        } else {
            (d.memory_used_mib as f64 / d.memory_total_mib as f64).clamp(0.0, 1.0)
        };
        let monthly_cost = estimate_gpu_monthly_cost(&d.name);

        if d.utilization_gpu_pct < 20.0 {
            findings.push(WastedResource {
                id: format!("ai/{}/idle", d.device_id),
                provider: "ai-runtime".to_string(),
                region: "local".to_string(),
                resource_type: "AI Idle GPU".to_string(),
                details: format!(
                    "GPU '{}' ({}) utilization is {:.1}%, below 20% threshold.",
                    d.device_id, d.name, d.utilization_gpu_pct
                ),
                estimated_monthly_cost: monthly_cost * 0.35,
                action_type: "RIGHTSIZE".to_string(),
            });
        }

        if d.utilization_gpu_pct < 30.0 && mem_ratio > 0.75 {
            findings.push(WastedResource {
                id: format!("ai/{}/memory-stranded", d.device_id),
                provider: "ai-runtime".to_string(),
                region: "local".to_string(),
                resource_type: "AI Stranded GPU Memory".to_string(),
                details: format!(
                    "GPU '{}' ({}) memory usage is {:.0}% while compute utilization is only {:.1}%.",
                    d.device_id,
                    d.name,
                    mem_ratio * 100.0,
                    d.utilization_gpu_pct
                ),
                estimated_monthly_cost: monthly_cost * 0.25,
                action_type: "RIGHTSIZE".to_string(),
            });
        }

        if d.power_draw_w > 220.0 && d.utilization_gpu_pct < 35.0 {
            findings.push(WastedResource {
                id: format!("ai/{}/power-inefficient", d.device_id),
                provider: "ai-runtime".to_string(),
                region: "local".to_string(),
                resource_type: "AI Power Inefficiency".to_string(),
                details: format!(
                    "GPU '{}' ({}) draws {:.1}W at only {:.1}% utilization.",
                    d.device_id, d.name, d.power_draw_w, d.utilization_gpu_pct
                ),
                estimated_monthly_cost: monthly_cost * 0.18,
                action_type: "RIGHTSIZE".to_string(),
            });
        }
    }
    findings
}

pub fn build_ai_recommendations(
    devices: &[AiDeviceSnapshot],
    findings: &[WastedResource],
) -> Vec<AiRecommendation> {
    if devices.is_empty() {
        return vec![AiRecommendation {
            id: "ai/no-devices".to_string(),
            title: "No AI device detected".to_string(),
            rationale:
                "No GPU telemetry was detected from local probe. Validate nvidia-smi availability."
                    .to_string(),
            expected_monthly_savings: 0.0,
            confidence: 0.95,
        }];
    }

    let savings = findings
        .iter()
        .map(|f| f.estimated_monthly_cost)
        .sum::<f64>();
    let mut recs = Vec::new();
    let idle_count = findings
        .iter()
        .filter(|f| f.resource_type == "AI Idle GPU")
        .count();
    if idle_count > 0 {
        recs.push(AiRecommendation {
            id: "ai/reclaim-idle-gpu".to_string(),
            title: "Reclaim idle GPU windows".to_string(),
            rationale:
                "Idle GPU findings indicate underutilized devices. Shift batch jobs or stop idle nodes off-hours."
                    .to_string(),
            expected_monthly_savings: savings * 0.45,
            confidence: 0.82,
        });
    }
    recs.push(AiRecommendation {
        id: "ai/rightsize-inference".to_string(),
        title: "Right-size inference or training shape".to_string(),
        rationale:
            "Low compute utilization with high memory/power suggests workload and GPU class mismatch."
                .to_string(),
        expected_monthly_savings: savings * 0.35,
        confidence: 0.76,
    });
    recs.push(AiRecommendation {
        id: "ai/power-thermal-guardrail".to_string(),
        title: "Add power/thermal guardrail for low-throughput jobs".to_string(),
        rationale:
            "Power-inefficient runs should be scheduled or constrained to avoid waste in low-throughput windows."
                .to_string(),
        expected_monthly_savings: savings * 0.20,
        confidence: 0.71,
    });
    recs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_csv_and_analyze_findings() {
        let raw =
            "0, NVIDIA A100, 12, 64000, 80000, 245, 62\n1, NVIDIA T4, 74, 5000, 16000, 68, 49";
        let devices = parse_nvidia_smi_csv(raw);
        assert_eq!(devices.len(), 2);
        let findings = analyze_ai_devices(&devices);
        assert!(!findings.is_empty());
        assert!(findings
            .iter()
            .any(|f| f.resource_type.contains("AI Idle GPU")));
    }

    #[test]
    fn recommendations_for_empty_devices() {
        let recs = build_ai_recommendations(&[], &[]);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].id, "ai/no-devices");
    }

    #[test]
    fn parse_rocm_json_builds_devices() {
        let raw = r#"{
          "card0": {
            "Card series": "AMD Instinct MI250",
            "GPU use (%)": "12%",
            "GPU memory use (%)": "80%",
            "Temperature (Sensor edge) (C)": "64.0",
            "Average Graphics Package Power (W)": "238.0"
          }
        }"#;
        let devices = parse_rocm_smi_json(raw);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "AMD Instinct MI250");
        assert!((devices[0].utilization_gpu_pct - 12.0).abs() < 0.01);
    }
}
