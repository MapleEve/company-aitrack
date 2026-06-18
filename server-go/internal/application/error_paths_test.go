package application_test

// error_paths_test.go covers error-return branches in the application layer:
//   - IngestService.saveEdit Save failure → rejected with save_error
//   - IngestService.saveEdit Save failure for FLAGGED → rejected with save_error
//   - IngestService.QueryEdits error propagation
//   - TokenService.CreateToken encrypt failure
//   - TokenService.FindActiveToken repo error propagation
//   - TokenService.FindActiveToken decrypt error
//   - HeartbeatService.RecordHeartbeat FindByDeviceID error
//   - HeartbeatService.RecordHeartbeat Upsert error
//   - StatsService.GetDevices ListAll error

import (
	"errors"
	"testing"
	"time"

	"github.com/aitrack/server/internal/application"
	"github.com/aitrack/server/internal/domain/model"
	"github.com/aitrack/server/internal/domain/port"
	"github.com/aitrack/server/internal/domain/service"
	"github.com/aitrack/server/internal/testkit"
)

// ─── Stub implementations of port interfaces ─────────────────────────────────

// errEditRepo is an EditRecordPort that always fails Save.
type errEditRepo struct {
	saveErr    error
	queryErr   error
	countValue int64
}

func (r *errEditRepo) Save(_ *model.EditRecord) error { return r.saveErr }
func (r *errEditRepo) CountByTokenKeyAndFilePathSince(_, _ string, _ time.Time) (int64, error) {
	return r.countValue, nil
}
func (r *errEditRepo) Query(_, _ string, _, _ int) ([]model.EditRecord, int64, error) {
	return nil, 0, r.queryErr
}
func (r *errEditRepo) AggregateByTokenKey() ([]model.StatsRow, error) { return nil, nil }
func (r *errEditRepo) AggregateByRepo() ([]model.StatsRow, error)     { return nil, nil }
func (r *errEditRepo) AggregateByDevice() ([]model.StatsRow, error)   { return nil, nil }
func (r *errEditRepo) AggregateByHostname() ([]model.StatsRow, error) { return nil, nil }
func (r *errEditRepo) AggregateByTool() ([]model.StatsRow, error)     { return nil, nil }

// errTokenRepo is a TokenPort that always returns errors.
type errTokenRepo struct {
	saveErr error
	findErr error
	token   *model.Token
}

func (r *errTokenRepo) Save(_ *model.Token) error { return r.saveErr }
func (r *errTokenRepo) FindActiveByHash(_ string) (*model.Token, error) {
	return r.token, r.findErr
}

// errDeviceRepo is a DevicePort with configurable errors.
type errDeviceRepo struct {
	findErr   error
	upsertErr error
	listErr   error
	existing  *model.Device
}

func (r *errDeviceRepo) FindByDeviceID(_ string) (*model.Device, error) {
	return r.existing, r.findErr
}
func (r *errDeviceRepo) Upsert(_ *model.Device) error { return r.upsertErr }
func (r *errDeviceRepo) ListAll() ([]*model.Device, error) {
	return nil, r.listErr
}

// ─── Helper to build an IngestService with a custom repo ─────────────────────

func newIngestSvcWithRepo(editRepo port.EditRecordPort) *application.IngestService {
	policy := service.ValidationPolicy{RateLimitPerHour: 30, MaxAddedLines: 5000}
	sig := service.NewSignatureService()
	diff := service.NewDiffConsistencyService()
	counter := editRepo.(port.EditRecordCounter)
	validation := service.NewValidationService(sig, diff, counter, policy)
	validator := service.NewEditValidator()
	return application.NewIngestService(validation, validator, editRepo)
}

// ─── IngestService Save error paths ──────────────────────────────────────────

func TestIngest_SaveError_AcceptedBecomesRejected(t *testing.T) {
	saveErr := errors.New("db write failure")
	repo := &errEditRepo{saveErr: saveErr}
	ingest := newIngestSvcWithRepo(repo)

	token := testkit.BuildTokenWithSig()
	req := testkit.BuildUploadRequest(token, testkit.BuildEditDTO())

	resp := ingest.Ingest(token, req)
	// The record passed validation but Save failed → should be rejected with save_error
	if resp.Accepted != 0 {
		t.Errorf("expected 0 accepted on save error, got %d", resp.Accepted)
	}
	if len(resp.Rejected) != 1 {
		t.Fatalf("expected 1 rejected on save error, got %v", resp.Rejected)
	}
	if !containsSubstring(resp.Rejected[0].Reason, "save_error") {
		t.Errorf("expected 'save_error' in reason, got %q", resp.Rejected[0].Reason)
	}
}

