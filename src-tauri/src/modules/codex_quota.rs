use crate::models::codex::{CodexAccount, CodexQuota, CodexQuotaErrorInfo};
use crate::modules::{codex_account, codex_wakeup, logger};
use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::time::{timeout, Duration};

const COCKPIT_API_PROVIDER_ID: &str = "cockpit_api";
const LEGACY_NEW_API_PROVIDER_ID: &str = "new_api";
const COCKPIT_API_PLAN_TYPE: &str = "Cockpit Api";
const LEGACY_NEW_API_EXCLUSIVE_PLAN_TYPE: &str = "NEW_API_EXCLUSIVE";
const COCKPIT_API_BASE_URL: &str = "https://chongcodex.cn/v1";
const APP_SERVER_TIMEOUT: Duration = Duration::from_secs(20);
const APP_SERVER_REFRESH_TOKEN_REQUESTED: &str = "Codex app-server 请求刷新 ChatGPT token";

fn extract_error_code_from_message(message: &str) -> Option<String> {
    let marker = "[error_code:";
    if let Some(start) = message.find(marker) {
        let code_start = start + marker.len();
        let end = message[code_start..].find(']')?;
        return Some(message[code_start..code_start + end].to_string());
    }

    let marker = "error_code=";
    let start = message.find(marker)?;
    let code_start = start + marker.len();
    let tail = &message[code_start..];
    let end = tail
        .find(|ch: char| ch == ',' || ch == ']' || ch.is_whitespace())
        .unwrap_or(tail.len());
    let code = tail[..end].trim();
    if code.is_empty() {
        None
    } else {
        Some(code.to_string())
    }
}

