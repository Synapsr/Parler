//! Parler Cloud: the paid plan that replaces an API key. A computer is linked
//! once with the device code flow (like `gh auth login`): Parler asks the
//! service for a code, opens the browser on it, and polls until the person
//! signs in and authorizes this computer. The device token it receives is
//! then used like any provider's API key.
//!
//! The service and the app share a contract: see `docs/architecture.md`.

use crate::providers::{self, TranscribeError};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

/// The provider id of Parler Cloud.
pub const ID: &str = "cloud";
pub const LABEL: &str = "Parler Cloud";
/// The service in production; `PARLER_CLOUD_URL` overrides it at startup,
/// for instance `http://localhost:3100` while working on the service.
pub const DEFAULT_URL: &str = "https://cloud.parler.synapsr.io";

/// The account calls are small JSON requests: past this, the service is
/// treated as unreachable rather than left hanging.
const TIMEOUT: Duration = Duration::from_secs(20);

static BASE_URL: OnceLock<String> = OnceLock::new();
static API_URL: OnceLock<String> = OnceLock::new();

/// The service's root URL, without a trailing slash.
pub fn base_url() -> &'static str {
    BASE_URL.get_or_init(|| {
        std::env::var("PARLER_CLOUD_URL")
            .ok()
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| DEFAULT_URL.to_string())
    })
}

/// The OpenAI-compatible API, as a provider base URL.
pub fn api_url() -> &'static str {
    API_URL.get_or_init(|| format!("{}/v1", base_url()))
}

/// Where the person picks or manages a plan, when the service hasn't said.
pub fn manage_url() -> String {
    format!("{}/account", base_url())
}

/// A sign-in in progress: the code to show and the page to open.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    /// Seconds before the code expires.
    pub expires_in: u64,
    /// Seconds to wait between polls.
    pub interval: u64,
    /// The name this computer was announced with, for the interface.
    #[serde(default)]
    pub device_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    None,
    Trialing,
    Active,
    PastDue,
    Canceled,
    /// A status this version of Parler doesn't know yet.
    #[serde(other)]
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub day_seconds: f64,
    pub month_seconds: f64,
    pub day_limit_seconds: f64,
    pub month_limit_seconds: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub email: String,
    pub status: SubscriptionStatus,
    /// "monthly" or "yearly".
    pub plan: Option<String>,
    pub trial_ends_at: Option<String>,
    pub renews_at: Option<String>,
    #[serde(default)]
    pub cancel_at_period_end: bool,
    pub usage: Usage,
    pub manage_url: String,
}

/// One poll of a sign-in, as the interface sees it.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum SignIn {
    Pending,
    Approved { account: Account },
    Denied,
    Expired,
}

/// One poll of a sign-in, with the token the interface never sees.
#[derive(Debug, PartialEq)]
pub enum Poll {
    Pending,
    Approved { token: String, account: Account },
    Denied,
    Expired,
}

fn offline(e: reqwest::Error) -> TranscribeError {
    log::warn!("Request to {LABEL} failed: {e}");
    let provider = LABEL.to_string();
    if e.is_timeout() {
        TranscribeError::Timeout { provider }
    } else {
        TranscribeError::Offline { provider }
    }
}

fn client() -> Result<reqwest::Client, TranscribeError> {
    providers::http_client(TIMEOUT).map_err(offline)
}

fn invalid() -> TranscribeError {
    TranscribeError::InvalidResponse {
        provider: LABEL.to_string(),
    }
}

/// An answer the flow doesn't expect, with the service's own words.
async fn rejected(response: reqwest::Response) -> TranscribeError {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    log::warn!("{LABEL} answered {status}: {body}");
    match status {
        429 => TranscribeError::RateLimited {
            provider: LABEL.to_string(),
        },
        _ => TranscribeError::Rejected {
            provider: LABEL.to_string(),
            status,
            message: providers::api_error_message(&body),
        },
    }
}

/// Asks the service for a code to link this computer with.
pub async fn start(
    base: &str,
    device_name: &str,
    platform: &str,
) -> Result<DeviceStart, TranscribeError> {
    let response = client()?
        .post(format!("{base}/api/device/start"))
        .json(&serde_json::json!({ "deviceName": device_name, "platform": platform }))
        .send()
        .await
        .map_err(offline)?;
    if !response.status().is_success() {
        return Err(rejected(response).await);
    }
    let mut start: DeviceStart = response.json().await.map_err(|_| invalid())?;
    start.device_name = device_name.to_string();
    // Polling faster than the service allows only earns refusals.
    start.interval = start.interval.max(1);
    Ok(start)
}

#[derive(Deserialize)]
struct PollAnswer {
    status: String,
    token: Option<String>,
    account: Option<Account>,
}

