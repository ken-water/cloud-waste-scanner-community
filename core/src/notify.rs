use crate::models::NotificationChannel;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize)]
struct SlackMessage {
    text: String,
}

#[derive(Serialize)]
struct TeamsMessage {
    text: String,
}

#[derive(Serialize)]
struct DiscordMessage {
    content: String,
}

#[derive(Serialize, Deserialize)]
struct TelegramMessage {
    chat_id: String,
    text: String,
    parse_mode: String,
}

#[derive(Serialize)]
struct WhatsAppMessage {
    messaging_product: String,
    to: String,
    #[serde(rename = "type")]
    msg_type: String,
    text: WhatsAppText,
}

#[derive(Serialize)]
struct WhatsAppText {
    body: String,
}

#[derive(Serialize, Deserialize)]
struct ChannelConfig {
    url: Option<String>,
    token: Option<String>,
    chat_id: Option<String>,
    phone_id: Option<String>,
    to_phone: Option<String>,
}

fn require_non_empty(value: Option<String>, field_name: &str) -> Result<String> {
    let normalized = value.unwrap_or_default().trim().to_string();
    if normalized.is_empty() {
        return Err(anyhow!("Missing required field: {}", field_name));
    }
    Ok(normalized)
}

fn ensure_http_success(status: reqwest::StatusCode) -> Result<u16> {
    if status.is_success() {
        Ok(status.as_u16())
    } else {
        Err(anyhow!(
            "Notification request failed with HTTP {}",
            status.as_u16()
        ))
    }
}

fn normalize_proxy_url(raw: &str) -> String {
    raw.trim().to_string()
}

fn build_http_client(proxy_url: Option<&str>) -> Result<Client> {
    let mut builder = Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(20));

    if let Some(raw_proxy) = proxy_url {
        let normalized = normalize_proxy_url(raw_proxy);
        if !normalized.is_empty() {
            let proxy = reqwest::Proxy::all(&normalized)?;
            builder = builder.proxy(proxy);
        }
    } else if std::env::var("NO_PROXY")
        .ok()
        .map(|v| v.trim() == "*")
        .unwrap_or(false)
    {
        // Respect explicit "direct mode" when caller has set NO_PROXY=*
        builder = builder.no_proxy();
    }

    Ok(builder.build()?)
}

pub async fn send_notification(channel: &NotificationChannel, message: &str) -> Result<u16> {
    send_notification_with_proxy(channel, message, None).await
}

pub async fn send_notification_with_proxy(
    channel: &NotificationChannel,
    message: &str,
    proxy_url: Option<&str>,
) -> Result<u16> {
    let client = build_http_client(proxy_url)?;
    let config: ChannelConfig = serde_json::from_str(&channel.config)?;

    let status_code = match channel.method.as_str() {
        "slack" => {
            let url = require_non_empty(config.url, "url")?;
            let msg = SlackMessage {
                text: message.to_string(),
            };
            let response = client.post(&url).json(&msg).send().await?;
            ensure_http_success(response.status())?
        }
        "teams" => {
            let url = require_non_empty(config.url, "url")?;
            let msg = TeamsMessage {
                text: message.to_string(),
            };
            let response = client.post(&url).json(&msg).send().await?;
            ensure_http_success(response.status())?
        }
        "discord" => {
            let url = require_non_empty(config.url, "url")?;
            let msg = DiscordMessage {
                content: message.to_string(),
            };
            let response = client.post(&url).json(&msg).send().await?;
            ensure_http_success(response.status())?
        }
        "telegram" => {
            let token = require_non_empty(config.token, "token")?;
            let chat_id = require_non_empty(config.chat_id, "chat_id")?;
            let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
            let msg = TelegramMessage {
                chat_id,
                text: message.to_string(),
                parse_mode: "Markdown".to_string(),
            };
            let response = client.post(&url).json(&msg).send().await?;
            ensure_http_success(response.status())?
        }
        "whatsapp" => {
            let token = require_non_empty(config.token, "token")?;
            let phone_id = require_non_empty(config.phone_id, "phone_id")?;
            let to_phone = require_non_empty(config.to_phone, "to_phone")?;
            let url = format!("https://graph.facebook.com/v17.0/{}/messages", phone_id);
            let msg = WhatsAppMessage {
                messaging_product: "whatsapp".to_string(),
                to: to_phone,
                msg_type: "text".to_string(),
                text: WhatsAppText {
                    body: message.to_string(),
                },
            };
            let response = client
                .post(&url)
                .bearer_auth(token)
                .json(&msg)
                .send()
                .await?;
            ensure_http_success(response.status())?
        }
        "webhook" => {
            let url = require_non_empty(config.url, "url")?;
            let msg = serde_json::json!({ "message": message });
            let response = client.post(&url).json(&msg).send().await?;
            ensure_http_success(response.status())?
        }
        _ => {
            return Err(anyhow!(
                "Unsupported notification method: {}",
                channel.method
            ))
        }
    };

    Ok(status_code)
}

pub async fn send_slack_notification(
    webhook_url: &str,
    total_savings: f64,
    resource_count: usize,
    symbol: &str,
) -> Result<()> {
    let client = build_http_client(None)?;
    let msg = SlackMessage {
        text: format!("🚨 *Cloud Waste Found!* \n\nCloud Waste Scanner detected *{}* idle resources with a potential savings of *{}{:.2}/mo*.\n\nPlease open the app to review and cleanup.", resource_count, symbol, total_savings),
    };

    client.post(webhook_url).json(&msg).send().await?;

    Ok(())
}
