//! Self-update logic for the aitrack binary.
//!
//! Flow:
//!   1. Detect current platform to build the expected asset filename.
//!   2. Query the GitHub Releases API for the latest release.
//!   3. Find the matching binary asset and its companion `.sig` file.
//!   4. Download both files.
//!   5. Verify the ed25519 signature over the binary bytes.
//!   6. On success, atomically replace the current executable.

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use std::fs;

pub const GITHUB_RELEASES_API: &str =
    "https://api.github.com/repos/MapleEve/company-aitrack/releases/latest";

/// ed25519 public key used to verify release binary signatures.
/// Corresponds to the RELEASE_SIGNING_KEY secret in the GitHub repository.
/// Generated 2026-05-21; do not rotate — see CONTRIBUTING.md § 签名密钥管理.
pub const ED25519_PUBKEY_BASE64: &str = "qOpEg8nQAxAst7Cd2styaUXYUyhwr1l048FfCqCqvvI=";

// ---------------------------------------------------------------------------
// GitHub Releases API response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

// ---------------------------------------------------------------------------
// Platform detection
// ---------------------------------------------------------------------------

/// Build the asset filename stem for the current platform.
/// Format: `aitrack-{arch}-{os}` on Unix, `aitrack-{arch}-{os}.exe` on Windows.
pub fn platform_target_string() -> String {
    let arch = std::env::consts::ARCH;
    let os = match std::env::consts::OS {
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-musl",
        "windows" => "pc-windows-msvc",
        other => other,
    };
    if std::env::consts::OS == "windows" {
        format!("aitrack-{arch}-{os}.exe")
    } else {
        format!("aitrack-{arch}-{os}")
    }
}

// ---------------------------------------------------------------------------
// Signature verification
// ---------------------------------------------------------------------------

/// Decode and validate an ed25519 verifying key from a base64 string.
///
/// Returns `Err` if:
/// - the base64 is invalid,
/// - the decoded length is not exactly 32 bytes,
/// - the bytes form an all-zero key (placeholder — not a real key), or
/// - the bytes do not constitute a valid ed25519 point.
pub fn load_verifying_key(pubkey_b64: &str) -> Result<VerifyingKey> {
    let key_bytes = B64
        .decode(pubkey_b64)
        .context("failed to base64-decode public key")?;
    if key_bytes.len() != 32 {
        bail!("public key must be 32 bytes, got {}", key_bytes.len());
    }
    if key_bytes.iter().all(|&b| b == 0) {
        bail!("ed25519 public key is placeholder (all-zero); set real key before release");
    }
    let key_arr: [u8; 32] = key_bytes.try_into().unwrap();
    VerifyingKey::from_bytes(&key_arr).context("invalid ed25519 public key bytes")
}

/// Verify a raw 64-byte ed25519 `signature` over `message` using `pubkey_b64`.
fn verify_signature(pubkey_b64: &str, message: &[u8], signature_b64: &str) -> Result<()> {
    let verifying_key = load_verifying_key(pubkey_b64)?;

    let sig_bytes = B64
        .decode(signature_b64.trim())
        .context("failed to base64-decode signature")?;
    if sig_bytes.len() != 64 {
        bail!("signature must be 64 bytes, got {}", sig_bytes.len());
    }
    let sig_arr: [u8; 64] = sig_bytes.try_into().unwrap();
    let signature = Signature::from_bytes(&sig_arr);

    use ed25519_dalek::Verifier as _;
    verifying_key
        .verify(message, &signature)
        .context("ed25519 signature verification failed")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP helpers (blocking)
// ---------------------------------------------------------------------------

fn http_get_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("aitrack-updater/1")
        .build()
        .context("failed to build HTTP client")?;
    let resp = client
        .get(url)
        .send()
        .with_context(|| format!("GET {url} failed"))?;
    if !resp.status().is_success() {
        bail!("GET {url} returned HTTP {}", resp.status());
    }
    resp.bytes()
        .map(|b| b.to_vec())
        .context("failed to read response body")
}

fn http_get_text(url: &str) -> Result<String> {
    String::from_utf8(http_get_bytes(url)?).context("response body is not valid UTF-8")
}

// ---------------------------------------------------------------------------
// Atomic binary replacement
// ---------------------------------------------------------------------------

