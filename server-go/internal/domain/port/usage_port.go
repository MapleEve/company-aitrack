package port

import "github.com/aitrack/server/internal/domain/model"

// UsagePort persists scalar usage rollups and quota snapshots.
type UsagePort interface {
	UpsertRollups(tokenKey string, items []model.UsageRollupItem) error
	UpsertSubscription(tokenKey string, snapshot *model.UsageSubscriptionSnapshotRequest) error
	Summary(tokenKey, fromDay, toDay, agent string, limit int) (*model.UsageSummary, error)
}
