package model

import "time"

// UsageDailyRollup stores scalar token usage by day/tool/model/account.
type UsageDailyRollup struct {
	TokenKey         string
	DeviceID         string
	Day              string
	Agent            string
	Model            string
	Account          string
	UsageBasis       string
	TokensIn         int64
	TokensOut        int64
	TokensCacheRead  int64
	TokensCacheWrite int64
	TokensReasoning  int64
	MessageCount     int64
	SourceCost       float64
	UpdatedAt        time.Time
}

// UsageSubscriptionSnapshot stores the latest scalar quota snapshot for a tool/account.
type UsageSubscriptionSnapshot struct {
	TokenKey              string
	DeviceID              string
	Agent                 string
	Account               string
	Plan                  *string
	QuotaSessionRemaining *int64
	QuotaWeeklyRemaining  *int64
	QuotaResetAt          *string
	ReaderStatus          string
	SnapshottedAt         string
	UpdatedAt             time.Time
}

// UsageSummary is the admin-facing scalar usage aggregate.
type UsageSummary struct {
	TotalTokens      int64              `json:"total_tokens"`
	TokensIn         int64              `json:"tokens_in"`
	TokensOut        int64              `json:"tokens_out"`
	TokensCacheRead  int64              `json:"tokens_cache_read"`
	TokensCacheWrite int64              `json:"tokens_cache_write"`
	TokensReasoning  int64              `json:"tokens_reasoning"`
	MessageCount     int64              `json:"message_count"`
	SourceCost       float64            `json:"source_cost"`
	Items            []UsageSummaryItem `json:"items"`
}

// UsageSummaryItem is a token total grouped by token/tool/model.
type UsageSummaryItem struct {
	TokenKey     string  `json:"token_key"`
	Agent        string  `json:"agent"`
	Model        string  `json:"model"`
	Account      string  `json:"account"`
	UsageBasis   string  `json:"usage_basis"`
	TotalTokens  int64   `json:"total_tokens"`
	MessageCount int64   `json:"message_count"`
	SourceCost   float64 `json:"source_cost"`
}
