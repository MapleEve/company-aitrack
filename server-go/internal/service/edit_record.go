package service

import (
	"database/sql"
	"strings"
	"time"

	"github.com/aitrack/server/internal/model"
)

// EditRecordRepository handles persistence of edit records.
type EditRecordRepository struct {
	db *sql.DB
}

func NewEditRecordRepository(db *sql.DB) *EditRecordRepository {
	return &EditRecordRepository{db: db}
}

func (r *EditRecordRepository) Save(rec *model.EditRecord) error {
	_, err := r.db.Exec(`
		INSERT INTO edit_records
		  (token_key, device_id, hostname, tool, tool_version, provider, model, session_id,
		   repo_url, branch, current_sha, file_path, added_lines, removed_lines,
		   diff_hunk, metadata, timestamp, record_sig, status, flags, received_at)
		VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)`,
		rec.TokenKey, rec.DeviceID, rec.Hostname, rec.Tool, rec.ToolVersion, rec.Provider, rec.Model,
		rec.SessionID, rec.RepoURL, rec.Branch, rec.CurrentSHA, rec.FilePath,
		rec.AddedLines, rec.RemovedLines, rec.DiffHunk, rec.Metadata, rec.Timestamp,
		rec.RecordSig, rec.Status, rec.Flags,
		rec.ReceivedAt.UTC().Format(time.RFC3339),
	)
	return err
}

func (r *EditRecordRepository) CountByTokenKeyAndFilePathSince(tokenKey, filePath string, since time.Time) (int64, error) {
	var count int64
	err := r.db.QueryRow(
		`SELECT COUNT(*) FROM edit_records
		 WHERE token_key = ? AND file_path = ? AND received_at >= ?`,
		tokenKey, filePath, since.UTC().Format(time.RFC3339),
	).Scan(&count)
	return count, err
}

func (r *EditRecordRepository) Query(tokenKey, repoURL string, page, size int) ([]model.EditRecord, int64, error) {
	var args []interface{}
	var conditions []string

	if tokenKey != "" {
		conditions = append(conditions, "token_key = ?")
		args = append(args, tokenKey)
	}
	if repoURL != "" {
		conditions = append(conditions, "repo_url = ?")
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
	queryArgs := append(args, size, offset)
	rows, err := r.db.Query(
		`SELECT id, token_key, device_id, hostname, tool, tool_version, provider, model, session_id,
		        repo_url, branch, current_sha, file_path, added_lines, removed_lines,
		        diff_hunk, metadata, timestamp, record_sig, status, flags, received_at
		 FROM edit_records `+where+` ORDER BY received_at DESC LIMIT ? OFFSET ?`,
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
		if err := rows.Scan(
			&rec.ID, &rec.TokenKey, &rec.DeviceID, &rec.Hostname, &rec.Tool, &rec.ToolVersion,
			&rec.Provider, &rec.Model, &rec.SessionID, &rec.RepoURL, &rec.Branch,
			&rec.CurrentSHA, &rec.FilePath, &rec.AddedLines, &rec.RemovedLines,
			&rec.DiffHunk, &rec.Metadata, &rec.Timestamp, &rec.RecordSig,
			&rec.Status, &rec.Flags, &receivedAt,
		); err != nil {
			return nil, 0, err
		}
		rec.ReceivedAt, _ = time.Parse(time.RFC3339, receivedAt)
		records = append(records, rec)
	}
	return records, total, rows.Err()
}

type StatsRow struct {
	Group        string
	Edits        int64
	AddedLines   int64
	RemovedLines int64
	Accepted     int64
	Flagged      int64
	Rejected     int64
	LastActive   *time.Time
}

func (r *EditRecordRepository) AggregateByTokenKey() ([]StatsRow, error) {
	return r.aggregate("token_key")
}

func (r *EditRecordRepository) AggregateByRepo() ([]StatsRow, error) {
	return r.aggregate("repo_url")
}

func (r *EditRecordRepository) AggregateByDevice() ([]StatsRow, error) {
	return r.aggregate("device_id")
}

func (r *EditRecordRepository) AggregateByHostname() ([]StatsRow, error) {
	return r.aggregate("hostname")
}

func (r *EditRecordRepository) aggregate(groupCol string) ([]StatsRow, error) {
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
	var result []StatsRow
	for rows.Next() {
		var sr StatsRow
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
