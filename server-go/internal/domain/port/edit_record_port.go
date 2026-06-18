// Package port defines the driven-side interfaces (secondary ports) of the
// hexagon. Application use cases depend on these interfaces; the adapter/db
// package provides the concrete implementations.
package port

import (
	"time"

	"github.com/aitrack/server/internal/domain/model"
)

// EditRecordPort is the persistence port for edit records.
type EditRecordPort interface {
	// Save persists a single validated edit record.
	Save(record *model.EditRecord) error
	// CountByTokenKeyAndFilePathSince counts records for rate limiting.
	CountByTokenKeyAndFilePathSince(tokenKey, filePath string, since time.Time) (int64, error)
	// Query returns a page of records plus the total count.
	Query(tokenKey, repoURL string, page, size int) ([]model.EditRecord, int64, error)
	// AggregateByTokenKey aggregates stats grouped by token key.
	AggregateByTokenKey() ([]model.StatsRow, error)
	// AggregateByRepo aggregates stats grouped by repo URL.
	AggregateByRepo() ([]model.StatsRow, error)
	// AggregateByDevice aggregates stats grouped by device ID.
	AggregateByDevice() ([]model.StatsRow, error)
	// AggregateByHostname aggregates stats grouped by hostname.
	AggregateByHostname() ([]model.StatsRow, error)
	// AggregateByTool aggregates stats grouped by tool name.
	AggregateByTool() ([]model.StatsRow, error)
}

// EditRecordCounter is the narrow port used by the validation domain service
// for rate limiting; EditRecordPort satisfies it.
type EditRecordCounter interface {
	CountByTokenKeyAndFilePathSince(tokenKey, filePath string, since time.Time) (int64, error)
}
