//! Speech-to-text providers. Every one speaks the OpenAI audio API: a
//! multipart `POST {base_url}/audio/transcriptions` answered with
//! `{"text": "..."}`. That covers Groq, OpenAI, Fireworks and self-hosted
//! Whisper servers (speaches, LocalAI, ...).

use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

pub const CUSTOM: &str = "custom";
pub const DEFAULT_CUSTOM_BASE_URL: &str = "http://localhost:8000/v1";

/// Fireworks serves each Whisper model from its own host:
/// https://docs.fireworks.ai/guides/querying-asr-models
const FIREWORKS_TURBO_BASE_URL: &str = "https://audio-turbo.us-virginia-1.direct.fireworks.ai/v1";
const FIREWORKS_V3_BASE_URL: &str = "https://audio-prod.us-virginia-1.direct.fireworks.ai/v1";

/// Long dictations take a while to upload and decode; past this the request
/// is abandoned.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// Whisper reads at most 224 prompt tokens; stay well under so providers
/// never reject the vocabulary hint.
const MAX_PROMPT_CHARS: usize = 600;

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Provider {
    pub id: &'static str,
    pub label: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
    /// Suggestions; any other model id the provider serves also works.
    pub models: &'static [&'static str],
    /// The custom provider may be a local server that needs no key.
    pub requires_api_key: bool,
    pub api_key_url: Option<&'static str>,
}

pub const PROVIDERS: &[Provider] = &[
    Provider {
        id: "groq",
        label: "Groq",
        base_url: "https://api.groq.com/openai/v1",
        default_model: "whisper-large-v3-turbo",
        models: &["whisper-large-v3-turbo", "whisper-large-v3"],
        requires_api_key: true,
        api_key_url: Some("https://console.groq.com/keys"),
    },
    Provider {
        id: "openai",
        label: "OpenAI",
        base_url: "https://api.openai.com/v1",
        default_model: "gpt-4o-mini-transcribe",
        models: &["gpt-4o-mini-transcribe", "gpt-4o-transcribe", "whisper-1"],
        requires_api_key: true,
        api_key_url: Some("https://platform.openai.com/api-keys"),
    },
    Provider {
        id: "fireworks",
        label: "Fireworks",
        base_url: FIREWORKS_TURBO_BASE_URL,
        default_model: "whisper-v3-turbo",
        models: &["whisper-v3-turbo", "whisper-v3"],
        requires_api_key: true,
        api_key_url: Some("https://app.fireworks.ai/settings/users/api-keys"),
    },
    Provider {
        id: CUSTOM,
        label: "Custom",
        base_url: DEFAULT_CUSTOM_BASE_URL,
        default_model: "whisper-1",
        models: &[],
        requires_api_key: false,
        api_key_url: None,
    },
];

pub fn find(id: &str) -> Option<&'static Provider> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// Why a transcription failed, in terms the interface can explain.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TranscribeError {
    UnknownProvider,
    MissingApiKey {
        provider: String,
    },
    MissingBaseUrl,
    /// 401/403: the key is wrong or lacks access.
    Unauthorized {
        provider: String,
    },
    /// 429: rate-limited or out of credit.
    RateLimited {
        provider: String,
    },
    Offline {
        provider: String,
    },
    Timeout {
        provider: String,
    },
    Rejected {
        provider: String,
        status: u16,
        message: String,
    },
    InvalidResponse {
        provider: String,
    },
}

impl fmt::Display for TranscribeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProvider => write!(f, "unknown provider"),
            Self::MissingApiKey { provider } => write!(f, "no API key for {provider}"),
            Self::MissingBaseUrl => write!(f, "no base URL for the custom provider"),
            Self::Unauthorized { provider } => write!(f, "{provider} refused the API key"),
            Self::RateLimited { provider } => write!(f, "{provider} rate limit reached"),
            Self::Offline { provider } => write!(f, "could not reach {provider}"),
            Self::Timeout { provider } => write!(f, "{provider} did not answer in time"),
            Self::Rejected {
                provider,
                status,
                message,
            } => write!(f, "{provider} answered {status}: {message}"),
            Self::InvalidResponse { provider } => {
                write!(f, "{provider} returned an unexpected response")
            }
        }
    }
}

/// One transcription request, resolved from the settings before any audio is
/// sent so a misconfiguration fails at once.
#[derive(Debug)]
pub struct Request {
    label: String,
    endpoint: String,
    api_key: String,
    model: String,
    language: Option<String>,
    prompt: Option<String>,
}