/// Asks once whether the person has authorized this computer yet.
pub async fn poll(base: &str, device_code: &str) -> Result<Poll, TranscribeError> {
    let response = client()?
        .post(format!("{base}/api/device/poll"))
        .json(&serde_json::json!({ "deviceCode": device_code }))
        .send()
        .await
        .map_err(offline)?;
    match response.status().as_u16() {
        200 => {}
        // The code is spent or gone: start again.
        404 | 410 => return Ok(Poll::Expired),
        // Polled a little early: the next poll will do.
        429 => return Ok(Poll::Pending),
        _ => return Err(rejected(response).await),
    }
    let answer: PollAnswer = response.json().await.map_err(|_| invalid())?;
    Ok(match answer.status.as_str() {
        "pending" => Poll::Pending,
        "denied" => Poll::Denied,
        "expired" => Poll::Expired,
        "approved" => match (answer.token, answer.account) {
            (Some(token), Some(account)) if !token.trim().is_empty() => Poll::Approved {
                token: token.trim().to_string(),
                account,
            },
            _ => return Err(invalid()),
        },
        _ => return Err(invalid()),
    })
}

/// The account behind a device token; `SignedOut` when the service no
/// longer knows the token.
pub async fn account(base: &str, token: &str) -> Result<Account, TranscribeError> {
    let response = client()?
        .get(format!("{base}/api/me"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(offline)?;
    if response.status().as_u16() == 401 {
        return Err(TranscribeError::SignedOut {
            provider: LABEL.to_string(),
        });
    }
    if !response.status().is_success() {
        return Err(rejected(response).await);
    }
    response.json().await.map_err(|_| invalid())
}

/// Tells the service to forget this computer's token.
pub async fn revoke(base: &str, token: &str) -> Result<(), TranscribeError> {
    let response = client()?
        .post(format!("{base}/api/device/revoke"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(offline)?;
    // Already unknown is as good as revoked.
    if response.status().is_success() || response.status().as_u16() == 401 {
        Ok(())
    } else {
        Err(rejected(response).await)
    }
}

/// The name the person gave this computer, shown on the authorization page
/// and in their list of devices.
pub fn device_name() -> String {
    // Tried in turn until one answers.
    let sources: &[fn() -> Option<String>] = &[
        // "Loan's MacBook Pro", rather than the network name.
        #[cfg(target_os = "macos")]
        || command_output("scutil", &["--get", "ComputerName"]),
        #[cfg(windows)]
        || std::env::var("COMPUTERNAME").ok(),
        #[cfg(target_os = "linux")]
        || std::fs::read_to_string("/etc/hostname").ok(),
        #[cfg(not(windows))]
        || command_output("hostname", &[]),
    ];
    sources
        .iter()
        .filter_map(|source| source())
        .map(|name| name.trim().to_string())
        .find(|name| !name.is_empty())
        .map(|name| name.chars().take(64).collect())
        .unwrap_or_else(|| "Computer".to_string())
}

#[cfg(not(windows))]
pub(crate) fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::tests::one_shot_server;

    /// The test server answers on `.../v1`; the account API lives at the root.
    fn serve(status: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
        let (url, server) = one_shot_server(status, body);
        (url.trim_end_matches("/v1").to_string(), server)
    }

    fn block<T>(future: impl std::future::Future<Output = T>) -> T {
        tauri::async_runtime::block_on(future)
    }

    const ACCOUNT: &str = r#"{"email":"name@example.com","status":"trialing","plan":"monthly",
        "trialEndsAt":"2026-10-27T10:00:00Z","renewsAt":null,"cancelAtPeriodEnd":false,
        "usage":{"daySeconds":600,"monthSeconds":43200,"dayLimitSeconds":14400,"monthLimitSeconds":144000},
        "manageUrl":"https://cloud.parler.synapsr.io/account"}"#;

    #[test]
    fn start_announces_this_computer() {
        let (base, server) = serve(
            "200 OK",
            r#"{"deviceCode":"dc_0123456789abcdef0123456789abcdef","userCode":"ABCD-EFGH",
                "verificationUrl":"https://cloud.parler.synapsr.io/link?code=ABCD-EFGH",
                "expiresIn":600,"interval":3}"#,
        );
        let start = block(start(&base, "Test Mac", "macos")).unwrap();
        assert_eq!(start.user_code, "ABCD-EFGH");
        assert_eq!(start.interval, 3);
        assert_eq!(start.expires_in, 600);
        assert_eq!(start.device_name, "Test Mac");

        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /api/device/start HTTP/1.1"));
        assert!(raw.contains(r#""deviceName":"Test Mac""#));
        assert!(raw.contains(r#""platform":"macos""#));
    }

    #[test]
    fn poll_waits_until_approved() {
        let (base, server) = serve("200 OK", r#"{"status":"pending"}"#);
        assert_eq!(block(poll(&base, "dc_1")).unwrap(), Poll::Pending);
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /api/device/poll HTTP/1.1"));
        assert!(raw.contains(r#""deviceCode":"dc_1""#));

        let body = format!(r#"{{"status":"approved","token":"pc_secret","account":{ACCOUNT}}}"#);
        let (base, server) = serve("200 OK", &body);
        let Poll::Approved { token, account } = block(poll(&base, "dc_1")).unwrap() else {
            panic!("expected an approval");
        };
        server.join().unwrap();
        assert_eq!(token, "pc_secret");
        assert_eq!(account.email, "name@example.com");
        assert_eq!(account.status, SubscriptionStatus::Trialing);
        assert_eq!(account.usage.month_seconds, 43200.0);
    }

    #[test]
    fn poll_reports_refusal_and_expiry() {
        for (status, body, expected) in [
            ("200 OK", r#"{"status":"denied"}"#, Poll::Denied),
            ("410 Gone", r#"{"status":"expired"}"#, Poll::Expired),
            ("429 Too Many Requests", "", Poll::Pending),
        ] {
            let (base, server) = serve(status, body);
            assert_eq!(block(poll(&base, "dc_1")).unwrap(), expected, "{status}");
            server.join().unwrap();
        }
    }

    #[test]
    fn an_approval_without_a_token_is_an_error() {
        let (base, server) = serve("200 OK", r#"{"status":"approved"}"#);
        let result = block(poll(&base, "dc_1"));
        server.join().unwrap();
        assert_eq!(
            result.unwrap_err(),
            TranscribeError::InvalidResponse {
                provider: LABEL.to_string()
            }
        );
    }

    #[test]
    fn account_sends_the_token() {
        let (base, server) = serve("200 OK", ACCOUNT);
        let account = block(account(&base, "pc_secret")).unwrap();
        assert_eq!(account.plan.as_deref(), Some("monthly"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("GET /api/me HTTP/1.1"));
        assert!(raw
            .to_ascii_lowercase()
            .contains("authorization: bearer pc_secret"));
    }

    #[test]
    fn an_unknown_token_means_signed_out() {
        let (base, server) = serve(
            "401 Unauthorized",
            r#"{"error":{"message":"Unknown token","type":"auth","code":"invalid_token"}}"#,
        );
        let result = block(account(&base, "pc_revoked"));
        server.join().unwrap();
        assert_eq!(
            result.unwrap_err(),
            TranscribeError::SignedOut {
                provider: LABEL.to_string()
            }
        );
    }

    #[test]
    fn revoke_forgets_the_token() {
        let (base, server) = serve("204 No Content", "");
        block(revoke(&base, "pc_secret")).unwrap();
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /api/device/revoke HTTP/1.1"));
        assert!(raw
            .to_ascii_lowercase()
            .contains("authorization: bearer pc_secret"));
    }

    #[test]
    fn statuses_follow_the_contract() {
        let status: SubscriptionStatus = serde_json::from_str(r#""past_due""#).unwrap();
        assert_eq!(status, SubscriptionStatus::PastDue);
        let unknown: SubscriptionStatus = serde_json::from_str(r#""paused""#).unwrap();
        assert_eq!(unknown, SubscriptionStatus::Unknown);
        assert_eq!(
            serde_json::to_string(&SubscriptionStatus::PastDue).unwrap(),
            r#""past_due""#
        );
    }

    #[test]
    fn this_computer_has_a_name() {
        let name = device_name();
        assert!(!name.trim().is_empty());
        assert!(name.chars().count() <= 64);
    }

    /// The whole journey against a running Parler Cloud: link this "computer",
    /// dictate once, read the account, sign out. Someone approves the printed
    /// code on `/link` meanwhile (or the local dev server's database does).
    /// `PARLER_CLOUD_URL=http://localhost:3100 cargo test --no-default-features e2e -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn e2e_against_a_running_service() {
        let base = std::env::var("PARLER_CLOUD_URL").expect("PARLER_CLOUD_URL");
        tauri::async_runtime::block_on(async {
            let started = start(&base, "Parler e2e", "macos").await.unwrap();
            println!(
                "APPROVE {} at {}",
                started.user_code, started.verification_url
            );

            let mut token = None;
            for _ in 0..60 {
                std::thread::sleep(std::time::Duration::from_secs(started.interval));
                match poll(&base, &started.device_code).await.unwrap() {
                    Poll::Pending => continue,
                    Poll::Approved { token: t, account } => {
                        println!("approved for {} ({:?})", account.email, account.status);
                        token = Some(t);
                        break;
                    }
                    other => panic!("sign-in ended with {other:?}"),
                }
            }
            let token = token.expect("nobody approved the code in time");
            assert_eq!(
                poll(&base, &started.device_code).await.unwrap(),
                Poll::Expired
            );

            let settings = crate::settings::Settings {
                provider: ID.to_string(),
                api_keys: [(ID.to_string(), token.clone())].into(),
                ..Default::default()
            };
            let request = crate::providers::Request::from_settings(&settings).unwrap();
            let tone: Vec<f32> = (0..32_000).map(|i| (i as f32 / 20.0).sin() * 0.2).collect();
            let text = crate::providers::transcribe(&request, crate::audio::encode_wav(&tone))
                .await
                .unwrap();
            println!("transcribed: {text}");
            assert!(!text.is_empty());

            let me = account(&base, &token).await.unwrap();
            println!("usage today: {} s", me.usage.day_seconds);
            assert!(me.usage.day_seconds >= 2.0);

            revoke(&base, &token).await.unwrap();
            assert!(account(&base, &token).await.is_err());
            let after =
                crate::providers::transcribe(&request, crate::audio::encode_wav(&tone)).await;
            println!("after sign-out: {after:?}");
            assert!(after.is_err());
        });
    }
}