fn write_quota_error(account: &mut CodexAccount, message: String) {
    account.quota_error = Some(CodexQuotaErrorInfo {
        code: extract_error_code_from_message(&message),
        message,
        timestamp: chrono::Utc::now().timestamp(),
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppServerWindow {
    #[serde(rename = "usedPercent")]
    used_percent: Option<f64>,
    #[serde(rename = "windowDurationMins")]
    window_duration_mins: Option<i64>,
    #[serde(rename = "resetsAt")]
    resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppServerRateLimit {
    #[serde(rename = "limitId")]
    limit_id: Option<String>,
    #[serde(rename = "limitName")]
    limit_name: Option<String>,
    primary: Option<AppServerWindow>,
    secondary: Option<AppServerWindow>,
    #[serde(rename = "rateLimitReachedType")]
    rate_limit_reached_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AppServerRateLimitsResult {
    #[serde(rename = "rateLimits")]
    rate_limits: Option<AppServerRateLimit>,
    #[serde(rename = "rateLimitsByLimitId")]
    rate_limits_by_limit_id: Option<HashMap<String, AppServerRateLimit>>,
}

#[derive(Debug, Deserialize)]
struct AppServerError {
    code: Option<i64>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AppServerMessage {
    id: Option<i64>,
    method: Option<String>,
    result: Option<Value>,
    error: Option<AppServerError>,
}

fn normalize_app_server_remaining_percentage(window: &AppServerWindow) -> i32 {
    let used = window.used_percent.unwrap_or(0.0).clamp(0.0, 100.0);
    (100.0 - used).round().clamp(0.0, 100.0) as i32
}

fn normalize_app_server_window_minutes(window: &AppServerWindow) -> Option<i64> {
    window.window_duration_mins.filter(|value| *value > 0)
}

fn normalize_app_server_reset_time(window: &AppServerWindow) -> Option<i64> {
    window.resets_at.filter(|value| *value > 0)
}

struct FetchQuotaResult {
    quota: CodexQuota,
    plan_type: Option<String>,
}

async fn refresh_account_tokens(account: &mut CodexAccount, reason: &str) -> Result<(), String> {
    logger::log_info(&format!(
        "Codex 账号 {} 触发强制 Token 刷新: {}",
        account.email, reason
    ));

    let refreshed = codex_account::force_refresh_managed_account(&account.id, reason)
        .await
        .map_err(|e| format!("{}，刷新 Token 失败: {}", reason, e))?;
    *account = refreshed;
    Ok(())
}

async fn fetch_app_server_quota(account: &CodexAccount) -> Result<FetchQuotaResult, String> {
    let account_id = account.account_id.clone().or_else(|| {
        codex_account::extract_chatgpt_account_id_from_access_token(&account.tokens.access_token)
    });
    let account_id = account_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Codex access_token 缺少 ChatGPT 账号 ID".to_string())?;

    logger::log_info(&format!(
        "Codex 配额请求: codex app-server account/rateLimits/read (account_id: {})",
        account_id
    ));

    let raw_result = fetch_app_server_rate_limits(account, &account_id).await?;
    let quota = parse_quota_from_app_server_result(raw_result)?;

    Ok(FetchQuotaResult {
        quota,
        plan_type: None,
    })
}

async fn fetch_app_server_rate_limits(
    account: &CodexAccount,
    account_id: &str,
) -> Result<Value, String> {
    let mut child = spawn_codex_app_server()?;
    let result = run_app_server_rate_limit_read(&mut child, account, account_id).await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    result
}

fn spawn_codex_app_server() -> Result<tokio::process::Child, String> {
    let runtime = codex_wakeup::resolve_cli_runtime()?;
    let mut command = if let Some(node_path) = runtime.node_path {
        let mut command = Command::new(node_path);
        command.arg(runtime.binary_path);
        command
    } else {
        Command::new(runtime.binary_path)
    };
    command
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command
        .spawn()
        .map_err(|e| format!("启动 Codex app-server 失败: {}", e))
}

async fn run_app_server_rate_limit_read(
    child: &mut tokio::process::Child,
    account: &CodexAccount,
    account_id: &str,
) -> Result<Value, String> {
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Codex app-server stdin 不可用".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Codex app-server stdout 不可用".to_string())?;
    let mut lines = BufReader::new(stdout).lines();

    send_app_server_message(
        &mut stdin,
        json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": {
                    "name": "cockpit_tools",
                    "title": "Cockpit Tools",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }
        }),
    )
    .await?;
    read_app_server_result(&mut lines, 1).await?;

    send_app_server_message(
        &mut stdin,
        json!({
            "method": "initialized",
            "params": {}
        }),
    )
    .await?;

    let mut login_params = serde_json::Map::new();
    login_params.insert("type".to_string(), json!("chatgptAuthTokens"));
    login_params.insert(
        "accessToken".to_string(),
        json!(account.tokens.access_token.clone()),
    );
    login_params.insert("chatgptAccountId".to_string(), json!(account_id));
    if let Some(plan_type) = account
        .plan_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        login_params.insert("chatgptPlanType".to_string(), json!(plan_type));
    }

    send_app_server_message(
        &mut stdin,
        json!({
            "method": "account/login/start",
            "id": 2,
            "params": Value::Object(login_params)
        }),
    )
    .await?;
    read_app_server_result(&mut lines, 2).await?;

    send_app_server_message(
        &mut stdin,
        json!({
            "method": "account/rateLimits/read",
            "id": 3
        }),
    )
    .await?;
    read_app_server_result(&mut lines, 3).await
}

async fn send_app_server_message(stdin: &mut ChildStdin, message: Value) -> Result<(), String> {
    let mut payload =
        serde_json::to_vec(&message).map_err(|e| format!("序列化 app-server 请求失败: {}", e))?;
    payload.push(b'\n');
    stdin
        .write_all(&payload)
        .await
        .map_err(|e| format!("写入 app-server 请求失败: {}", e))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("刷新 app-server 请求失败: {}", e))
}