impl Request {
    pub fn from_settings(settings: &Settings) -> Result<Self, TranscribeError> {
        let provider = find(&settings.provider).ok_or(TranscribeError::UnknownProvider)?;
        let label = provider.label.to_string();

        let api_key = settings
            .api_keys
            .get(provider.id)
            .map(|key| key.trim().to_string())
            .unwrap_or_default();
        if provider.requires_api_key && api_key.is_empty() {
            return Err(TranscribeError::MissingApiKey { provider: label });
        }

        let model = settings
            .models
            .get(provider.id)
            .map(|model| model.trim())
            .filter(|model| !model.is_empty())
            .unwrap_or(provider.default_model)
            .to_string();

        let base_url = match (provider.id, model.as_str()) {
            (CUSTOM, _) => settings.custom_base_url.trim(),
            ("fireworks", "whisper-v3") => FIREWORKS_V3_BASE_URL,
            _ => provider.base_url,
        };
        if base_url.is_empty() {
            return Err(TranscribeError::MissingBaseUrl);
        }

        let language = match settings.language.trim() {
            "" | "auto" => None,
            language => Some(language.to_string()),
        };

        Ok(Self {
            label,
            endpoint: format!("{}/audio/transcriptions", base_url.trim_end_matches('/')),
            api_key,
            model,
            language,
            prompt: vocabulary_prompt(&settings.vocabulary),
        })
    }

    pub fn provider_label(&self) -> &str {
        &self.label
    }
}

/// Whisper takes a free-text prompt rather than a word list; naming the
/// vocabulary there biases it toward those spellings.
fn vocabulary_prompt(vocabulary: &str) -> Option<String> {
    let mut prompt = String::new();
    let words = vocabulary
        .split([',', '\n'])
        .map(str::trim)
        .filter(|w| !w.is_empty());
    for word in words {
        let separator = if prompt.is_empty() { "" } else { ", " };
        if prompt.len() + separator.len() + word.len() > MAX_PROMPT_CHARS {
            break;
        }
        prompt.push_str(separator);
        prompt.push_str(word);
    }
    (!prompt.is_empty()).then_some(prompt)
}

#[derive(Deserialize)]
struct TranscriptionResponse {
    text: String,
}

/// Sends a WAV file and returns the transcript.
pub async fn transcribe(request: &Request, wav: Vec<u8>) -> Result<String, TranscribeError> {
    let provider = request.label.clone();
    log::info!(
        "Transcribing {} KiB with {} ({}, language={:?}, vocabulary={})",
        wav.len() / 1024,
        request.label,
        request.model,
        request.language,
        request.prompt.is_some()
    );

    let file = reqwest::multipart::Part::bytes(wav)
        .file_name("dictation.wav")
        .mime_str("audio/wav")
        .expect("audio/wav is a valid mime type");
    let mut form = reqwest::multipart::Form::new()
        .part("file", file)
        .text("model", request.model.clone())
        .text("response_format", "json")
        .text("temperature", "0");
    if let Some(language) = &request.language {
        form = form.text("language", language.clone());
    }
    if let Some(prompt) = &request.prompt {
        form = form.text("prompt", prompt.clone());
    }

    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!(
            "Parler/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/Synapsr/Parler)"
        ))
        .build()
        .map_err(|_| TranscribeError::Offline {
            provider: provider.clone(),
        })?;
    let mut http = client.post(&request.endpoint).multipart(form);
    if !request.api_key.is_empty() {
        http = http.bearer_auth(&request.api_key);
    }

    let response = http.send().await.map_err(|e| {
        log::warn!("Request to {} failed: {e}", request.label);
        if e.is_timeout() {
            TranscribeError::Timeout {
                provider: provider.clone(),
            }
        } else {
            TranscribeError::Offline {
                provider: provider.clone(),
            }
        }
    })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        log::warn!("{} answered {status}: {body}", request.label);
        return Err(match status.as_u16() {
            401 | 403 => TranscribeError::Unauthorized { provider },
            429 => TranscribeError::RateLimited { provider },
            code => TranscribeError::Rejected {
                provider,
                status: code,
                message: api_error_message(&body),
            },
        });
    }

    let parsed: TranscriptionResponse = response
        .json()
        .await
        .map_err(|_| TranscribeError::InvalidResponse { provider })?;
    Ok(parsed.text.trim().to_string())
}

