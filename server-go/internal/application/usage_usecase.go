package application

import (
	"strings"

	"github.com/aitrack/server/internal/domain/model"
	"github.com/aitrack/server/internal/domain/port"
)

// UsageService processes scalar usage rollups and quota snapshots.
type UsageService struct {
	repo port.UsagePort
}

// NewUsageService constructs the usage use case.
func NewUsageService(repo port.UsagePort) *UsageService {
	return &UsageService{repo: repo}
}

// IngestRollups validates and persists scalar usage rollups.
func (s *UsageService) IngestRollups(token *model.Token, req *model.UsageRollupRequest) error {
	for i := range req.Items {
		req.Items[i].Agent = strings.TrimSpace(req.Items[i].Agent)
		req.Items[i].Model = strings.TrimSpace(req.Items[i].Model)
		req.Items[i].Account = strings.TrimSpace(req.Items[i].Account)
	}
	return s.repo.UpsertRollups(token.TokenKey, req.Items)
}

// IngestSubscription persists one scalar quota snapshot.
func (s *UsageService) IngestSubscription(token *model.Token, req *model.UsageSubscriptionSnapshotRequest) error {
	req.Agent = strings.TrimSpace(req.Agent)
	req.Account = strings.TrimSpace(req.Account)
	req.ReaderStatus = strings.TrimSpace(req.ReaderStatus)
	return s.repo.UpsertSubscription(token.TokenKey, req)
}

// Summary returns grouped scalar usage totals.
func (s *UsageService) Summary(tokenKey, fromDay, toDay, agent string, limit int) (*model.UsageSummary, error) {
	return s.repo.Summary(tokenKey, fromDay, toDay, agent, limit)
}
