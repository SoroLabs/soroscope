//! Optional startup notifications for Slack and Discord incoming webhooks.
//!
//! Set `NOTIFY_WEBHOOK_URL` to enable. Delivery failures are logged but never
//! prevent the server from starting.

use chrono::Utc;
use reqwest::{Client, Url};
use serde::Serialize;
use std::{env, time::Duration};
use thiserror::Error;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WebhookFormat {
    Slack,
    Discord,
}

impl WebhookFormat {
    fn detect(url: &Url) -> Result<Self, StartupNotificationError> {
        let host = url
            .host_str()
            .unwrap_or_default()
            .trim_end_matches('.')
            .to_ascii_lowercase();

        if host == "hooks.slack.com" || host.ends_with(".slack.com") {
            return Ok(Self::Slack);
        }
        if host == "discord.com"
            || host.ends_with(".discord.com")
            || host == "discordapp.com"
            || host.ends_with(".discordapp.com")
        {
            return Ok(Self::Discord);
        }

        Err(StartupNotificationError::UnsupportedHost(host))
    }
}

#[derive(Debug, Error)]
pub enum StartupNotificationError {
    #[error("invalid NOTIFY_WEBHOOK_URL: {0}")]
    InvalidUrl(String),
    #[error("unsupported notification webhook host: {0}")]
    UnsupportedHost(String),
    #[error("startup notification request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("startup notification endpoint returned HTTP {0}")]
    HttpStatus(reqwest::StatusCode),
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct StartupEvent {
    event: &'static str,
    commit: String,
    environment: String,
    timestamp: String,
}

#[derive(Debug, Serialize)]
struct SlackPayload<'a> {
    text: String,
    #[serde(flatten)]
    event: &'a StartupEvent,
}

#[derive(Debug, Serialize)]
struct DiscordPayload<'a> {
    content: String,
    #[serde(flatten)]
    event: &'a StartupEvent,
}

/// Sends one startup event if `NOTIFY_WEBHOOK_URL` is configured.
///
/// Missing configuration is a no-op. Network errors and non-success HTTP
/// responses are returned to the caller so it can log them without failing
/// server startup.
pub async fn notify_server_started() -> Result<(), StartupNotificationError> {
    let raw_url = match env::var("NOTIFY_WEBHOOK_URL") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => {
            tracing::debug!("NOTIFY_WEBHOOK_URL is unset; startup notification disabled");
            return Ok(());
        }
    };

    let url = Url::parse(raw_url.trim())
        .map_err(|error| StartupNotificationError::InvalidUrl(error.to_string()))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(StartupNotificationError::InvalidUrl(
            "only http and https URLs are supported".to_string(),
        ));
    }
    let format = WebhookFormat::detect(&url)?;

    let event = StartupEvent {
        event: "server_started",
        commit: commit_sha(),
        environment: env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
        timestamp: Utc::now().to_rfc3339(),
    };

    let client = Client::builder().timeout(REQUEST_TIMEOUT).build()?;
    let response = match format {
        WebhookFormat::Slack => {
            let message = format!(
                "SoroScope server started\nCommit: {}\nEnvironment: {}\nTimestamp: {}",
                event.commit, event.environment, event.timestamp
            );
            client
                .post(url)
                .json(&SlackPayload {
                    text: message,
                    event: &event,
                })
                .send()
                .await?
        }
        WebhookFormat::Discord => {
            let message = format!(
                "SoroScope server started\nCommit: {}\nEnvironment: {}\nTimestamp: {}",
                event.commit, event.environment, event.timestamp
            );
            client
                .post(url)
                .json(&DiscordPayload {
                    content: message,
                    event: &event,
                })
                .send()
                .await?
        }
    };

    if !response.status().is_success() {
        return Err(StartupNotificationError::HttpStatus(response.status()));
    }

    tracing::info!(
        event = "server_started",
        commit = %event.commit,
        environment = %event.environment,
        "Startup notification delivered"
    );
    Ok(())
}

fn commit_sha() -> String {
    env::var("GIT_SHA")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| option_env!("GIT_SHA").map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn detects_slack_webhook_urls() {
        let url = Url::parse("https://hooks.slack.com/services/T/B/secret").unwrap();
        assert_eq!(WebhookFormat::detect(&url).unwrap(), WebhookFormat::Slack);
    }

    #[test]
    fn detects_discord_webhook_urls() {
        let url = Url::parse("https://discord.com/api/webhooks/123/token").unwrap();
        assert_eq!(WebhookFormat::detect(&url).unwrap(), WebhookFormat::Discord);

        let legacy = Url::parse("https://discordapp.com/api/webhooks/123/token").unwrap();
        assert_eq!(WebhookFormat::detect(&legacy).unwrap(), WebhookFormat::Discord);
    }

    #[test]
    fn rejects_unknown_webhook_hosts() {
        let url = Url::parse("https://example.com/webhook").unwrap();
        assert!(matches!(
            WebhookFormat::detect(&url),
            Err(StartupNotificationError::UnsupportedHost(_))
        ));
    }

    #[test]
    fn slack_payload_contains_text_and_startup_metadata() {
        let event = StartupEvent {
            event: "server_started",
            commit: "abc123".to_string(),
            environment: "production".to_string(),
            timestamp: "2026-10-22T12:00:00+00:00".to_string(),
        };
        let payload = serde_json::to_value(SlackPayload {
            text: "server started".to_string(),
            event: &event,
        })
        .unwrap();

        assert_eq!(payload["text"], "server started");
        assert_eq!(payload["event"], "server_started");
        assert_eq!(payload["commit"], "abc123");
        assert_eq!(payload["environment"], "production");
        assert_eq!(payload["timestamp"], "2026-10-22T12:00:00+00:00");
    }

    #[test]
    fn discord_payload_contains_content_and_startup_metadata() {
        let event = StartupEvent {
            event: "server_started",
            commit: "abc123".to_string(),
            environment: "production".to_string(),
            timestamp: "2026-10-22T12:00:00+00:00".to_string(),
        };
        let payload = serde_json::to_value(DiscordPayload {
            content: "server started".to_string(),
            event: &event,
        })
        .unwrap();

        assert_eq!(payload["content"], "server started");
        assert_eq!(payload["event"], "server_started");
        assert_eq!(payload["commit"], "abc123");
        assert_eq!(payload["environment"], "production");
        assert_eq!(payload["timestamp"], "2026-10-22T12:00:00+00:00");
    }

    #[test]
    fn event_name_is_stable() {
        let event = StartupEvent {
            event: "server_started",
            commit: "unknown".to_string(),
            environment: "development".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };
        let value: Value = serde_json::to_value(event).unwrap();
        assert_eq!(value["event"], "server_started");
    }
}