/// OpenAI-style errors carry `{"error": {"message": ...}}`; anything else is
/// shown as-is, cut short.
fn api_error_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|json| {
            json.pointer("/error/message")
                .and_then(|m| m.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| body.chars().take(300).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(provider: &str) -> Settings {
        Settings {
            provider: provider.to_string(),
            ..Settings::default()
        }
    }

    #[test]
    fn hosted_providers_need_a_key() {
        assert_eq!(
            Request::from_settings(&settings("groq")).unwrap_err(),
            TranscribeError::MissingApiKey {
                provider: "Groq".to_string()
            }
        );
    }

    #[test]
    fn custom_provider_works_without_a_key() {
        let request = Request::from_settings(&settings(CUSTOM)).unwrap();
        assert_eq!(
            request.endpoint,
            "http://localhost:8000/v1/audio/transcriptions"
        );
        assert_eq!(request.model, "whisper-1");
    }

    #[test]
    fn fireworks_routes_each_model_to_its_host() {
        let mut s = settings("fireworks");
        s.api_keys.insert("fireworks".into(), "fw_key".into());
        let turbo = Request::from_settings(&s).unwrap();
        assert!(turbo.endpoint.starts_with(FIREWORKS_TURBO_BASE_URL));

        s.models.insert("fireworks".into(), "whisper-v3".into());
        let v3 = Request::from_settings(&s).unwrap();
        assert!(v3.endpoint.starts_with(FIREWORKS_V3_BASE_URL));
    }

    #[test]
    fn auto_language_sends_no_hint() {
        let mut s = settings(CUSTOM);
        assert_eq!(Request::from_settings(&s).unwrap().language, None);
        s.language = "fr".into();
        assert_eq!(
            Request::from_settings(&s).unwrap().language.as_deref(),
            Some("fr")
        );
    }

    #[test]
    fn vocabulary_becomes_a_bounded_prompt() {
        assert_eq!(
            vocabulary_prompt("Synapsr, Tauri\n Groq ,"),
            Some("Synapsr, Tauri, Groq".to_string())
        );
        assert_eq!(vocabulary_prompt("  "), None);
        let long = "x".repeat(MAX_PROMPT_CHARS - 2);
        assert_eq!(vocabulary_prompt(&format!("{long}, overflow")), Some(long));
    }

    /// Serves one HTTP response on a local port and hands back the raw
    /// request it received.
    fn one_shot_server(status: &str, body: &str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut raw = Vec::new();
            let mut buf = [0u8; 8192];
            // Read the headers, then exactly Content-Length bytes of body.
            loop {
                let n = stream.read(&mut buf).unwrap();
                raw.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&raw).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text[..end]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if raw.len() >= end + 4 + length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&raw).to_string()
        });
        (base_url, server)
    }

    fn custom_settings(base_url: String) -> Settings {
        let mut s = settings(CUSTOM);
        s.custom_base_url = base_url;
        s.api_keys.insert(CUSTOM.into(), "sk-test".into());
        s.models.insert(CUSTOM.into(), "whisper-large-v3".into());
        s.language = "fr".into();
        s.vocabulary = "Parler, Synapsr".into();
        s
    }

    #[test]
    fn sends_an_openai_style_multipart_request() {
        let (base_url, server) = one_shot_server("200 OK", r#"{"text":" Bonjour !"}"#);
        let request = Request::from_settings(&custom_settings(base_url)).unwrap();
        let wav = crate::audio::encode_wav(&[0.0; 1600]);
        let text = tauri::async_runtime::block_on(transcribe(&request, wav)).unwrap();
        assert_eq!(text, "Bonjour !");

        let raw = server.join().unwrap();
        let lower = raw.to_ascii_lowercase();
        assert!(raw.starts_with("POST /v1/audio/transcriptions HTTP/1.1"));
        assert!(lower.contains("authorization: bearer sk-test"));
        assert!(lower.contains("content-type: multipart/form-data; boundary="));
        for (field, value) in [
            ("model", "whisper-large-v3"),
            ("language", "fr"),
            ("prompt", "Parler, Synapsr"),
            ("response_format", "json"),
        ] {
            assert!(
                raw.contains(&format!("name=\"{field}\"\r\n\r\n{value}\r\n")),
                "missing {field}"
            );
        }
        assert!(raw.contains("name=\"file\"; filename=\"dictation.wav\""));
        assert!(raw.contains("Content-Type: audio/wav"));
    }

    #[test]
    fn a_refused_key_is_reported_as_such() {
        let (base_url, server) = one_shot_server(
            "401 Unauthorized",
            r#"{"error":{"message":"Invalid API Key"}}"#,
        );
        let request = Request::from_settings(&custom_settings(base_url)).unwrap();
        let result = tauri::async_runtime::block_on(transcribe(&request, Vec::new()));
        server.join().unwrap();
        assert_eq!(
            result.unwrap_err(),
            TranscribeError::Unauthorized {
                provider: "Custom".to_string()
            }
        );
    }

    #[test]
    fn api_errors_surface_the_provider_message() {
        let body = r#"{"error":{"message":"model not found","type":"invalid_request_error"}}"#;
        assert_eq!(api_error_message(body), "model not found");
        assert_eq!(api_error_message("Bad Gateway"), "Bad Gateway");
    }
}
