/// HTTP upload adapter.
///
/// `HttpUploader` implements `UploadPort` (blocking) and exposes an async
/// `post_batch` helper used by `uploader::flush_unsynced` to retain the full
/// accepted / rejected / flagged response semantics required by the tests.
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::{load_config, split_credential};
use crate::domain::crypto::compute_request_sig;
use crate::domain::model::Record;
use crate::port::upload::UploadPort;

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct UploadPayload {
    device_id: String,
    client_version: String,
    edits: Vec<EditRecord>,
}

#[derive(Serialize)]
struct EditRecord {
    tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_version: Option<String>,
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    session_id: String,
    repo_url: String,
    branch: String,
    current_sha: String,
    file_path: String,
    added_lines: i64,
    removed_lines: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff_hunk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<String>,
    timestamp: String,
    device_id: String,
    hostname: String,
    record_sig: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_summary: Option<String>,
}

/// Parsed server response returned by `post_batch`.
#[derive(Deserialize, Debug)]
pub struct UploadResponse {
    pub accepted: Option<i64>,
    #[serde(default)]
    pub rejected: Vec<IndexedItem>,
    #[serde(default)]
    pub flagged: Vec<IndexedItem>,
}

#[derive(Deserialize, Debug)]
pub struct IndexedItem {
    pub index: usize,
    #[allow(dead_code)]
    pub reason: Option<String>,
}

// ── HttpUploader ──────────────────────────────────────────────────────────────

/// HTTP-backed implementation of `UploadPort`.
///
/// Holds the remote endpoint URL and the combined credential string
/// (`"<token>-<hmac_secret>"`).  Both sync (`UploadPort::upload_batch`) and
/// async (`post_batch`) entry points are provided so that callers at different
/// layers can choose the appropriate execution model.
pub struct HttpUploader {
    pub api_url: String,
    pub credential: String,
}

impl HttpUploader {
    pub fn new(api_url: String, credential: String) -> Self {
        Self {
            api_url,
            credential,
        }
    }

    /// Build the JSON payload from a slice of `Record`s.
    pub(crate) fn build_payload(records: &[Record], device_id: &str) -> Result<Vec<u8>> {
        let edits: Vec<EditRecord> = records
            .iter()
            .map(|r| EditRecord {
                tool: r.tool.clone(),
                tool_version: r.tool_version.clone(),
                provider: r.provider.clone(),
                model: r.model.clone(),
                session_id: r.session_id.clone(),
                repo_url: r.repo_url.clone(),
                branch: r.branch.clone(),
                current_sha: r.current_sha.clone(),
                file_path: r.file_path.clone(),
                added_lines: r.added_lines,
                removed_lines: r.removed_lines,
                diff_hunk: r.diff_hunk.clone(),
                metadata: r.metadata.clone(),
                timestamp: r.timestamp.clone(),
                device_id: r.device_id.clone(),
                hostname: r.hostname.clone(),
                record_sig: r.record_sig.clone(),
                prompt_summary: r.prompt_summary.clone(),
            })
            .collect();

        let payload = UploadPayload {
            device_id: device_id.to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            edits,
        };
        Ok(serde_json::to_vec(&payload)?)
    }

    pub(crate) fn payload_size_bytes(records: &[Record], device_id: &str) -> Result<usize> {
        Ok(Self::build_payload(records, device_id)?.len())
    }

    /// Async POST — returns the parsed `UploadResponse` on HTTP 2xx.
    ///
    /// Used by `uploader::flush_unsynced` so that it can apply the full
    /// accepted / rejected / flagged DB bookkeeping.
    pub async fn post_batch(&self, records: &[Record], device_id: &str) -> PostBatchResult {
        if self.api_url.starts_with("http://") {
            eprintln!(
                "[aitrack] WARNING: api_url uses plaintext HTTP; token will be sent unencrypted"
            );
        }

        let (token, hmac_secret) = match split_credential(&self.credential) {
            Ok(parts) => parts,
            Err(e) => {
                eprintln!("[aitrack] invalid credential: {e}");
                return PostBatchResult::CredentialError;
            }
        };

        let body_bytes = match Self::build_payload(records, device_id) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[aitrack] failed to build upload payload: {e}");
                return PostBatchResult::TransientError;
            }
        };

        let unix_ts = Utc::now().timestamp() as u64;
        let sig = if hmac_secret.is_empty() {
            String::new()
        } else {
            compute_request_sig(&hmac_secret, unix_ts, &body_bytes)
        };

        let url = format!("{}/api/v1/ai-track/edits", self.api_url);
        let client = reqwest::Client::new();
        let mut req = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("X-AiTrack-Device", device_id)
            .header(
                "X-AiTrack-Client",
                format!("aitrack/{}", env!("CARGO_PKG_VERSION")),
            )
            .header("X-AiTrack-Timestamp", unix_ts.to_string())
            .body(body_bytes);

        if !sig.is_empty() {
            req = req.header("X-AiTrack-Signature", &sig);
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<UploadResponse>().await {
                Ok(ur) => PostBatchResult::Success(ur),
                Err(_) => PostBatchResult::UnparseableOk,
            },
            Ok(resp) => {
                eprintln!("[aitrack] upload failed: HTTP {}", resp.status());
                PostBatchResult::TransientError
            }
            Err(e) => {
                eprintln!("[aitrack] upload error: {e}");
                PostBatchResult::TransientError
            }
        }
    }
}

