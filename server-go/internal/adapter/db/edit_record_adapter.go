package dbadapter

import (
	"database/sql"
	"fmt"
	"strings"
	"time"

	"github.com/aitrack/server/internal/domain/model"
	"github.com/aitrack/server/internal/domain/port"
)

// EditRecordAdapter persists edit records. It implements port.EditRecordPort.
type EditRecordAdapter struct {
	db *sql.DB
}

// NewEditRecordAdapter constructs an EditRecordAdapter over the given database.
func NewEditRecordAdapter(db *sql.DB) *EditRecordAdapter {
	return &EditRecordAdapter{db: db}
}

var _ port.EditRecordPort = (*EditRecordAdapter)(nil)

// Save persists a single validated edit record.
func (r *EditRecordAdapter) Save(rec *model.EditRecord) error {
	_, err := r.db.Exec(`
		INSERT INTO edit_records
		  (token_key, device_id, hostname, tool, tool_version, provider, model, session_id,
		   repo_url, branch, current_sha, file_path, added_lines, removed_lines,
		   diff_hunk, metadata, timestamp, record_sig, status, flags, received_at, prompt_summary)
		VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22)`,
		rec.TokenKey, rec.DeviceID, rec.Hostname, rec.Tool, rec.ToolVersion, rec.Provider, rec.Model,
		rec.SessionID, rec.RepoURL, rec.Branch, rec.CurrentSHA, rec.FilePath,
		rec.AddedLines, rec.RemovedLines, rec.DiffHunk, rec.Metadata, rec.Timestamp,
		rec.RecordSig, rec.Status, rec.Flags,
		rec.ReceivedAt.UTC().Format(time.RFC3339),
		rec.PromptSummary,
	)
	return err
}

// CountByTokenKeyAndFilePathSince counts records for rate limiting.
func (r *EditRecordAdapter) CountByTokenKeyAndFilePathSince(tokenKey, filePath string, since time.Time) (int64, error) {
	var count int64
	err := r.db.QueryRow(
		`SELECT COUNT(*) FROM edit_records
		 WHERE token_key = $1 AND file_path = $2 AND received_at >= $3`,
		tokenKey, filePath, since.UTC().Format(time.RFC3339),
	).Scan(&count)
	return count, err
}

// Query returns a page of records plus the total count.
func (r *EditRecordAdapter) Query(tokenKey, repoURL string, page, size int) ([]model.EditRecord, int64, error) {
	var args []interface{}
	var conditions []string

	if tokenKey != "" {
		conditions = append(conditions, fmt.Sprintf("token_key = $%d", len(args)+1))
		args = append(args, tokenKey)
	}
	if repoURL != "" {
		conditions = append(conditions, fmt.Sprintf("repo_url = $%d", len(args)+1))
		args = append(args, repoURL)
	}

	where := ""
	if len(conditions) > 0 {
		where = "WHERE " + strings.Join(conditions, " AND ")
	}

	var total int64
	if err := r.db.QueryRow("SELECT COUNT(*) FROM edit_records "+where, args...).Scan(&total); err != nil {
		return nil, 0, err
	}

	offset := page * size
	limitN := len(args) + 1
	offsetN := len(args) + 2
	queryArgs := append(args, size, offset)
	rows, err := r.db.Query(
		`SELECT id, token_key, device_id, hostname, tool, tool_version, provider, model, session_id,
		        repo_url, branch, current_sha, file_path, added_lines, removed_lines,
		        diff_hunk, metadata, timestamp, record_sig, status, flags, received_at, prompt_summary
		 FROM edit_records `+where+fmt.Sprintf(` ORDER BY received_at DESC LIMIT $%d OFFSET $%d`, limitN, offsetN),
		queryArgs...,
	)
	if err != nil {
		return nil, 0, err
	}
	defer rows.Close()

	var records []model.EditRecord
	for rows.Next() {
		var rec model.EditRecord
		var receivedAt string
		var toolVersion, modelNS, diffHunk, metadata, flags, promptSummary sql.NullString
		if err := rows.Scan(
			&rec.ID, &rec.TokenKey, &rec.DeviceID, &rec.Hostname, &rec.Tool, &toolVersion,
			&rec.Provider, &modelNS, &rec.SessionID, &rec.RepoURL, &rec.Branch,
			&rec.CurrentSHA, &rec.FilePath, &rec.AddedLines, &rec.RemovedLines,
			&diffHunk, &metadata, &rec.Timestamp, &rec.RecordSig,
			&rec.Status, &flags, &receivedAt, &promptSummary,
		); err != nil {
			return nil, 0, err
		}
		rec.ToolVersion = toolVersion.String
		rec.Model = modelNS.String
		rec.DiffHunk = diffHunk.String
		rec.Metadata = metadata.String
		rec.Flags = flags.String
		if promptSummary.Valid {
			s := promptSummary.String
			rec.PromptSummary = &s
		}
		rec.ReceivedAt, _ = time.Parse(time.RFC3339, receivedAt)
		records = append(records, rec)
	}
	return records, total, rows.Err()
}

// AggregateByTokenKey aggregates stats grouped by token key.
func (r *EditRecordAdapter) AggregateByTokenKey() ([]model.StatsRow, error) {
	return r.aggregate("token_key")
}

// AggregateByRepo aggregates stats grouped by repo URL.
func (r *EditRecordAdapter) AggregateByRepo() ([]model.StatsRow, error) {
	return r.aggregate("repo_url")
}

// AggregateByDevice aggregates stats grouped by device ID.
func (r *EditRecordAdapter) AggregateByDevice() ([]model.StatsRow, error) {
	return r.aggregate("device_id")
}

// AggregateByHostname aggregates stats grouped by hostname.
func (r *EditRecordAdapter) AggregateByHostname() ([]model.StatsRow, error) {
	return r.aggregate("hostname")
}

// AggregateByTool aggregates stats grouped by tool.
func (r *EditRecordAdapter) AggregateByTool() ([]model.StatsRow, error) {
	return r.aggregate("tool")
}

var allowedGroupCols = map[string]bool{
	"token_key": true,
	"repo_url":  true,
	"device_id": true,
	"hostname":  true,
	"tool":      true,
}

func (r *EditRecordAdapter) aggregate(groupCol string) ([]model.StatsRow, error) {
	if !allowedGroupCols[groupCol] {
		return nil, fmt.Errorf("invalid group column: %q", groupCol)
	}
	rows, err := r.db.Query(`
		SELECT ` + groupCol + `,
		       COUNT(*) AS edits,
		       COALESCE(SUM(added_lines),0),
		       COALESCE(SUM(removed_lines),0),
		       COALESCE(SUM(CASE WHEN status='ACCEPTED' THEN 1 ELSE 0 END),0),
		       COALESCE(SUM(CASE WHEN status='FLAGGED'  THEN 1 ELSE 0 END),0),
		       COALESCE(SUM(CASE WHEN status='REJECTED' THEN 1 ELSE 0 END),0),
		       MAX(received_at)
		FROM edit_records
		GROUP BY ` + groupCol)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []model.StatsRow
	for rows.Next() {
		var sr model.StatsRow
		var lastActive string
		if err := rows.Scan(&sr.Group, &sr.Edits, &sr.AddedLines, &sr.RemovedLines,
			&sr.Accepted, &sr.Flagged, &sr.Rejected, &lastActive); err != nil {
			return nil, err
		}
		if lastActive != "" {
			t, _ := time.Parse(time.RFC3339, lastActive)
			sr.LastActive = &t
		}
		result = append(result, sr)
	}
	return result, rows.Err()
}