async fn read_app_server_result(
    lines: &mut Lines<BufReader<ChildStdout>>,
    target_id: i64,
) -> Result<Value, String> {
    loop {
        let line = timeout(APP_SERVER_TIMEOUT, lines.next_line())
            .await
            .map_err(|_| "等待 Codex app-server 响应超时".to_string())?
            .map_err(|e| format!("读取 app-server 响应失败: {}", e))?
            .ok_or_else(|| "Codex app-server 已退出".to_string())?;
        let message: AppServerMessage =
            serde_json::from_str(&line).map_err(|e| format!("解析 app-server 响应失败: {}", e))?;

        if message.method.as_deref() == Some("account/chatgptAuthTokens/refresh") {
            return Err(APP_SERVER_REFRESH_TOKEN_REQUESTED.to_string());
        }

        if message.id != Some(target_id) {
            continue;
        }

        if let Some(error) = message.error {
            let mut message = error
                .message
                .unwrap_or_else(|| "Codex app-server 请求失败".to_string());
            if let Some(code) = error.code {
                message.push_str(&format!(" [error_code:{}]", code));
            }
            return Err(message);
        }

        return message
            .result
            .ok_or_else(|| "Codex app-server 响应缺少 result".to_string());
    }
}

fn parse_quota_from_app_server_result(raw_result: Value) -> Result<CodexQuota, String> {
    let result: AppServerRateLimitsResult = serde_json::from_value(raw_result)
        .map_err(|e| format!("解析 app-server 配额 JSON 失败: {}", e))?;
    let rate_limit = resolve_app_server_codex_rate_limit(&result)
        .ok_or_else(|| "app-server 配额响应缺少 codex rateLimits".to_string())?;
    let primary_window = rate_limit.primary.as_ref();
    let secondary_window = rate_limit.secondary.as_ref();

    let (hourly_percentage, hourly_reset_time, hourly_window_minutes) =
        if let Some(primary) = primary_window {
            (
                normalize_app_server_remaining_percentage(primary),
                normalize_app_server_reset_time(primary),
                normalize_app_server_window_minutes(primary),
            )
        } else {
            (100, None, None)
        };

    let (weekly_percentage, weekly_reset_time, weekly_window_minutes) =
        if let Some(secondary) = secondary_window {
            (
                normalize_app_server_remaining_percentage(secondary),
                normalize_app_server_reset_time(secondary),
                normalize_app_server_window_minutes(secondary),
            )
        } else {
            (100, None, None)
        };

    Ok(CodexQuota {
        hourly_percentage,
        hourly_reset_time,
        hourly_window_minutes,
        hourly_window_present: Some(primary_window.is_some()),
        weekly_percentage,
        weekly_reset_time,
        weekly_window_minutes,
        weekly_window_present: Some(secondary_window.is_some()),
        raw_data: Some(build_app_server_raw_data(rate_limit)),
    })
}

fn resolve_app_server_codex_rate_limit<'a>(
    result: &'a AppServerRateLimitsResult,
) -> Option<&'a AppServerRateLimit> {
    result
        .rate_limits_by_limit_id
        .as_ref()
        .and_then(|limits| limits.get("codex"))
        .or_else(|| {
            result
                .rate_limits
                .as_ref()
                .filter(|rate_limit| rate_limit.limit_id.as_deref() == Some("codex"))
        })
        .or(result.rate_limits.as_ref())
}

fn app_server_window_to_legacy_raw(window: Option<&AppServerWindow>) -> Value {
    match window {
        Some(window) => json!({
            "used_percent": window.used_percent,
            "limit_window_seconds": window.window_duration_mins.map(|value| value * 60),
            "reset_at": window.resets_at,
        }),
        None => Value::Null,
    }
}

fn build_app_server_raw_data(rate_limit: &AppServerRateLimit) -> Value {
    json!({
        "provider": "codex-app-server",
        "rate_limit": {
            "allowed": rate_limit.rate_limit_reached_type.is_none(),
            "limit_reached": rate_limit.rate_limit_reached_type.is_some(),
            "primary_window": app_server_window_to_legacy_raw(rate_limit.primary.as_ref()),
            "secondary_window": app_server_window_to_legacy_raw(rate_limit.secondary.as_ref()),
        },
        "rateLimits": rate_limit,
    })
}