func TestIngest_SaveError_FlaggedBecomesRejected(t *testing.T) {
	saveErr := errors.New("db write failure")
	repo := &errEditRepo{saveErr: saveErr}
	ingest := newIngestSvcWithRepo(repo)

	token := testkit.BuildTokenWithSig()
	// OversizedEditDTO triggers FLAGGED
	req := testkit.BuildUploadRequest(token, testkit.OversizedEditDTO())

	resp := ingest.Ingest(token, req)
	if len(resp.Flagged) != 0 {
		t.Errorf("expected 0 flagged on save error, got %v", resp.Flagged)
	}
	if len(resp.Rejected) != 1 {
		t.Fatalf("expected 1 rejected on flagged save error, got %v", resp.Rejected)
	}
	if !containsSubstring(resp.Rejected[0].Reason, "save_error") {
		t.Errorf("expected 'save_error' in reason, got %q", resp.Rejected[0].Reason)
	}
}

func TestIngest_QueryEdits_Error(t *testing.T) {
	queryErr := errors.New("query db failure")
	repo := &errEditRepo{queryErr: queryErr}
	ingest := newIngestSvcWithRepo(repo)

	_, err := ingest.QueryEdits("", "", 0, 10)
	if err == nil {
		t.Error("expected error from QueryEdits when repo fails")
	}
}

// ─── TokenService error paths ─────────────────────────────────────────────────

func TestTokenService_CreateToken_EncryptError(t *testing.T) {
	// Use an encryptor with a deliberately bad key length to force encrypt failure
	// We can't force NewHmacSecretEncryptor to return an error-producing encryptor,
	// so we test through token repo save error instead (covers CreateToken error branch)
	saveErr := errors.New("repo save error")
	repo := &errTokenRepo{saveErr: saveErr}
	enc, _ := service.NewHmacSecretEncryptor("")
	sig := service.NewSignatureService()
	svc := application.NewTokenService(repo, sig, enc)

	_, err := svc.CreateToken(&model.CreateTokenRequest{Owner: "test"})
	if err == nil {
		t.Error("expected error when token repo.Save fails")
	}
}

func TestTokenService_FindActiveToken_RepoError(t *testing.T) {
	findErr := errors.New("db lookup error")
	repo := &errTokenRepo{findErr: findErr}
	enc, _ := service.NewHmacSecretEncryptor("")
	sig := service.NewSignatureService()
	svc := application.NewTokenService(repo, sig, enc)

	_, err := svc.FindActiveToken("aitrack_sometoken1234567890")
	if err == nil {
		t.Error("expected error when repo.FindActiveByHash fails")
	}
}

func TestTokenService_FindActiveToken_DecryptError(t *testing.T) {
	// Return a token with a non-plain ciphertext but no key configured
	// The decryptor (dev mode, no key) will fail on an encrypted value
	storedToken := &model.Token{
		ID:         1,
		TokenHash:  "fakehash",
		TokenKey:   "abcdef…7890",
		HmacSecret: "SomeBase64EncryptedValue=", // looks encrypted, not "plain:" prefixed
		Owner:      "test",
		Active:     true,
		CreatedAt:  time.Now().UTC(),
	}
	repo := &errTokenRepo{token: storedToken}
	// Dev mode encryptor (no key) — cannot decrypt non-plain: values
	enc, _ := service.NewHmacSecretEncryptor("")
	sig := service.NewSignatureService()
	svc := application.NewTokenService(repo, sig, enc)

	_, err := svc.FindActiveToken("aitrack_sometoken1234567890")
	if err == nil {
		t.Error("expected error when decryption fails")
	}
}

// ─── HeartbeatService error paths ────────────────────────────────────────────

func TestHeartbeat_FindByDeviceID_Error(t *testing.T) {
	findErr := errors.New("db lookup error")
	repo := &errDeviceRepo{findErr: findErr}
	svc := application.NewHeartbeatService(repo)

	token := testkit.BuildTokenWithSig()
	req := testkit.BuildHeartbeatRequest()

	err := svc.RecordHeartbeat(token, req)
	if err == nil {
		t.Error("expected error when FindByDeviceID fails")
	}
}

func TestHeartbeat_Upsert_Error(t *testing.T) {
	upsertErr := errors.New("db upsert error")
	repo := &errDeviceRepo{upsertErr: upsertErr}
	svc := application.NewHeartbeatService(repo)

	token := testkit.BuildTokenWithSig()
	req := testkit.BuildHeartbeatRequest()

	err := svc.RecordHeartbeat(token, req)
	if err == nil {
		t.Error("expected error when Upsert fails")
	}
}