fn replace_current_exe(new_bytes: &[u8]) -> Result<()> {
    let current = std::env::current_exe().context("cannot determine current executable path")?;
    let tmp_path = current.with_extension("tmp");
    fs::write(&tmp_path, new_bytes)
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;

    // Make the new binary executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&tmp_path, perms)
            .context("failed to set executable permission on new binary")?;
    }

    fs::rename(&tmp_path, &current).with_context(|| {
        format!(
            "failed to replace {} with {}",
            current.display(),
            tmp_path.display()
        )
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Asset lookup helpers (testable without network)
// ---------------------------------------------------------------------------

/// Given a parsed `Release`, return `(binary_url, sig_url)` for `target`.
/// Returns `Err` if either asset is absent.
pub(crate) fn find_asset_urls<'a>(
    release: &'a Release,
    target: &str,
) -> Result<(&'a str, &'a str)> {
    let bin_asset = release
        .assets
        .iter()
        .find(|a| a.name == target)
        .with_context(|| format!("no asset named '{target}' in latest release"))?;

    let sig_name = format!("{target}.sig");
    let sig_asset = release
        .assets
        .iter()
        .find(|a| a.name == sig_name)
        .with_context(|| format!("no signature asset named '{sig_name}' in latest release"))?;

    Ok((
        bin_asset.browser_download_url.as_str(),
        sig_asset.browser_download_url.as_str(),
    ))
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run_update() -> Result<()> {
    let target = platform_target_string();
    println!("Checking for updates (platform: {target})...");

    // Fetch latest release metadata.
    let release_json =
        http_get_text(GITHUB_RELEASES_API).context("failed to reach GitHub Releases API")?;
    let release: Release =
        serde_json::from_str(&release_json).context("failed to parse GitHub release JSON")?;

    println!("Latest release: {}", release.tag_name);

    // Early-exit if already on the latest version.
    let current_version = env!("CARGO_PKG_VERSION");
    let tag = release.tag_name.trim_start_matches('v');
    if tag == current_version {
        println!("Already up to date (v{current_version}).");
        return Ok(());
    }

    let (bin_url, sig_url) = find_asset_urls(&release, &target)?;

    println!("Downloading {target}...");
    let bin_bytes = http_get_bytes(bin_url).context("failed to download binary")?;

    println!("Downloading {target}.sig...");
    let sig_text = http_get_text(sig_url).context("failed to download signature")?;

    // Verify before touching the filesystem.
    println!("Verifying ed25519 signature...");
    verify_signature(ED25519_PUBKEY_BASE64, &bin_bytes, sig_text.trim())
        .context("signature verification failed — update aborted")?;

    println!("Signature OK. Replacing binary...");
    replace_current_exe(&bin_bytes)?;

    println!("Update complete. You are now on {}.", release.tag_name);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke-test: platform detection must not panic and must return a non-empty string.
    #[test]
    fn test_platform_target_string() {
        let s = platform_target_string();
        assert!(
            !s.is_empty(),
            "platform_target_string() returned empty string"
        );
        assert!(
            s.starts_with("aitrack-"),
            "expected 'aitrack-' prefix, got: {s}"
        );
    }

    /// A tampered (all-zeros) signature must be rejected even against a valid keypair.
    #[test]
    fn test_sig_verification_rejects_bad_sig() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        // Generate a fresh real keypair for this test — do NOT use the placeholder key.
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey_b64 = B64.encode(verifying_key.as_bytes());

        let message = b"test payload";

        // A tampered signature: 64 zero bytes — definitely not valid.
        let bad_sig = B64.encode([0u8; 64]);

        let result = verify_signature(&pubkey_b64, message, &bad_sig);
        assert!(
            result.is_err(),
            "expected tampered signature to be rejected, but it was accepted"
        );
    }

    /// A correct signature produced by a matching key must be accepted.
    #[test]
    fn test_sig_verification_accepts_valid_sig() {
        use ed25519_dalek::{Signer, SigningKey};
        use rand::rngs::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey_b64 = B64.encode(verifying_key.as_bytes());

        let message = b"authentic payload";
        let signature: ed25519_dalek::Signature = signing_key.sign(message);
        let sig_b64 = B64.encode(signature.to_bytes());

        let result = verify_signature(&pubkey_b64, message, &sig_b64);
        assert!(
            result.is_ok(),
            "expected valid signature to be accepted: {result:?}"
        );
    }

    /// A signature valid for one message must be rejected for a different message.
    #[test]
    fn test_sig_verification_rejects_wrong_message() {
        use ed25519_dalek::{Signer, SigningKey};
        use rand::rngs::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey_b64 = B64.encode(verifying_key.as_bytes());

        let original = b"original message";
        let signature: ed25519_dalek::Signature = signing_key.sign(original);
        let sig_b64 = B64.encode(signature.to_bytes());

        let tampered = b"tampered message";
        let result = verify_signature(&pubkey_b64, tampered, &sig_b64);
        assert!(
            result.is_err(),
            "expected signature to be rejected for wrong message"
        );
    }

    // -----------------------------------------------------------------------
    // load_verifying_key: all-zero key must be rejected
    // -----------------------------------------------------------------------

    /// The constant `ED25519_PUBKEY_BASE64` shipped in the binary is intentionally
    /// all-zero (placeholder). `load_verifying_key` must refuse it so a binary
    /// built without a real signing key cannot silently accept any signature.
    #[test]
    fn test_load_verifying_key_rejects_zero_key() {
        let all_zeros_b64 = B64.encode([0u8; 32]);
        let result = load_verifying_key(&all_zeros_b64);
        assert!(
            result.is_err(),
            "expected all-zero key to be rejected, but load_verifying_key returned Ok"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("placeholder") || msg.contains("all-zero"),
            "error message should mention placeholder/all-zero, got: {msg}"
        );
    }

    /// The shipped `ED25519_PUBKEY_BASE64` constant must be a valid (non-zero) key.
    #[test]
    fn test_default_pubkey_constant_is_valid() {
        // The shipped public key must not be the all-zero placeholder.
        let result = load_verifying_key(ED25519_PUBKEY_BASE64);
        assert!(
            result.is_ok(),
            "ED25519_PUBKEY_BASE64 must be a valid key: {:?}",
            result.err()
        );
    }

    // -----------------------------------------------------------------------
    // load_verifying_key: wrong length rejected
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_verifying_key_rejects_short_key() {
        // Only 16 bytes — should be rejected for wrong length.
        let short_b64 = B64.encode([0xABu8; 16]);
        let result = load_verifying_key(&short_b64);
        assert!(result.is_err(), "short key (16 bytes) must be rejected");
    }

    #[test]
    fn test_load_verifying_key_rejects_invalid_base64() {
        let result = load_verifying_key("not-valid-base64!!!");
        assert!(result.is_err(), "invalid base64 must be rejected");
    }

    // -----------------------------------------------------------------------
    // verify_signature: wrong-length signature rejected
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_signature_rejects_short_sig() {
        use ed25519_dalek::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let pubkey_b64 = B64.encode(verifying_key.as_bytes());

        // Only 32 bytes for the sig instead of 64.
        let short_sig = B64.encode([0u8; 32]);
        let result = verify_signature(&pubkey_b64, b"message", &short_sig);
        assert!(result.is_err(), "short signature must be rejected");
    }

    // -----------------------------------------------------------------------
    // find_asset_urls: happy path and missing-asset error
    // -----------------------------------------------------------------------

    fn make_release(tag: &str, asset_names: &[&str]) -> Release {
        Release {
            tag_name: tag.to_string(),
            assets: asset_names
                .iter()
                .map(|name| Asset {
                    name: name.to_string(),
                    browser_download_url: format!("https://example.com/{name}"),
                })
                .collect(),
        }
    }

    #[test]
    fn test_find_asset_urls_happy_path() {
        let target = "aitrack-x86_64-unknown-linux-musl";
        let sig = format!("{target}.sig");
        let release = make_release("v1.2.3", &[target, sig.as_str(), "other-file"]);

        let result = find_asset_urls(&release, target);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        let (bin_url, sig_url) = result.unwrap();
        assert!(bin_url.contains(target));
        assert!(sig_url.contains(target));
        assert!(sig_url.ends_with(".sig"));
    }

    #[test]
    fn test_find_asset_urls_missing_binary() {
        let target = "aitrack-x86_64-unknown-linux-musl";
        // Release has only the .sig, not the binary.
        let sig = format!("{target}.sig");
        let release = make_release("v1.2.3", &[sig.as_str()]);

        let result = find_asset_urls(&release, target);
        assert!(result.is_err(), "missing binary asset must return Err");
    }

    #[test]
    fn test_find_asset_urls_missing_sig() {
        let target = "aitrack-x86_64-unknown-linux-musl";
        // Release has only the binary, not the .sig.
        let release = make_release("v1.2.3", &[target]);

        let result = find_asset_urls(&release, target);
        assert!(result.is_err(), "missing sig asset must return Err");
    }

    #[test]
    fn test_find_asset_urls_empty_assets() {
        let target = "aitrack-x86_64-unknown-linux-musl";
        let release = make_release("v1.2.3", &[]);

        let result = find_asset_urls(&release, target);
        assert!(result.is_err(), "empty asset list must return Err");
    }

    // -----------------------------------------------------------------------
    // platform_target_string: additional invariants
    // -----------------------------------------------------------------------

    #[test]
    fn test_platform_target_no_spaces() {
        let s = platform_target_string();
        assert!(
            !s.contains(' '),
            "platform string must not contain spaces: {s}"
        );
    }

    #[test]
    fn test_platform_target_contains_arch() {
        let s = platform_target_string();
        let arch = std::env::consts::ARCH;
        // The arch should appear somewhere in the string.
        let arch_mapped = arch;
        assert!(
            s.contains(arch_mapped),
            "platform string '{s}' should contain arch '{arch_mapped}'"
        );
    }
}