fn is_new_api_account(account: &CodexAccount) -> bool {
    account
        .api_provider_id
        .as_deref()
        .map(|value| {
            let value = value.trim();
            value.eq_ignore_ascii_case(COCKPIT_API_PROVIDER_ID)
                || value.eq_ignore_ascii_case(LEGACY_NEW_API_PROVIDER_ID)
        })
        .unwrap_or(false)
        || is_cockpit_api_base_url(account.api_base_url.as_deref())
        || account
            .plan_type
            .as_deref()
            .map(|value| {
                let value = value.trim();
                value.eq_ignore_ascii_case(COCKPIT_API_PLAN_TYPE)
                    || value.eq_ignore_ascii_case(LEGACY_NEW_API_EXCLUSIVE_PLAN_TYPE)
            })
            .unwrap_or(false)
}

fn normalize_api_base_url_for_match(raw: Option<&str>) -> Option<String> {
    let parsed = reqwest::Url::parse(raw?.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?;
    let port = parsed
        .port()
        .map(|value| format!(":{}", value))
        .unwrap_or_default();
    let path = parsed.path().trim_end_matches('/');
    Some(format!("{}://{}{}{}", parsed.scheme(), host, port, path).to_ascii_lowercase())
}

fn is_cockpit_api_base_url(raw: Option<&str>) -> bool {
    let Some(actual) = normalize_api_base_url_for_match(raw) else {
        return false;
    };
    let Some(expected) = normalize_api_base_url_for_match(Some(COCKPIT_API_BASE_URL)) else {
        return false;
    };
    actual == expected
}

fn build_new_api_profile_url(account: &CodexAccount) -> Result<String, String> {
    let base_url = account
        .api_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("Cockpit Api 账号缺少 Base URL")?;
    let mut parsed = reqwest::Url::parse(base_url)
        .map_err(|err| format!("Cockpit Api Base URL 无效: {}", err))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Cockpit Api Base URL 仅支持 http/https".to_string());
    }
    parsed.set_path("/api/cockpit-tools/token-profile");
    parsed.set_query(None);
    parsed.set_fragment(None);
    Ok(parsed.to_string())
}

fn read_i64(value: &serde_json::Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(|item| {
            item.as_i64()
                .or_else(|| item.as_u64().and_then(|raw| i64::try_from(raw).ok()))
        })
        .unwrap_or(0)
}

fn read_bool(value: &serde_json::Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(|item| item.as_bool())
        .unwrap_or(false)
}

fn new_api_percentage(available: i64, total: i64, unlimited: bool) -> i32 {
    if unlimited {
        return 100;
    }
    if total <= 0 {
        return 0;
    }
    let percentage = (available.max(0) as f64 / total.max(1) as f64) * 100.0;
    percentage.round().clamp(0.0, 100.0) as i32
}

