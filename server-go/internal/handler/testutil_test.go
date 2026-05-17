package handler_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strconv"
	"testing"
	"time"

	"github.com/aitrack/server/internal/config"
	"github.com/aitrack/server/internal/db"
	"github.com/aitrack/server/internal/handler"
	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
	"github.com/aitrack/server/internal/testkit"
)

// testEnv holds a fully wired server for handler tests.
type testEnv struct {
	router     http.Handler
	tokenSvc   *service.TokenService
	sig        *service.SignatureService
	cfg        *config.Config
	rawToken   string
	hmacSecret string
}

func newTestEnv(t *testing.T) *testEnv {
	t.Helper()
	database, err := db.Open(":memory:")
	if err != nil {
		t.Fatalf("open test db: %v", err)
	}
	t.Cleanup(func() { database.Close() })

	cfg := &config.Config{}
	cfg.Server.Port = 8080
	cfg.TimestampWindowSeconds = 300
	cfg.RateLimitPerHour = 30
	cfg.MaxAddedLines = 5000
	cfg.AdminKey = "test-admin-key"

	enc, _ := service.NewHmacSecretEncryptor("")
	sig := service.NewSignatureService()
	diff := service.NewDiffConsistencyService()
	ev := service.NewEditValidator()

	tokenRepo := service.NewTokenRepository(database)
	editRepo := service.NewEditRecordRepository(database)
	deviceRepo := service.NewDeviceRepository(database)

	tokenSvc := service.NewTokenService(tokenRepo, sig, enc)
	validationSvc := service.NewValidationService(sig, diff, editRepo, cfg)
	ingestSvc := service.NewIngestService(validationSvc, ev, editRepo)
	heartbeatSvc := service.NewHeartbeatService(deviceRepo)
	statsSvc := service.NewStatsService(editRepo, deviceRepo)

	auth := handler.NewAuthMiddleware(tokenSvc, sig, cfg)
	adminH := handler.NewAdminHandler(tokenSvc, cfg)
	editsH := handler.NewEditsHandler(auth, ingestSvc)
	hbH := handler.NewHeartbeatHandler(auth, heartbeatSvc)
	statsH := handler.NewStatsHandler(auth, statsSvc)
	router := handler.NewRouter(adminH, editsH, hbH, statsH)

	// Pre-create a token for API tests
	resp, err := tokenSvc.CreateToken(&model.CreateTokenRequest{Owner: "tester"})
	if err != nil {
		t.Fatalf("create test token: %v", err)
	}

	return &testEnv{
		router:     router,
		tokenSvc:   tokenSvc,
		sig:        sig,
		cfg:        cfg,
		rawToken:   resp.Token,
		hmacSecret: resp.HmacSecret,
	}
}

// signedRequest builds a request with all required AiTrack headers.
func (e *testEnv) signedRequest(method, path string, body []byte) *http.Request {
	var bodyBuf *bytes.Buffer
	if body != nil {
		bodyBuf = bytes.NewBuffer(body)
	} else {
		bodyBuf = bytes.NewBuffer([]byte("{}"))
		body = []byte("{}")
	}
	req := httptest.NewRequest(method, path, bodyBuf)
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+e.rawToken)

	ts := strconv.FormatInt(time.Now().Unix(), 10)
	sig := e.sig.ComputeRequestSignature(e.hmacSecret, ts, body)
	req.Header.Set("X-AiTrack-Timestamp", ts)
	req.Header.Set("X-AiTrack-Signature", sig)
	return req
}

// adminRequest builds a request with X-Admin-Key header.
func (e *testEnv) adminRequest(method, path string, body interface{}) *http.Request {
	b, _ := json.Marshal(body)
	req := httptest.NewRequest(method, path, bytes.NewReader(b))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("X-Admin-Key", e.cfg.AdminKey)
	return req
}

func do(router http.Handler, req *http.Request) *httptest.ResponseRecorder {
	w := httptest.NewRecorder()
	router.ServeHTTP(w, req)
	return w
}

// buildSignedEditBatch builds a valid batch request body bytes,
// signed with the given hmac secret and token key.
func (e *testEnv) buildEditBatch(tokenKey string, edits ...*model.EditDTO) []byte {
	if len(edits) == 0 {
		p := testkit.DefaultEditParams()
		p.HmacSecret = e.hmacSecret
		p.TokenKey = tokenKey
		edits = []*model.EditDTO{testkit.BuildEditDTO(func(ep *testkit.EditParams) {
			*ep = p
		})}
	}
	dtos := make([]model.EditDTO, len(edits))
	for i, ed := range edits {
		dtos[i] = *ed
	}
	req := model.EditBatchRequest{
		DeviceID:      "device-001",
		ClientVersion: "1.0.0",
		Edits:         dtos,
	}
	b, _ := json.Marshal(req)
	return b
}

// resolveTokenKey fetches the token_key for the env's raw token.
func (e *testEnv) resolveTokenKey(t *testing.T) string {
	t.Helper()
	tok, err := e.tokenSvc.FindActiveToken(e.rawToken)
	if err != nil || tok == nil {
		t.Fatal("could not resolve token")
	}
	return tok.TokenKey
}

// buildValidEditDTO builds an EditDTO with correct sig for this env's token.
func (e *testEnv) buildValidEditDTO(t *testing.T) *model.EditDTO {
	t.Helper()
	tokenKey := e.resolveTokenKey(t)
	p := testkit.DefaultEditParams()
	p.HmacSecret = e.hmacSecret
	p.TokenKey = tokenKey
	return testkit.BuildEditDTO(func(ep *testkit.EditParams) { *ep = p })
}

func decodeJSON(t *testing.T, w *httptest.ResponseRecorder, v interface{}) {
	t.Helper()
	if err := json.NewDecoder(w.Body).Decode(v); err != nil {
		t.Fatalf("decode response: %v\nbody: %s", err, w.Body.String())
	}
}

func assertStatus(t *testing.T, w *httptest.ResponseRecorder, want int) {
	t.Helper()
	if w.Code != want {
		t.Errorf("status = %d, want %d\nbody: %s", w.Code, want, w.Body.String())
	}
}

// buildSignedBatch builds body bytes and a signed request in one step.
func (e *testEnv) signedEditRequest(t *testing.T) (*http.Request, []byte) {
	t.Helper()
	edit := e.buildValidEditDTO(t)
	body := e.buildEditBatch(e.resolveTokenKey(t), edit)
	req := e.signedRequest(http.MethodPost, "/api/v1/ai-track/edits", body)
	return req, body
}

// timestamp returns current unix seconds as string.
func nowTS() string {
	return fmt.Sprintf("%d", time.Now().Unix())
}
