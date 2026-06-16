package dbadapter

import (
	"database/sql"
	"time"

	"github.com/aitrack/server/internal/domain/model"
	"github.com/aitrack/server/internal/domain/port"
)

// UsageAdapter persists scalar usage data. It implements port.UsagePort.
type UsageAdapter struct {
	db *sql.DB
}

// NewUsageAdapter constructs a UsageAdapter over the given database.
func NewUsageAdapter(db *sql.DB) *UsageAdapter {
	return &UsageAdapter{db: db}
}

var _ port.UsagePort = (*UsageAdapter)(nil)

// UpsertRollups inserts or replaces daily scalar token buckets.
func (r *UsageAdapter) UpsertRollups(tokenKey string, items []model.UsageRollupItem) error {
	tx, err := r.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback() //nolint:errcheck

	now := time.Now().UTC().Format(time.RFC3339)
	for _, item := range items {
		if _, err := tx.Exec(`
			INSERT INTO usage_daily_rollups
			  (token_key, device_id, day, agent, model, account, tokens_in, tokens_out,
			   tokens_cache_read, tokens_cache_write, tokens_reasoning, message_count, source_cost, updated_at)
			VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
			ON CONFLICT(token_key, device_id, day, agent, model, account) DO UPDATE SET
			  tokens_in = excluded.tokens_in,
			  tokens_out = excluded.tokens_out,
			  tokens_cache_read = excluded.tokens_cache_read,
			  tokens_cache_write = excluded.tokens_cache_write,
			  tokens_reasoning = excluded.tokens_reasoning,
			  message_count = excluded.message_count,
			  source_cost = excluded.source_cost,
			  updated_at = excluded.updated_at`,
			tokenKey, item.DeviceID, item.Day, item.Agent, item.Model, item.Account,
			item.TokensIn, item.TokensOut, item.TokensCacheRead, item.TokensCacheWrite,
			item.TokensReasoning, item.MessageCount, item.SourceCost, now,
		); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// UpsertSubscription inserts or replaces the latest scalar quota snapshot.
func (r *UsageAdapter) UpsertSubscription(tokenKey string, snapshot *model.UsageSubscriptionSnapshotRequest) error {
	now := time.Now().UTC().Format(time.RFC3339)
	_, err := r.db.Exec(`
		INSERT INTO usage_subscription_snapshots
		  (token_key, device_id, agent, account, plan, quota_session_remaining,
		   quota_weekly_remaining, quota_reset_at, reader_status, snapshotted_at, updated_at)
		VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
		ON CONFLICT(token_key, device_id, agent, account) DO UPDATE SET
		  plan = excluded.plan,
		  quota_session_remaining = excluded.quota_session_remaining,
		  quota_weekly_remaining = excluded.quota_weekly_remaining,
		  quota_reset_at = excluded.quota_reset_at,
		  reader_status = excluded.reader_status,
		  snapshotted_at = excluded.snapshotted_at,
		  updated_at = excluded.updated_at`,
		tokenKey, snapshot.DeviceID, snapshot.Agent, snapshot.Account, snapshot.Plan,
		snapshot.QuotaSessionRemaining, snapshot.QuotaWeeklyRemaining, snapshot.QuotaResetAt,
		snapshot.ReaderStatus, snapshot.SnapshottedAt, now,
	)
	return err
}

// Summary returns grouped scalar usage totals.
func (r *UsageAdapter) Summary(tokenKey, fromDay, toDay, agent string, limit int) (*model.UsageSummary, error) {
	if limit <= 0 || limit > 100 {
		limit = 20
	}
	rows, err := r.db.Query(`
		SELECT token_key, agent, model, account,
		       SUM(tokens_in + tokens_out + tokens_cache_read + tokens_cache_write + tokens_reasoning) AS total_tokens,
		       COALESCE(SUM(message_count),0), COALESCE(SUM(source_cost),0)
		FROM usage_daily_rollups
		WHERE ($1 = '' OR token_key = $1)
		  AND ($2 = '' OR day >= $2)
		  AND ($3 = '' OR day <= $3)
		  AND ($4 = '' OR agent = $4)
		GROUP BY token_key, agent, model, account
		ORDER BY total_tokens DESC
		LIMIT $5`,
		tokenKey, fromDay, toDay, agent, limit,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	summary := &model.UsageSummary{}
	for rows.Next() {
		var item model.UsageSummaryItem
		if err := rows.Scan(
			&item.TokenKey,
			&item.Agent,
			&item.Model,
			&item.Account,
			&item.TotalTokens,
			&item.MessageCount,
			&item.SourceCost,
		); err != nil {
			return nil, err
		}
		summary.TotalTokens += item.TotalTokens
		summary.MessageCount += item.MessageCount
		summary.SourceCost += item.SourceCost
		summary.Items = append(summary.Items, item)
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}

	row := r.db.QueryRow(`
		SELECT COALESCE(SUM(tokens_in),0), COALESCE(SUM(tokens_out),0),
		       COALESCE(SUM(tokens_cache_read),0), COALESCE(SUM(tokens_cache_write),0),
		       COALESCE(SUM(tokens_reasoning),0), COALESCE(SUM(message_count),0),
		       COALESCE(SUM(source_cost),0)
		FROM usage_daily_rollups
		WHERE ($1 = '' OR token_key = $1)
		  AND ($2 = '' OR day >= $2)
		  AND ($3 = '' OR day <= $3)
		  AND ($4 = '' OR agent = $4)`,
		tokenKey, fromDay, toDay, agent,
	)
	if err := row.Scan(
		&summary.TokensIn,
		&summary.TokensOut,
		&summary.TokensCacheRead,
		&summary.TokensCacheWrite,
		&summary.TokensReasoning,
		&summary.MessageCount,
		&summary.SourceCost,
	); err != nil {
		return nil, err
	}
	summary.TotalTokens = summary.TokensIn + summary.TokensOut + summary.TokensCacheRead + summary.TokensCacheWrite + summary.TokensReasoning
	return summary, nil
}