async fn fetch_new_api_quota(account: &CodexAccount) -> Result<FetchQuotaResult, String> {
    let api_key = account
        .openai_api_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or("Cockpit Api 账号缺少 OPENAI_API_KEY")?;
    let profile_url = build_new_api_profile_url(account)?;
    let client = reqwest::Client::new();
    let response = client
        .get(&profile_url)
        .bearer_auth(api_key)
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|err| format!("请求 Cockpit Api 额度失败: {}", err))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| format!("读取 Cockpit Api 额度响应失败: {}", err))?;
    if !status.is_success() {
        return Err(format!("Cockpit Api 额度接口返回 HTTP {}", status.as_u16()));
    }

    let root: serde_json::Value = serde_json::from_str(&body)
        .map_err(|err| format!("解析 Cockpit Api 额度 JSON 失败: {}", err))?;
    if root.get("success").and_then(|item| item.as_bool()) == Some(false) {
        let message = root
            .get("message")
            .and_then(|item| item.as_str())
            .unwrap_or("Cockpit Api 额度接口返回失败");
        return Err(message.to_string());
    }
    let data = root.get("data").unwrap_or(&root);
    let usage = data.get("usage").ok_or("Cockpit Api 额度响应缺少 usage")?;
    let total = read_i64(usage, "total_granted");
    let used = read_i64(usage, "total_used");
    let available = read_i64(usage, "total_available");
    let unlimited = read_bool(usage, "unlimited_quota");
    let percentage = new_api_percentage(available, total, unlimited);
    let expires_at = read_i64(usage, "expires_at");
    let reset_time = if expires_at > 0 {
        Some(expires_at)
    } else {
        None
    };

    Ok(FetchQuotaResult {
        quota: CodexQuota {
            hourly_percentage: percentage,
            hourly_reset_time: reset_time,
            hourly_window_minutes: None,
            hourly_window_present: Some(true),
            weekly_percentage: 0,
            weekly_reset_time: None,
            weekly_window_minutes: None,
            weekly_window_present: Some(false),
            raw_data: Some(json!({
                "provider": "cockpit-api",
                "object": "codex_cockpit_api_quota",
                "profile": data,
                "usage": usage,
                "total_granted": total,
                "total_used": used,
                "total_available": available,
                "unlimited_quota": unlimited
            })),
        },
        plan_type: Some(
            data.get("plan_type")
                .and_then(|item| item.as_str())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| COCKPIT_API_PLAN_TYPE.to_string()),
        ),
    })
}

/// 从 id_token 中提取订阅标识并同步更新账号和索引
fn sync_subscription_from_token(
    account: &mut CodexAccount,
    plan_type: Option<String>,
    subscription_active_until: Option<String>,
) {
    let mut changed = false;
    if let Some(ref new_plan) = plan_type {
        let old_plan = account.plan_type.clone();
        if account.plan_type.as_deref() != Some(new_plan) {
            logger::log_info(&format!(
                "Codex 账号 {} 订阅标识已更新: {:?} -> {:?}",
                account.email, old_plan, plan_type
            ));
            account.plan_type = plan_type;
            changed = true;
        }
    }

    if let Some(ref next_expiry) = subscription_active_until {
        if account.subscription_active_until.as_deref() != Some(next_expiry) {
            account.subscription_active_until = Some(next_expiry.clone());
            changed = true;
        }
    }

    if changed {
        if let Err(e) = codex_account::update_account_plan_type_in_index(
            &account.id,
            &account.plan_type,
            &account.subscription_active_until,
        ) {
            logger::log_warn(&format!("更新索引 plan_type 失败: {}", e));
        }
    }
}

fn sync_subscription_expiry_from_current_id_token(account: &mut CodexAccount) {
    if let Ok((_, _, _, subscription_active_until, _, _)) =
        codex_account::extract_user_info(&account.tokens.id_token)
    {
        sync_subscription_from_token(account, None, subscription_active_until);
    }
}

async fn fetch_app_server_quota_with_token_refresh(
    account: &mut CodexAccount,
) -> Result<FetchQuotaResult, String> {
    match fetch_app_server_quota(account).await {
        Ok(result) => Ok(result),
        Err(e) if e == APP_SERVER_REFRESH_TOKEN_REQUESTED => {
            refresh_account_tokens(account, APP_SERVER_REFRESH_TOKEN_REQUESTED).await?;
            sync_subscription_expiry_from_current_id_token(account);
            codex_account::save_account(account)?;
            fetch_app_server_quota(account).await
        }
        Err(e) => Err(e),
    }
}

