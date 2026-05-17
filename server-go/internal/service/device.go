package service

import (
	"database/sql"
	"encoding/json"
	"time"

	"github.com/aitrack/server/internal/model"
)

// DeviceRepository handles device upsert and listing.
type DeviceRepository struct {
	db *sql.DB
}

func NewDeviceRepository(db *sql.DB) *DeviceRepository {
	return &DeviceRepository{db: db}
}

func (r *DeviceRepository) Upsert(d *model.Device) error {
	var lastHB interface{}
	if d.LastHeartbeat != nil {
		lastHB = d.LastHeartbeat.UTC().Format(time.RFC3339)
	}
	_, err := r.db.Exec(`
		INSERT INTO devices (device_id, token_key, hostname, client_version, last_heartbeat, hooks_json, created_at)
		VALUES (?, ?, ?, ?, ?, ?, ?)
		ON CONFLICT(device_id) DO UPDATE SET
		  token_key      = excluded.token_key,
		  hostname       = excluded.hostname,
		  client_version = excluded.client_version,
		  last_heartbeat = excluded.last_heartbeat,
		  hooks_json     = excluded.hooks_json`,
		d.DeviceID, d.TokenKey, d.Hostname, d.ClientVersion, lastHB, d.HooksJSON,
		d.CreatedAt.UTC().Format(time.RFC3339),
	)
	return err
}

func (r *DeviceRepository) FindByDeviceID(deviceID string) (*model.Device, error) {
	row := r.db.QueryRow(`
		SELECT id, device_id, token_key, hostname, client_version, last_heartbeat, hooks_json, created_at
		FROM devices WHERE device_id = ?`, deviceID)
	var d model.Device
	var lastHB, createdAt sql.NullString
	err := row.Scan(&d.ID, &d.DeviceID, &d.TokenKey, &d.Hostname, &d.ClientVersion, &lastHB, &d.HooksJSON, &createdAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	if lastHB.Valid {
		t, _ := time.Parse(time.RFC3339, lastHB.String)
		d.LastHeartbeat = &t
	}
	d.CreatedAt, _ = time.Parse(time.RFC3339, createdAt.String)
	return &d, nil
}

func (r *DeviceRepository) ListAll() ([]*model.Device, error) {
	rows, err := r.db.Query(`
		SELECT id, device_id, token_key, hostname, client_version, last_heartbeat, hooks_json, created_at
		FROM devices`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []*model.Device
	for rows.Next() {
		var d model.Device
		var lastHB, createdAt sql.NullString
		if err := rows.Scan(&d.ID, &d.DeviceID, &d.TokenKey, &d.Hostname, &d.ClientVersion, &lastHB, &d.HooksJSON, &createdAt); err != nil {
			return nil, err
		}
		if lastHB.Valid {
			t, _ := time.Parse(time.RFC3339, lastHB.String)
			d.LastHeartbeat = &t
		}
		d.CreatedAt, _ = time.Parse(time.RFC3339, createdAt.String)
		result = append(result, &d)
	}
	return result, rows.Err()
}

// HeartbeatService records heartbeat events.
type HeartbeatService struct {
	deviceRepo *DeviceRepository
}

func NewHeartbeatService(repo *DeviceRepository) *HeartbeatService {
	return &HeartbeatService{deviceRepo: repo}
}

func (s *HeartbeatService) RecordHeartbeat(token *model.Token, req *model.HeartbeatRequest) error {
	existing, err := s.deviceRepo.FindByDeviceID(req.DeviceID)
	if err != nil {
		return err
	}

	now := time.Now().UTC()
	d := &model.Device{
		DeviceID:      req.DeviceID,
		TokenKey:      token.TokenKey,
		Hostname:      req.Hostname,
		ClientVersion: req.ClientVersion,
		LastHeartbeat: &now,
		CreatedAt:     now,
	}
	if existing != nil {
		d.CreatedAt = existing.CreatedAt
	}
	if req.Hooks != nil {
		b, _ := json.Marshal(req.Hooks)
		d.HooksJSON = string(b)
	} else if existing != nil {
		d.HooksJSON = existing.HooksJSON
	}
	return s.deviceRepo.Upsert(d)
}

// StatsService aggregates and lists stats.
type StatsService struct {
	editRepo   *EditRecordRepository
	deviceRepo *DeviceRepository
}

func NewStatsService(editRepo *EditRecordRepository, deviceRepo *DeviceRepository) *StatsService {
	return &StatsService{editRepo: editRepo, deviceRepo: deviceRepo}
}

func (s *StatsService) GetStats(groupBy string) ([]model.StatsRow, error) {
	var rows []StatsRow
	var err error
	switch groupBy {
	case "repo":
		rows, err = s.editRepo.AggregateByRepo()
	case "device":
		rows, err = s.editRepo.AggregateByDevice()
	case "hostname":
		rows, err = s.editRepo.AggregateByHostname()
	default:
		rows, err = s.editRepo.AggregateByTokenKey()
	}
	if err != nil {
		return nil, err
	}
	result := make([]model.StatsRow, len(rows))
	for i, r := range rows {
		result[i] = model.StatsRow{
			Group:        r.Group,
			Edits:        r.Edits,
			AddedLines:   r.AddedLines,
			RemovedLines: r.RemovedLines,
			Accepted:     r.Accepted,
			Flagged:      r.Flagged,
			Rejected:     r.Rejected,
			LastActive:   r.LastActive,
		}
	}
	return result, nil
}

func (s *StatsService) GetDevices() ([]model.DeviceInfo, error) {
	devices, err := s.deviceRepo.ListAll()
	if err != nil {
		return nil, err
	}
	silentThreshold := time.Now().Add(-7 * 24 * time.Hour)
	result := make([]model.DeviceInfo, len(devices))
	for i, d := range devices {
		silent := d.LastHeartbeat == nil || d.LastHeartbeat.Before(silentThreshold)
		result[i] = model.DeviceInfo{
			DeviceID:      d.DeviceID,
			TokenKey:      d.TokenKey,
			Hostname:      d.Hostname,
			ClientVersion: d.ClientVersion,
			LastHeartbeat: d.LastHeartbeat,
			HooksJSON:     d.HooksJSON,
			Silent:        silent,
		}
	}
	return result, nil
}