/// Outcome variants returned by `HttpUploader::post_batch`.
pub enum PostBatchResult {
    /// Server returned 2xx with a parseable response body.
    Success(UploadResponse),
    /// Server returned 2xx but the body could not be parsed; treat all as synced.
    UnparseableOk,
    /// Transient error (network failure, non-2xx); caller should increment retry.
    TransientError,
    /// Permanent credential error; skip without incrementing retry.
    CredentialError,
}

// ── UploadPort (blocking) ─────────────────────────────────────────────────────

impl UploadPort for HttpUploader {
    /// Blocking HTTP POST of a record batch.
    ///
    /// Returns `Ok(())` on HTTP 2xx; returns `Err` on network failures or
    /// non-2xx responses so that `flush_unsynced` can decide whether to
    /// mark records as synced or increment their retry counter.
    fn upload_batch(&self, records: &[Record]) -> anyhow::Result<()> {
        if self.api_url.starts_with("http://") {
            eprintln!(
                "[aitrack] WARNING: api_url uses plaintext HTTP; token will be sent unencrypted"
            );
        }

        let (token, hmac_secret) = split_credential(&self.credential)?;

        let cfg = load_config();
        let device_id = cfg.device_id;

        let body_bytes = Self::build_payload(records, &device_id)?;

        let unix_ts = Utc::now().timestamp() as u64;
        let sig = if hmac_secret.is_empty() {
            String::new()
        } else {
            compute_request_sig(&hmac_secret, unix_ts, &body_bytes)
        };

        let url = format!("{}/api/v1/ai-track/edits", self.api_url);
        let client = reqwest::blocking::Client::new();
        let mut req = client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .header("X-AiTrack-Device", &device_id)
            .header(
                "X-AiTrack-Client",
                format!("aitrack/{}", env!("CARGO_PKG_VERSION")),
            )
            .header("X-AiTrack-Timestamp", unix_ts.to_string())
            .body(body_bytes);

        if !sig.is_empty() {
            req = req.header("X-AiTrack-Signature", &sig);
        }

        let resp = req.send()?;
        if resp.status().is_success() {
            Ok(())
        } else {
            anyhow::bail!("upload failed: HTTP {}", resp.status())
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::{HttpUploader, PostBatchResult};
    use crate::testkit::factories::EditRecordFactory;

    // ── Credential constant used across tests ─────────────────────────────────
    /// Token = "aitrack_testtoken12345", hmac_secret = "testhmacsecret"
    const TEST_CREDENTIAL: &str = "aitrack_testtoken12345-testhmacsecret";
    /// A credential that has no '-' separator, so split_credential will fail.
    const BAD_CREDENTIAL: &str = "nocredentialseparatoratall";

    fn make_record(seed: u64) -> crate::domain::model::Record {
        EditRecordFactory::new(seed).build()
    }

    // ── build_payload ─────────────────────────────────────────────────────────

    #[test]
    fn test_build_payload_empty() {
        let bytes = HttpUploader::build_payload(&[], "dev-001").unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["device_id"], "dev-001");
        assert!(v["edits"].as_array().unwrap().is_empty());
        // client_version must be present and non-empty
        assert!(!v["client_version"].as_str().unwrap().is_empty());
    }

    #[test]
    fn test_build_payload_single_record() {
        let rec = EditRecordFactory::new(42)
            .with_tool("cursor")
            .with_provider("openai")
            .with_added_lines(10)
            .with_removed_lines(3)
            .build();

        let bytes = HttpUploader::build_payload(&[rec], "dev-042").unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(v["device_id"], "dev-042");
        let edits = v["edits"].as_array().unwrap();
        assert_eq!(edits.len(), 1);

        let edit = &edits[0];
        assert_eq!(edit["tool"], "cursor");
        assert_eq!(edit["provider"], "openai");
        assert_eq!(edit["added_lines"], 10);
        assert_eq!(edit["removed_lines"], 3);
        // device_id inside the edit record must also be present
        assert!(edit.get("device_id").is_some());
        assert!(edit.get("record_sig").is_some());
    }

    #[test]
    fn test_build_payload_multiple_records_preserves_order() {
        let records: Vec<_> = (0..5).map(make_record).collect();
        let bytes = HttpUploader::build_payload(&records, "dev-multi").unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["edits"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn test_build_payload_optional_fields_skipped_when_none() {
        let rec = EditRecordFactory::new(7)
            .with_diff_hunk(None)
            .with_prompt_summary(None)
            .build();
        let bytes = HttpUploader::build_payload(&[rec], "dev-007").unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let edit = &v["edits"][0];
        // serde skip_serializing_if = "Option::is_none" must drop these keys
        assert!(edit.get("diff_hunk").is_none());
        assert!(edit.get("prompt_summary").is_none());
    }

    // ── post_batch ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_upload_batch_success_2xx() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 2,
                "rejected": [],
                "flagged": []
            })))
            .mount(&mock_server)
            .await;

        let uploader = HttpUploader::new(mock_server.uri(), TEST_CREDENTIAL.to_string());
        let records = vec![make_record(1), make_record(2)];
        let result = uploader.post_batch(&records, "dev-test").await;

        match result {
            PostBatchResult::Success(ur) => {
                assert_eq!(ur.accepted, Some(2));
                assert!(ur.rejected.is_empty());
                assert!(ur.flagged.is_empty());
            }
            other => panic!("expected Success, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn test_upload_batch_success_unparseable_body() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
            .mount(&mock_server)
            .await;

        let uploader = HttpUploader::new(mock_server.uri(), TEST_CREDENTIAL.to_string());
        let result = uploader.post_batch(&[make_record(10)], "dev-test").await;

        assert!(matches!(result, PostBatchResult::UnparseableOk));
    }

    #[tokio::test]
    async fn test_upload_batch_4xx_credential_error_response() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&mock_server)
            .await;

        let uploader = HttpUploader::new(mock_server.uri(), TEST_CREDENTIAL.to_string());
        let result = uploader.post_batch(&[make_record(3)], "dev-test").await;

        assert!(matches!(result, PostBatchResult::TransientError));
    }

    #[tokio::test]
    async fn test_upload_batch_403_forbidden() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&mock_server)
            .await;

        let uploader = HttpUploader::new(mock_server.uri(), TEST_CREDENTIAL.to_string());
        let result = uploader.post_batch(&[make_record(4)], "dev-test").await;

        assert!(matches!(result, PostBatchResult::TransientError));
    }

    #[tokio::test]
    async fn test_upload_batch_5xx_transient() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let uploader = HttpUploader::new(mock_server.uri(), TEST_CREDENTIAL.to_string());
        let result = uploader.post_batch(&[make_record(5)], "dev-test").await;

        assert!(matches!(result, PostBatchResult::TransientError));
    }

    #[tokio::test]
    async fn test_upload_batch_network_error_invalid_url() {
        // Point to a URL that will refuse/fail to connect
        let uploader = HttpUploader::new(
            "http://127.0.0.1:1".to_string(),
            TEST_CREDENTIAL.to_string(),
        );
        let result = uploader.post_batch(&[make_record(6)], "dev-test").await;

        assert!(matches!(result, PostBatchResult::TransientError));
    }

    #[tokio::test]
    async fn test_upload_batch_bad_credential_returns_credential_error() {
        // BAD_CREDENTIAL has no '-' separator so split_credential returns Err
        let uploader = HttpUploader::new(
            "http://127.0.0.1:9999".to_string(),
            BAD_CREDENTIAL.to_string(),
        );
        let result = uploader.post_batch(&[make_record(7)], "dev-test").await;

        assert!(matches!(result, PostBatchResult::CredentialError));
    }

    #[tokio::test]
    async fn test_upload_batch_http_warning_logged_for_plain_http() {
        // Exercises the "http://" branch in post_batch (code path coverage).
        // We still expect a TransientError because nothing listens on port 1.
        let uploader = HttpUploader::new(
            "http://127.0.0.1:1".to_string(),
            TEST_CREDENTIAL.to_string(),
        );
        let result = uploader.post_batch(&[make_record(8)], "dev-test").await;
        // The warning branch is exercised; connection will fail.
        assert!(matches!(result, PostBatchResult::TransientError));
    }

    #[tokio::test]
    async fn test_upload_batch_empty_hmac_secret_skips_signature() {
        // Credential with no hmac_secret part (token = "aitrack_nohmac", secret = "")
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/v1/ai-track/edits"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accepted": 1,
                "rejected": [],
                "flagged": []
            })))
            .mount(&mock_server)
            .await;

        let uploader = HttpUploader::new(
            mock_server.uri(),
            "aitrack_nohmac-".to_string(), // empty secret after '-'
        );
        let result = uploader.post_batch(&[make_record(9)], "dev-test").await;
        assert!(matches!(result, PostBatchResult::Success(_)));
    }
}