/// 刷新账号配额并保存（包含 token 自动刷新）
async fn refresh_account_quota_once(account_id: &str) -> Result<CodexQuota, String> {
    let mut account = codex_account::prepare_account_for_injection(account_id).await?;
    if account.is_api_key_auth() {
        if is_new_api_account(&account) {
            let result = match fetch_new_api_quota(&account).await {
                Ok(result) => result,
                Err(e) => {
                    write_quota_error(&mut account, e.clone());
                    if let Err(save_err) = codex_account::save_account(&account) {
                        logger::log_warn(&format!("写入 Cockpit Api 配额错误失败: {}", save_err));
                    }
                    return Err(e);
                }
            };
            if let Some(plan_type) = result.plan_type {
                sync_subscription_from_token(&mut account, Some(plan_type), None);
            }
            account.quota = Some(result.quota.clone());
            account.quota_error = None;
            account.usage_updated_at = Some(chrono::Utc::now().timestamp());
            codex_account::save_account(&account)?;
            return Ok(result.quota);
        }
        account.quota = None;
        account.quota_error = None;
        account.usage_updated_at = None;
        let _ = codex_account::save_account(&account);
        return Err("API Key 账号不支持刷新配额，请在网页端查看。".to_string());
    }

    // 检查 token 是否过期，如果过期则刷新
    if crate::modules::codex_oauth::is_token_expired(&account.tokens.access_token) {
        match refresh_account_tokens(&mut account, "Token 已过期").await {
            Ok(()) => {
                logger::log_info(&format!("账号 {} 的 Token 刷新成功", account.email));

                sync_subscription_expiry_from_current_id_token(&mut account);

                codex_account::save_account(&account)?;
            }
            Err(e) => {
                logger::log_error(&format!("账号 {} Token 刷新失败: {}", account.email, e));
                let message = e;
                write_quota_error(&mut account, message.clone());
                if let Err(save_err) = codex_account::save_account(&account) {
                    logger::log_warn(&format!("写入 Codex 配额错误失败: {}", save_err));
                }
                return Err(message);
            }
        }
    }

    let result = match fetch_app_server_quota_with_token_refresh(&mut account).await {
        Ok(result) => result,
        Err(e) => {
            write_quota_error(&mut account, e.clone());
            if let Err(save_err) = codex_account::save_account(&account) {
                logger::log_warn(&format!("写入 Codex 配额错误失败: {}", save_err));
            }
            return Err(e);
        }
    };

    if let Some(plan_type) = result.plan_type {
        sync_subscription_from_token(&mut account, Some(plan_type), None);
    }

    account.quota = Some(result.quota.clone());
    account.quota_error = None;
    account.usage_updated_at = Some(chrono::Utc::now().timestamp());
    codex_account::save_account(&account)?;

    Ok(result.quota)
}

pub async fn refresh_account_quota(account_id: &str) -> Result<CodexQuota, String> {
    refresh_account_quota_once(account_id).await
}

/// 刷新所有账号配额
pub async fn refresh_all_quotas() -> Result<Vec<(String, Result<CodexQuota, String>)>, String> {
    use futures::future::join_all;
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    const MAX_CONCURRENT: usize = 5;
    let accounts: Vec<_> = codex_account::list_accounts()
        .into_iter()
        .filter(|account| !account.is_api_key_auth() || is_new_api_account(account))
        .collect();

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let tasks: Vec<_> = accounts
        .into_iter()
        .map(|account| {
            let account_id = account.id;
            let semaphore = semaphore.clone();
            async move {
                let _permit = semaphore
                    .acquire_owned()
                    .await
                    .map_err(|e| format!("获取 Codex 刷新并发许可失败: {}", e))?;
                let result = refresh_account_quota(&account_id).await;
                Ok::<(String, Result<CodexQuota, String>), String>((account_id, result))
            }
        })
        .collect();

    let mut results = Vec::with_capacity(tasks.len());
    for task in join_all(tasks).await {
        match task {
            Ok(item) => results.push(item),
            Err(err) => return Err(err),
        }
    }

    Ok(results)
}