func TestHeartbeat_ExistingDevice_NilHooksPreserved(t *testing.T) {
	// Existing device has hooks; new heartbeat has nil hooks → preserves old hooks
	existingHooks := `{"claude":true}`
	existing := &model.Device{
		DeviceID:  "device-001",
		TokenKey:  "abcdef…7890",
		HooksJSON: existingHooks,
		CreatedAt: time.Now().Add(-24 * time.Hour).UTC(),
	}
	var upserted *model.Device
	repo := &captureDeviceRepo{existing: existing, onUpsert: func(d *model.Device) {
		upserted = d
	}}
	svc := application.NewHeartbeatService(repo)

	token := testkit.BuildTokenWithSig()
	req := testkit.BuildHeartbeatRequest(func(r *testkit.HeartbeatReq) {
		r.Hooks = nil // no new hooks
	})

	err := svc.RecordHeartbeat(token, req)
	if err != nil {
		t.Fatal(err)
	}
	if upserted == nil {
		t.Fatal("expected Upsert to be called")
	}
	if upserted.HooksJSON != existingHooks {
		t.Errorf("expected hooks preserved as %q, got %q", existingHooks, upserted.HooksJSON)
	}
	// CreatedAt should be preserved from existing device
	if !upserted.CreatedAt.Equal(existing.CreatedAt) {
		t.Errorf("CreatedAt not preserved: got %v, want %v", upserted.CreatedAt, existing.CreatedAt)
	}
}

// ─── StatsService error paths ─────────────────────────────────────────────────

func TestStatsService_GetDevices_ListAllError(t *testing.T) {
	listErr := errors.New("db list error")
	deviceRepo := &errDeviceRepo{listErr: listErr}
	editRepo := &errEditRepo{}
	svc := application.NewStatsService(editRepo, deviceRepo)

	_, err := svc.GetDevices()
	if err == nil {
		t.Error("expected error when ListAll fails")
	}
}

func TestStatsService_GetStats_RepoError(t *testing.T) {
	aggErr := errors.New("aggregate error")
	editRepo := &errAggEditRepo{aggErr: aggErr}
	deviceRepo := &errDeviceRepo{}
	svc := application.NewStatsService(editRepo, deviceRepo)

	for _, groupBy := range []string{"token", "repo", "device", "hostname", "tool"} {
		_, err := svc.GetStats(groupBy)
		if err == nil {
			t.Errorf("GetStats(%q): expected error when aggregate fails", groupBy)
		}
	}
}

// ─── Capture helper for heartbeat test ───────────────────────────────────────

type captureDeviceRepo struct {
	existing *model.Device
	onUpsert func(*model.Device)
}

func (r *captureDeviceRepo) FindByDeviceID(_ string) (*model.Device, error) {
	return r.existing, nil
}
func (r *captureDeviceRepo) Upsert(d *model.Device) error {
	if r.onUpsert != nil {
		r.onUpsert(d)
	}
	return nil
}
func (r *captureDeviceRepo) ListAll() ([]*model.Device, error) { return nil, nil }

// ─── errAggEditRepo returns errors from aggregate methods ────────────────────

type errAggEditRepo struct {
	aggErr error
}

func (r *errAggEditRepo) Save(_ *model.EditRecord) error { return nil }
func (r *errAggEditRepo) CountByTokenKeyAndFilePathSince(_, _ string, _ time.Time) (int64, error) {
	return 0, nil
}
func (r *errAggEditRepo) Query(_, _ string, _, _ int) ([]model.EditRecord, int64, error) {
	return nil, 0, nil
}
func (r *errAggEditRepo) AggregateByTokenKey() ([]model.StatsRow, error) { return nil, r.aggErr }
func (r *errAggEditRepo) AggregateByRepo() ([]model.StatsRow, error)     { return nil, r.aggErr }
func (r *errAggEditRepo) AggregateByDevice() ([]model.StatsRow, error)   { return nil, r.aggErr }
func (r *errAggEditRepo) AggregateByHostname() ([]model.StatsRow, error) { return nil, r.aggErr }
func (r *errAggEditRepo) AggregateByTool() ([]model.StatsRow, error)     { return nil, r.aggErr }

// ─── string helpers ───────────────────────────────────────────────────────────

func containsSubstring(s, sub string) bool {
	return len(s) >= len(sub) && (s == sub || len(sub) == 0 ||
		func() bool {
			for i := 0; i <= len(s)-len(sub); i++ {
				if s[i:i+len(sub)] == sub {
					return true
				}
			}
			return false
		}())
}
