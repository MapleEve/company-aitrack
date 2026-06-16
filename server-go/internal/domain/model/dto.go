package model

// CreateTokenRequest is the body of POST /admin/tokens.
type CreateTokenRequest struct {
	Owner string `json:"owner"`
	Note  string `json:"note"`
}

// CreateTokenResponse is returned by POST /admin/tokens.
type CreateTokenResponse struct {
	Credential string `json:"credential"`
	TokenKey   string `json:"token_key"`
}

// EditDTO is a single edit as submitted by the client.
type EditDTO struct {
	Tool          string  `json:"tool"`
	ToolVersion   string  `json:"tool_version"`
	Provider      string  `json:"provider"`
	Model         *string `json:"model"`
	SessionID     string  `json:"session_id"`
	RepoURL       string  `json:"repo_url"`
	Branch        string  `json:"branch"`
	CurrentSHA    string  `json:"current_sha"`
	FilePath      string  `json:"file_path"`
	AddedLines    *int64  `json:"added_lines"`
	RemovedLines  *int64  `json:"removed_lines"`
	DiffHunk      *string `json:"diff_hunk"`
	Metadata      *string `json:"metadata"`
	Timestamp     string  `json:"timestamp"`
	DeviceID      string  `json:"device_id"`
	Hostname      string  `json:"hostname"`
	RecordSig     string  `json:"record_sig"`
	PromptSummary *string `json:"prompt_summary,omitempty"`
}

// EditBatchRequest is the body of POST /api/v1/ai-track/edits.
type EditBatchRequest struct {
	DeviceID      string    `json:"device_id"`
	ClientVersion string    `json:"client_version"`
	Edits         []EditDTO `json:"edits"`
}

// IndexedReason ties a rejection/flag reason to a batch index.
type IndexedReason struct {
	Index  int    `json:"index"`
	Reason string `json:"reason"`
}

// EditBatchResponse is the result of an ingest batch.
type EditBatchResponse struct {
	Accepted int             `json:"accepted"`
	Rejected []IndexedReason `json:"rejected"`
	Flagged  []IndexedReason `json:"flagged"`
}

// HeartbeatHooks reports installed hooks by agent key.
type HeartbeatHooks map[string]bool

// HeartbeatRequest is the body of POST /api/v1/ai-track/heartbeat.
type HeartbeatRequest struct {
	DeviceID       string         `json:"device_id"`
	Hostname       string         `json:"hostname"`
	TokenKeyMasked string         `json:"token_key_masked"`
	ClientVersion  string         `json:"client_version"`
	TS             int64          `json:"ts"`
	Hooks          HeartbeatHooks `json:"hooks"`
	PendingCount   int            `json:"pending_count"`
}

// UsageRollupRequest is the scalar usage rollup ingestion body.
type UsageRollupRequest struct {
	Items []UsageRollupItem `json:"items"`
}

// UsageRollupItem is a day/model/account token bucket aggregate.
type UsageRollupItem struct {
	DeviceID         string `json:"device_id"`
	Day              string `json:"day"`
	Agent            string `json:"agent"`
	Model            string `json:"model"`
	Account          string `json:"account"`
	TokensIn         int64  `json:"tokens_in"`
	TokensOut        int64  `json:"tokens_out"`
	TokensCacheRead  int64  `json:"tokens_cache_read"`
	TokensCacheWrite int64  `json:"tokens_cache_write"`
	TokensReasoning  int64  `json:"tokens_reasoning"`
}

// UsageSubscriptionSnapshotRequest is a scalar subscription/quota snapshot.
type UsageSubscriptionSnapshotRequest struct {
	DeviceID              string  `json:"device_id"`
	Agent                 string  `json:"agent"`
	Account               string  `json:"account"`
	Plan                  *string `json:"plan"`
	QuotaSessionRemaining *int64  `json:"quota_session_remaining"`
	QuotaWeeklyRemaining  *int64  `json:"quota_weekly_remaining"`
	QuotaResetAt          *string `json:"quota_reset_at"`
	ReaderStatus          string  `json:"reader_status"`
	SnapshottedAt         string  `json:"snapshotted_at"`
}
