package handler_test

import (
	"bytes"
	"database/sql"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"strconv"
	"strings"
	"testing"
	"time"

	dbadapter "github.com/aitrack/server/internal/adapter/db"
	"github.com/aitrack/server/internal/adapter/handler"
	"github.com/aitrack/server/internal/application"
	"github.com/aitrack/server/internal/domain/model"
	"github.com/aitrack/server/internal/domain/service"
	"github.com/aitrack/server/internal/infrastructure/config"
	dbpkg "github.com/aitrack/server/internal/infrastructure/db"
	"github.com/aitrack/server/internal/testkit"
	_ "github.com/jackc/pgx/v5/stdlib"
)

func testDBURL() string {
	if u := os.Getenv("TEST_DATABASE_URL"); u != "" {
		return u
	}
	return "postgres://aitrack:aitrack_secret@localhost:5432/aitrack_test?sslmode=disable"
}

func TestMain(m *testing.M) {
	conn, err := sql.Open("pgx", testDBURL())
	if err != nil || conn.Ping() != nil {
		fmt.Println("SKIP: TEST_DATABASE_URL not reachable, skipping handler tests")
		os.Exit(0)
	}
	conn.Close()
	os.Exit(m.Run())
}

// testEnv holds a fully wired server for handler tests.
type testEnv struct {
	router     http.Handler
	db         *sql.DB
	tokenSvc   *application.TokenService
	sig        *service.SignatureService
	cfg        *config.Config
	rawToken   string
	hmacSecret string
}

func newTestEnv(t *testing.T) *testEnv {
	t.Helper()
	database, err := dbpkg.Open(testDBURL())
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

	tokenAdapter := dbadapter.NewTokenAdapter(database)
	editAdapter := dbadapter.NewEditRecordAdapter(database)
	deviceAdapter := dbadapter.NewDeviceAdapter(database)

	policy := service.ValidationPolicy{
		RateLimitPerHour: cfg.RateLimitPerHour,
		MaxAddedLines:    cfg.MaxAddedLines,
	}
	tokenSvc := application.NewTokenService(tokenAdapter, sig, enc)
	validationSvc := service.NewValidationService(sig, diff, editAdapter, policy)
	ingestSvc := application.NewIngestService(validationSvc, ev, editAdapter)
	heartbeatSvc := application.NewHeartbeatService(deviceAdapter)
	statsSvc := application.NewStatsService(editAdapter, deviceAdapter)

	auth := handler.NewAuthMiddleware(tokenSvc, sig, cfg)
	adminH := handler.NewAdminHandler(tokenSvc, cfg)
	editsH := handler.NewEditsHandler(auth, ingestSvc)
	hbH := handler.NewHeartbeatHandler(auth, heartbeatSvc)
	statsH := handler.NewStatsHandler(auth, statsSvc)
	searchH := handler.NewSearchHandler(database, cfg.AdminKey, true /* PostgreSQL */)
	similarH := handler.NewSimilarHandler(database, cfg.AdminKey, true /* PostgreSQL */)
	profileH := handler.NewProfileHandler(database, cfg.AdminKey)
	router := handler.NewRouter(adminH, editsH, hbH, statsH, searchH, similarH, profileH)

	// Pre-create a token for API tests
	resp, err := tokenSvc.CreateToken(&model.CreateTokenRequest{Owner: "tester"})
	if err != nil {
		t.Fatalf("create test token: %v", err)
	}
	credParts := strings.SplitN(resp.Credential, "-", 2)
	if len(credParts) != 2 {
		t.Fatalf("unexpected credential format: %q", resp.Credential)
	}

	return &testEnv{
		router:     router,
		db:         database,
		tokenSvc:   tokenSvc,
		sig:        sig,
		cfg:        cfg,
		rawToken:   credParts[0],
		hmacSecret: credParts[1],
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

// buildEditBatch builds a valid batch request body bytes,
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

// signedEditRequest builds body bytes and a signed request in one step.
func (e *testEnv) signedEditRequest(t *testing.T) (*http.Request, []byte) {
	t.Helper()
	edit := e.buildValidEditDTO(t)
	body := e.buildEditBatch(e.resolveTokenKey(t), edit)
	req := e.signedRequest(http.MethodPost, "/api/v1/ai-track/edits", body)
	return req, body
}

// nowTS returns current unix seconds as string.
func nowTS() string {
	return fmt.Sprintf("%d", time.Now().Unix())
}
