//! RSA signing key management.
//!
//! Resolves signing keys from existing PEM files or auto-generates a
//! self-signed RSA-2048 key pair with X.509 certificate on first run.
//! Keys persist to disk for reuse across restarts.

use std::path::Path;

use jsonwebtoken::EncodingKey;
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_RSA_SHA256};
use tracing::{info, warn};

use crate::config::RsaSource;

pub struct KeyPairResult {
    pub encoding_key: EncodingKey,
    pub public_cert_pem: String,
}

/// Load or generate the RSA key pair based on configuration.
pub fn resolve_key_pair(source: &RsaSource) -> anyhow::Result<KeyPairResult> {
    match source {
        RsaSource::Load {
            private_key,
            public_cert,
        } => {
            // F20: Check and fix key permissions on startup
            fix_key_permissions(private_key);
            load_key_pair(private_key, public_cert)
        }
        RsaSource::AutoGenerate { directory } => {
            let priv_path = directory.join("private.pem");
            let cert_path = directory.join("public.pem");

            if priv_path.exists() && cert_path.exists() {
                info!(
                    private_key = %priv_path.display(),
                    public_cert = %cert_path.display(),
                    "Loading existing RSA key pair from directory"
                );
                // F20: Check and fix permissions on existing keys
                fix_key_permissions(&priv_path);
                load_key_pair(&priv_path, &cert_path)
            } else {
                info!(
                    directory = %directory.display(),
                    "No existing keys found, generating new RSA-2048 key pair"
                );
                std::fs::create_dir_all(directory)?;
                generate_and_save_key_pair(&priv_path, &cert_path)
            }
        }
        RsaSource::Missing => {
            anyhow::bail!(
                "RSA key configuration missing. Provide either: \
                 (a) RSA_PRIVATE_KEY_PATH + RSA_PUBLIC_CERT_PATH, or \
                 (b) RSA_KEY_PATH (directory for auto-generated keys)"
            );
        }
    }
}

/// Load an existing RSA private key and X.509 certificate from disk.
fn load_key_pair(
    private_key_path: &Path,
    public_cert_path: &Path,
) -> anyhow::Result<KeyPairResult> {
    let private_key_pem = std::fs::read_to_string(private_key_path).map_err(|e| {
        anyhow::anyhow!(
            "failed to read private key '{}': {e}",
            private_key_path.display()
        )
    })?;

    let public_cert_pem = std::fs::read_to_string(public_cert_path).map_err(|e| {
        anyhow::anyhow!(
            "failed to read public certificate '{}': {e}",
            public_cert_path.display()
        )
    })?;

    let encoding_key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .map_err(|e| anyhow::anyhow!("invalid RSA private key PEM: {e}"))?;

    info!(
        private_key = %private_key_path.display(),
        public_cert = %public_cert_path.display(),
        "RSA key pair loaded from disk"
    );

    Ok(KeyPairResult {
        encoding_key,
        public_cert_pem,
    })
}

/// Generate a new RSA-2048 self-signed key pair and save to disk.
/// F3: Sets explicit 0600 permissions on the private key file.
fn generate_and_save_key_pair(
    private_key_path: &Path,
    public_cert_path: &Path,
) -> anyhow::Result<KeyPairResult> {
    // Generate RSA-2048 key pair using rcgen's pure-Rust crypto backend
    let key_pair = KeyPair::generate_for(&PKCS_RSA_SHA256)
        .map_err(|e| anyhow::anyhow!("failed to generate RSA key pair: {e}"))?;

    // Build self-signed certificate
    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "jwt-exchange");
    dn.push(DnType::OrganizationName, "JWT Exchange Service");
    params.distinguished_name = dn;

    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| anyhow::anyhow!("failed to create self-signed certificate: {e}"))?;

    // Serialize to PEM
    let private_key_pem = key_pair.serialize_pem();
    let cert_pem = cert.pem();

    // F3: Save private key with restrictive permissions (umask-independent)
    std::fs::write(private_key_path, private_key_pem.as_bytes()).map_err(|e| {
        anyhow::anyhow!(
            "failed to write private key '{}': {e}",
            private_key_path.display()
        )
    })?;
    set_file_permissions(private_key_path, 0o600)?;

    // Save public cert
    std::fs::write(public_cert_path, cert_pem.as_bytes()).map_err(|e| {
        anyhow::anyhow!(
            "failed to write public certificate '{}': {e}",
            public_cert_path.display()
        )
    })?;

    let encoding_key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .map_err(|e| anyhow::anyhow!("failed to create encoding key: {e}"))?;

    info!(
        private_key = %private_key_path.display(),
        public_cert = %public_cert_path.display(),
        "Generated and saved new RSA-2048 self-signed key pair"
    );

    Ok(KeyPairResult {
        encoding_key,
        public_cert_pem: cert_pem,
    })
}

/// F20: Check key file permissions and fix if too permissive.
/// Only logs a warning — does not abort, since the fix is best-effort.
fn fix_key_permissions(path: &Path) {
    match check_file_permissions(path) {
        Ok(mode) if mode != 0o600 => {
            warn!(
                path = %path.display(),
                mode = format!("{mode:04o}"),
                "Private key has permissive mode, attempting to fix to 0600"
            );
            if let Err(e) = set_file_permissions(path, 0o600) {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "Failed to fix private key permissions"
                );
            } else {
                info!(
                    path = %path.display(),
                    "Private key permissions fixed to 0600"
                );
            }
        }
        Ok(_) => {
            // Permissions are correct (0600)
        }
        Err(e) => {
            warn!(
                path = %path.display(),
                error = %e,
                "Could not check private key permissions"
            );
        }
    }
}

/// Get the Unix file mode (permissions) for a file.
#[cfg(unix)]
fn check_file_permissions(path: &Path) -> std::io::Result<u32> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn check_file_permissions(_path: &Path) -> std::io::Result<u32> {
    // On non-Unix platforms, we cannot check/set file mode.
    // Return a sentinel value that triggers no action.
    Ok(0o600)
}

/// Set the Unix file mode for a file.
#[cfg(unix)]
fn set_file_permissions(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(mode);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn set_file_permissions(_path: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
}
