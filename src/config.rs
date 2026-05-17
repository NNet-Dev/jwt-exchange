//! Application configuration and environment variable parsing.
//!
//! Defines `AppConfig` with defaults, env var parsing, and RSA key source
//! resolution (existing PEM files or auto-generated key pairs).

use std::collections::HashSet;
use std::path::PathBuf;

/// Application configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct AppConfig {
    // Inbound IdP
    pub inbound_issuer_uri: String,
    // Audience validation
    pub inbound_audience_validation_enabled: bool,
    pub inbound_expected_audience: Option<String>,
    // RSA key paths (explicit, optional)
    pub rsa_private_key_path: Option<PathBuf>,
    pub rsa_public_cert_path: Option<PathBuf>,
    // RSA key directory (auto-generate if keys missing, optional)
    pub rsa_key_path: Option<PathBuf>,
    // Qlik
    pub qlik_audience: String,
    pub qlik_user_directory: Option<String>,
    // Token TTL
    pub token_ttl_seconds: i64,
    // Server
    pub listen_host: String,
    pub listen_port: u16,
    // Database
    pub db_path: String,
    pub log_retention_days: i64,
    // Splunk HEC
    pub splunk_hec_url: Option<String>,
    pub splunk_hec_token: Option<String>,
    pub splunk_hec_skip_tls_verify: bool,
    // Groups whitelist
    pub groups_whitelist: HashSet<String>,
    // Input limits
    pub max_token_size: usize,
    pub max_groups_count: usize,
    pub max_group_name_length: usize,
    // Replay policy
    pub allow_replay: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        let inbound_issuer_uri = std::env::var("INBOUND_ISSUER_URI")
            .map_err(|_| ConfigError::Missing("INBOUND_ISSUER_URI"))?;

        // Audience validation: enabled by default
        let inbound_audience_validation_enabled = std::env::var("INBOUND_AUDIENCE_VALIDATION")
            .map(|v| {
                v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("1")
                    || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(true);

        let inbound_expected_audience = std::env::var("INBOUND_EXPECTED_AUDIENCE").ok();

        if inbound_audience_validation_enabled && inbound_expected_audience.is_none() {
            return Err(ConfigError::AudienceValidationConfigured(
                "INBOUND_AUDIENCE_VALIDATION is enabled (default) but INBOUND_EXPECTED_AUDIENCE is not set. \
                 Set INBOUND_EXPECTED_AUDIENCE to the expected audience value, or set INBOUND_AUDIENCE_VALIDATION=false to disable."
            ));
        }

        let rsa_private_key_path = std::env::var("RSA_PRIVATE_KEY_PATH")
            .ok()
            .map(PathBuf::from);
        let rsa_public_cert_path = std::env::var("RSA_PUBLIC_CERT_PATH")
            .ok()
            .map(PathBuf::from);
        let rsa_key_path = std::env::var("RSA_KEY_PATH").ok().map(PathBuf::from);

        let qlik_audience =
            std::env::var("QLIK_AUDIENCE").map_err(|_| ConfigError::Missing("QLIK_AUDIENCE"))?;
        let qlik_user_directory = std::env::var("QLIK_USER_DIRECTORY").ok();

        let token_ttl_seconds: i64 = std::env::var("TOKEN_TTL_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600);

        let listen_host = std::env::var("LISTEN_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let listen_port: u16 = std::env::var("LISTEN_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8080);

        let db_path = std::env::var("DB_PATH").unwrap_or_else(|_| "./jwt-exchange.db".to_string());
        let log_retention_days: i64 = std::env::var("LOG_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let splunk_hec_url = std::env::var("SPLUNK_HEC_URL").ok();
        let splunk_hec_token = std::env::var("SPLUNK_HEC_TOKEN").ok();

        // TLS skip: default false
        let splunk_hec_skip_tls_verify = std::env::var("SPLUNK_HEC_SKIP_TLS_VERIFY")
            .map(|v| {
                v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("1")
                    || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false);

        let groups_whitelist: HashSet<String> = std::env::var("GROUPS_WHITELIST")
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // Input limits
        let max_token_size: usize = std::env::var("MAX_TOKEN_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000); // 10KB max for a JWT

        let max_groups_count: usize = std::env::var("MAX_GROUPS_COUNT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);

        let max_group_name_length: usize = std::env::var("MAX_GROUP_NAME_LENGTH")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(256);

        // Replay policy: default false (strict replay protection)
        let allow_replay = std::env::var("ALLOW_REPLAY")
            .map(|v| {
                v.eq_ignore_ascii_case("true")
                    || v.eq_ignore_ascii_case("1")
                    || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false);

        Ok(Self {
            inbound_issuer_uri,
            inbound_audience_validation_enabled,
            inbound_expected_audience,
            rsa_private_key_path,
            rsa_public_cert_path,
            rsa_key_path,
            qlik_audience,
            qlik_user_directory,
            token_ttl_seconds,
            listen_host,
            listen_port,
            db_path,
            log_retention_days,
            splunk_hec_url,
            splunk_hec_token,
            splunk_hec_skip_tls_verify,
            groups_whitelist,
            max_token_size,
            max_groups_count,
            max_group_name_length,
            allow_replay,
        })
    }

    /// Determine how to obtain the RSA key pair.
    pub fn rsa_source(&self) -> RsaSource {
        match (
            &self.rsa_private_key_path,
            &self.rsa_public_cert_path,
            &self.rsa_key_path,
        ) {
            // Explicit paths provided — load from there
            (Some(pk), Some(cert), _) => RsaSource::Load {
                private_key: pk.clone(),
                public_cert: cert.clone(),
            },
            // Only key directory — auto-generate or load from dir
            (None, None, Some(dir)) => RsaSource::AutoGenerate {
                directory: dir.clone(),
            },
            // Mixed explicit + directory — explicit wins, dir is ignored
            (Some(pk), None, Some(dir)) => {
                let cert = dir.join("public.pem");
                RsaSource::Load {
                    private_key: pk.clone(),
                    public_cert: cert,
                }
            }
            (None, Some(cert), Some(dir)) => {
                let pk = dir.join("private.pem");
                RsaSource::Load {
                    private_key: pk,
                    public_cert: cert.clone(),
                }
            }
            // Nothing provided — error
            _ => RsaSource::Missing,
        }
    }
}

/// How the RSA key pair should be obtained at startup.
pub enum RsaSource {
    /// Load from explicit file paths.
    Load {
        private_key: PathBuf,
        public_cert: PathBuf,
    },
    /// Look in a directory; generate if files don't exist.
    AutoGenerate { directory: PathBuf },
    /// No RSA configuration found.
    Missing,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    Missing(&'static str),
    #[error("audience validation misconfiguration: {0}")]
    AudienceValidationConfigured(&'static str),
}
