use serde_json::Value;

pub(crate) fn proxy_scheme_from_url(proxy_url: &str) -> Option<String> {
    let trimmed = proxy_url.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed
        .split_once("://")
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
}

pub(crate) fn extract_aws_region_hint(credentials: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(credentials).ok()?;
    for key in ["region", "aws_region", "region_id"] {
        if let Some(value) = parsed.get(key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub(crate) fn encode_proxy_userinfo(raw: &str) -> String {
    raw.bytes()
        .flat_map(|b| {
            let ch = b as char;
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | '~') {
                vec![ch]
            } else {
                format!("%{:02X}", b).chars().collect::<Vec<char>>()
            }
        })
        .collect()
}

pub(crate) fn compose_proxy_url_from_parts(
    protocol: &str,
    host: &str,
    port: i64,
    auth_username: Option<&str>,
    auth_password: Option<&str>,
) -> String {
    let wrapped_host = if host.contains(':') && !host.starts_with('[') && !host.ends_with(']') {
        format!("[{}]", host)
    } else {
        host.to_string()
    };
    let username = auth_username.unwrap_or("").trim();
    if username.is_empty() {
        return format!("{}://{}:{}", protocol, wrapped_host, port);
    }
    let password = auth_password.unwrap_or("").trim();
    let userinfo = if password.is_empty() {
        encode_proxy_userinfo(username)
    } else {
        format!(
            "{}:{}",
            encode_proxy_userinfo(username),
            encode_proxy_userinfo(password)
        )
    };
    format!("{}://{}@{}:{}", protocol, userinfo, wrapped_host, port)
}

pub(crate) fn normalize_proxy_mode(raw: &str) -> String {
    let mode = raw.trim().to_lowercase();
    match mode.as_str() {
        "custom" | "none" | "system" => mode,
        _ => "none".to_string(),
    }
}

pub(crate) fn normalize_account_proxy_choice(proxy_choice: Option<&str>) -> String {
    let trimmed = proxy_choice.unwrap_or_default().trim();
    if trimmed.is_empty() {
        super::PROXY_CHOICE_DIRECT.to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn mask_proxy_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "".to_string();
    }

    if let Some(scheme_idx) = trimmed.find("://") {
        let scheme = &trimmed[..scheme_idx];
        let rest = &trimmed[(scheme_idx + 3)..];
        let host_with_auth_stripped = rest
            .rsplit_once('@')
            .map(|(_, right)| right)
            .unwrap_or(rest);
        let host_port = host_with_auth_stripped
            .split('/')
            .next()
            .unwrap_or(host_with_auth_stripped)
            .trim();
        if host_port.is_empty() {
            return format!("{}://***", scheme);
        }
        return format!("{}://{}", scheme, host_port);
    }

    let host_port = trimmed.split('/').next().unwrap_or(trimmed).trim();
    if host_port.is_empty() {
        "***".to_string()
    } else {
        host_port.to_string()
    }
}

pub(crate) fn proxy_endpoint_display(proxy_mode: &str, proxy_url: &str) -> String {
    if proxy_mode == "custom" && !proxy_url.trim().is_empty() {
        mask_proxy_url(proxy_url)
    } else {
        "-".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_helpers_build_and_mask_expected_urls() {
        assert_eq!(
            proxy_scheme_from_url("socks5h://127.0.0.1:1080"),
            Some("socks5h".to_string())
        );
        assert_eq!(
            compose_proxy_url_from_parts(
                "http",
                "proxy.example.com",
                8080,
                Some("ops user"),
                Some("pa:ss")
            ),
            "http://ops%20user:pa%3Ass@proxy.example.com:8080"
        );
        assert_eq!(
            compose_proxy_url_from_parts("http", "2001:db8::1", 8080, None, None),
            "http://[2001:db8::1]:8080"
        );
        assert_eq!(
            mask_proxy_url("http://user:secret@proxy.example.com:8080/path"),
            "http://proxy.example.com:8080"
        );
        assert_eq!(proxy_endpoint_display("none", "http://proxy"), "-");
    }

    #[test]
    fn helper_edge_cases_cover_empty_invalid_and_encoding_paths() {
        assert_eq!(proxy_scheme_from_url(""), None);
        assert_eq!(proxy_scheme_from_url("missing-scheme"), None);
        assert_eq!(
            extract_aws_region_hint(r#"{"aws_region":"eu-west-1"}"#),
            Some("eu-west-1".to_string())
        );
        assert_eq!(extract_aws_region_hint("not-json"), None);
        assert_eq!(encode_proxy_userinfo("ops+user"), "ops%2Buser");
    }

    #[test]
    fn normalize_mode_choice_and_mask_without_scheme() {
        assert_eq!(normalize_proxy_mode(" custom "), "custom");
        assert_eq!(normalize_proxy_mode("bad-mode"), "none");
        assert_eq!(
            normalize_account_proxy_choice(None),
            super::super::PROXY_CHOICE_DIRECT.to_string()
        );
        assert_eq!(normalize_account_proxy_choice(Some("proxy-1")), "proxy-1");
        assert_eq!(
            mask_proxy_url("proxy.example.com:1080/path"),
            "proxy.example.com:1080"
        );
        assert_eq!(mask_proxy_url("  "), "");
        assert_eq!(
            proxy_endpoint_display("custom", "http://u:p@host:9000"),
            "http://host:9000"
        );
    }
}
