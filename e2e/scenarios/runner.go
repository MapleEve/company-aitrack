// E2E scenario runner for aitrack.
// Reads SERVER_URL and ADMIN_KEY from env, then runs all contract scenarios.
package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/aitrack/e2e/factory"
)

// ─── HTTP helpers ─────────────────────────────────────────────────────────────

var httpClient = &http.Client{Timeout: 15 * time.Second}

func doRequest(method, url string, headers map[string]string, body []byte) (int, []byte, error) {
	var bodyReader io.Reader
	if body != nil {
		bodyReader = bytes.NewReader(body)
	}
	req, err := http.NewRequest(method, url, bodyReader)
	if err != nil {
		return 0, nil, err
	}
	for k, v := range headers {
		req.Header.Set(k, v)
	}
	if body != nil {
		req.Header.Set("Content-Type", "application/json")
	}
	resp, err := httpClient.Do(req)
	if err != nil {
		return 0, nil, err
	}
	defer resp.Body.Close()
	b, _ := io.ReadAll(resp.Body)
	return resp.StatusCode, b, nil
}

func signedHeaders(tok factory.TokenBundle, bodyBytes []byte) map[string]string {
	ts := time.Now().Unix()
	sig := factory.ComputeRequestSig(tok.HmacSecret, ts, bodyBytes)
	return map[string]string{
		"Authorization":        "Bearer " + tok.Token,
		"X-AiTrack-Device":    factory.SeedUUIDExport(1),
		"X-AiTrack-Client":    "aitrack/1.0.0",
		"X-AiTrack-Timestamp": fmt.Sprintf("%d", ts),
		"X-AiTrack-Signature": sig,
	}
}

// ─── Assertion helpers ────────────────────────────────────────────────────────

type testResult struct {
	name   string
	passed bool
	detail string
}

var allResults []testResult

func assert(name string, cond bool, detail string) {
	if cond {
		fmt.Printf("  PASS  %s\n", name)
	} else {
		fmt.Printf("  FAIL  %s — %s\n", name, detail)
	}
	allResults = append(allResults, testResult{name, cond, detail})
}

func assertStatus(name string, got, want int) {
	assert(name, got == want, fmt.Sprintf("got %d, want %d", got, want))
}

func parseJSON(b []byte) map[string]interface{} {
	var v map[string]interface{}
	json.Unmarshal(b, &v)
	return v
}

// ─── Token provisioning ───────────────────────────────────────────────────────

func provisionToken(serverURL, adminKey, owner string) (factory.TokenBundle, error) {
	body, _ := json.Marshal(map[string]string{"owner": owner, "note": "e2e-" + owner})
	code, resp, err := doRequest("POST", serverURL+"/admin/tokens",
		map[string]string{"X-Admin-Key": adminKey}, body)
	if err != nil {
		return factory.TokenBundle{}, err
	}
	if code != 200 {
		return factory.TokenBundle{}, fmt.Errorf("provision token: status %d body=%s", code, resp)
	}
	var tok factory.TokenBundle
	if err := json.Unmarshal(resp, &tok); err != nil {
		return factory.TokenBundle{}, fmt.Errorf("decode token: %w body=%s", err, resp)
	}
	return tok, nil
}

// ─── Scenarios ────────────────────────────────────────────────────────────────

func scenarioAdminTokenAuth(serverURL, adminKey string) {
	fmt.Println("\n=== Scenario: Admin token auth ===")

	// 403 with wrong admin key
	body, _ := json.Marshal(map[string]string{"owner": "test"})
	code, _, _ := doRequest("POST", serverURL+"/admin/tokens",
		map[string]string{"X-Admin-Key": "wrong-key"}, body)
	assertStatus("POST /admin/tokens wrong key → 403", code, 403)

	// 400 missing owner (empty body)
	body2, _ := json.Marshal(map[string]string{})
	code2, _, _ := doRequest("POST", serverURL+"/admin/tokens",
		map[string]string{"X-Admin-Key": adminKey}, body2)
	assertStatus("POST /admin/tokens missing owner → 400", code2, 400)

	// 200 valid provision
	body3, _ := json.Marshal(map[string]string{"owner": "scenario-test"})
	code3, resp3, _ := doRequest("POST", serverURL+"/admin/tokens",
		map[string]string{"X-Admin-Key": adminKey}, body3)
	assertStatus("POST /admin/tokens valid → 200", code3, 200)
	m := parseJSON(resp3)
	assert("response has token field", m["token"] != nil, fmt.Sprintf("resp=%s", resp3))
	assert("response has hmac_secret field", m["hmac_secret"] != nil, fmt.Sprintf("resp=%s", resp3))
	assert("response has token_key field", m["token_key"] != nil, fmt.Sprintf("resp=%s", resp3))
}

func scenarioContractValidation(serverURL string, tok factory.TokenBundle) {
	fmt.Println("\n=== Scenario: Contract validation (steps 1-3) ===")

	p := factory.DefaultEditParams(42, tok)
	body := factory.BuildBatchRequest(p.DeviceID, p.BuildDTO())

	// Step 1: no auth → 401
	code, _, _ := doRequest("POST", serverURL+"/api/v1/ai-track/edits", nil, body)
	assertStatus("No auth → 401", code, 401)

	// Step 1: wrong token → 401
	ts := time.Now().Unix()
	wrongSig := factory.ComputeRequestSig("wrong-secret", ts, body)
	badAuthHeaders := map[string]string{
		"Authorization":        "Bearer wrong-token-value",
		"X-AiTrack-Device":    p.DeviceID,
		"X-AiTrack-Client":    "aitrack/1.0.0",
		"X-AiTrack-Timestamp": fmt.Sprintf("%d", ts),
		"X-AiTrack-Signature": wrongSig,
	}
	code2, _, _ := doRequest("POST", serverURL+"/api/v1/ai-track/edits", badAuthHeaders, body)
	assertStatus("Wrong Bearer token → 401", code2, 401)

	// Step 2: expired timestamp → 401
	tsExpired := time.Now().Add(-10 * time.Minute).Unix()
	expiredSig := factory.ComputeRequestSig(tok.HmacSecret, tsExpired, body)
	expiredHeaders := map[string]string{
		"Authorization":        "Bearer " + tok.Token,
		"X-AiTrack-Device":    p.DeviceID,
		"X-AiTrack-Client":    "aitrack/1.0.0",
		"X-AiTrack-Timestamp": fmt.Sprintf("%d", tsExpired),
		"X-AiTrack-Signature": expiredSig,
	}
	code3, _, _ := doRequest("POST", serverURL+"/api/v1/ai-track/edits", expiredHeaders, body)
	assertStatus("Expired timestamp → 401", code3, 401)

	// Step 3: bad signature (zeros) → 401
	tsNow := time.Now().Unix()
	badSigHeaders := map[string]string{
		"Authorization":        "Bearer " + tok.Token,
		"X-AiTrack-Device":    p.DeviceID,
		"X-AiTrack-Client":    "aitrack/1.0.0",
		"X-AiTrack-Timestamp": fmt.Sprintf("%d", tsNow),
		"X-AiTrack-Signature": strings.Repeat("0", 64),
	}
	code4, _, _ := doRequest("POST", serverURL+"/api/v1/ai-track/edits", badSigHeaders, body)
	assertStatus("Bad signature → 401", code4, 401)

	// Missing / empty edits array → 400
	emptyBody, _ := json.Marshal(map[string]interface{}{
		"device_id": p.DeviceID, "client_version": "1.0.0", "edits": []interface{}{},
	})
	h5 := signedHeaders(tok, emptyBody)
	code5, _, _ := doRequest("POST", serverURL+"/api/v1/ai-track/edits", h5, emptyBody)
	assertStatus("Empty edits array → 400", code5, 400)
}

func scenarioHappyPath(serverURL string, tok factory.TokenBundle) {
	fmt.Println("\n=== Scenario: Full happy path (capture → upload → stats → devices) ===")

	p := factory.DefaultEditParams(1001, tok)
	dto := p.BuildDTO()
	body := factory.BuildBatchRequest(p.DeviceID, dto)
	headers := signedHeaders(tok, body)

	code, resp, err := doRequest("POST", serverURL+"/api/v1/ai-track/edits", headers, body)
	assert("POST /edits no error", err == nil, fmt.Sprintf("%v", err))
	assertStatus("POST /edits happy path → 200", code, 200)

	m := parseJSON(resp)
	accepted, _ := m["accepted"].(float64)
	assert("accepted == 1", int(accepted) == 1, fmt.Sprintf("resp=%s", resp))

	// GET /edits (paginated query) — assert snake_case shape and hostname field
	getHeaders := map[string]string{"Authorization": "Bearer " + tok.Token}
	code2, resp2, _ := doRequest("GET", serverURL+"/api/v1/ai-track/edits?page=0&size=10", getHeaders, nil)
	assertStatus("GET /edits → 200", code2, 200)
	assert("GET /edits returns content", len(resp2) > 2, fmt.Sprintf("resp=%s", resp2))
	editsMap := parseJSON(resp2)
	assert("GET /edits has total field", editsMap["total"] != nil, fmt.Sprintf("resp=%s", resp2))
	assert("GET /edits has page field", editsMap["page"] != nil, fmt.Sprintf("resp=%s", resp2))
	assert("GET /edits has size field", editsMap["size"] != nil, fmt.Sprintf("resp=%s", resp2))
	assert("GET /edits has records field", editsMap["records"] != nil, fmt.Sprintf("resp=%s", resp2))
	if records, ok := editsMap["records"].([]interface{}); ok && len(records) > 0 {
		firstRecord, _ := records[0].(map[string]interface{})
		hostnameVal, hasHostname := firstRecord["hostname"]
		assert("GET /edits record has hostname field", hasHostname && hostnameVal != nil, fmt.Sprintf("record=%v", firstRecord))
		assert("GET /edits record hostname matches uploaded value", hostnameVal == dto["hostname"], fmt.Sprintf("got=%v want=%v", hostnameVal, dto["hostname"]))
		assert("GET /edits record has file_path field", firstRecord["file_path"] != nil, fmt.Sprintf("record=%v", firstRecord))
		assert("GET /edits record has added_lines field", firstRecord["added_lines"] != nil, fmt.Sprintf("record=%v", firstRecord))
		assert("GET /edits record has token_key field", firstRecord["token_key"] != nil, fmt.Sprintf("record=%v", firstRecord))
		assert("GET /edits record has received_at field", firstRecord["received_at"] != nil, fmt.Sprintf("record=%v", firstRecord))
	}

	// GET /stats
	code3, resp3, _ := doRequest("GET", serverURL+"/api/v1/ai-track/stats", getHeaders, nil)
	assertStatus("GET /stats → 200", code3, 200)
	assert("GET /stats returns content", len(resp3) > 2, fmt.Sprintf("resp=%s", resp3))

	// GET /devices
	code4, resp4, _ := doRequest("GET", serverURL+"/api/v1/ai-track/devices", getHeaders, nil)
	assertStatus("GET /devices → 200", code4, 200)
	assert("GET /devices returns content", len(resp4) > 1, fmt.Sprintf("resp=%s", resp4))
}

func scenarioAnticheat(serverURL string, tok factory.TokenBundle) {
	fmt.Println("\n=== Scenario: Anti-cheat validation ===")

	// Tampered record_sig → rejected
	p := factory.DefaultEditParams(2001, tok)
	tampered := factory.TamperedRecordSig(p)
	body := factory.BuildBatchRequest(p.DeviceID, tampered)
	headers := signedHeaders(tok, body)
	code, resp, _ := doRequest("POST", serverURL+"/api/v1/ai-track/edits", headers, body)
	assertStatus("Tampered record_sig → 200", code, 200)
	m := parseJSON(resp)
	rejected, _ := m["rejected"].([]interface{})
	assert("tampered sig in rejected list", len(rejected) > 0, fmt.Sprintf("resp=%s", resp))

	// Oversized lines → flagged
	p2 := factory.DefaultEditParams(2002, tok)
	oversized := factory.OversizedEdit(p2)
	body2 := factory.BuildBatchRequest(p2.DeviceID, oversized)
	headers2 := signedHeaders(tok, body2)
	code2, resp2, _ := doRequest("POST", serverURL+"/api/v1/ai-track/edits", headers2, body2)
	assertStatus("Oversized edit → 200", code2, 200)
	m2 := parseJSON(resp2)
	flagged, _ := m2["flagged"].([]interface{})
	assert("oversized in flagged list", len(flagged) > 0, fmt.Sprintf("resp=%s", resp2))

	// Missing required field → malformed rejected
	p3 := factory.DefaultEditParams(2003, tok)
	missing := factory.MissingFieldEdit(p3)
	body3 := factory.BuildBatchRequest(p3.DeviceID, missing)
	headers3 := signedHeaders(tok, body3)
	code3, resp3, _ := doRequest("POST", serverURL+"/api/v1/ai-track/edits", headers3, body3)
	assertStatus("Missing field edit → 200", code3, 200)
	m3 := parseJSON(resp3)
	rejected3, _ := m3["rejected"].([]interface{})
	assert("missing field in rejected list", len(rejected3) > 0, fmt.Sprintf("resp=%s", resp3))
}

func scenarioHeartbeat(serverURL string, tok factory.TokenBundle) {
	fmt.Println("\n=== Scenario: Heartbeat ===")

	deviceID := factory.SeedUUIDExport(999)
	body := factory.BuildHeartbeatRequest(deviceID, "e2e-heartbeat-host", tok.TokenKey, 3)
	headers := signedHeaders(tok, body)

	code, resp, _ := doRequest("POST", serverURL+"/api/v1/ai-track/heartbeat", headers, body)
	assertStatus("POST /heartbeat → 200", code, 200)
	m := parseJSON(resp)
	assert("heartbeat response has ok=true", m["ok"] == true, fmt.Sprintf("resp=%s", resp))

	// Devices endpoint should reflect the heartbeat
	getHeaders := map[string]string{"Authorization": "Bearer " + tok.Token}
	code2, resp2, _ := doRequest("GET", serverURL+"/api/v1/ai-track/devices", getHeaders, nil)
	assertStatus("GET /devices after heartbeat → 200", code2, 200)
	assert("devices response non-empty", len(resp2) > 2, fmt.Sprintf("resp=%s", resp2))
}

func scenarioStatsByHostname(serverURL string, tok factory.TokenBundle) {
	fmt.Println("\n=== Scenario: Stats group_by=hostname ===")

	// Upload an edit so there is data to group
	p := factory.DefaultEditParams(4001, tok)
	dto := p.BuildDTO()
	body := factory.BuildBatchRequest(p.DeviceID, dto)
	headers := signedHeaders(tok, body)
	doRequest("POST", serverURL+"/api/v1/ai-track/edits", headers, body)

	getHeaders := map[string]string{"Authorization": "Bearer " + tok.Token}
	code, resp, _ := doRequest("GET", serverURL+"/api/v1/ai-track/stats?group_by=hostname", getHeaders, nil)
	assertStatus("GET /stats?group_by=hostname → 200", code, 200)
	assert("GET /stats?group_by=hostname returns content", len(resp) > 2, fmt.Sprintf("resp=%s", resp))

	// Response is an array of StatsRow; the grouped value (the hostname) is in
	// the "group" field — consistent with group_by=token/repo/device.
	var statsList []map[string]interface{}
	if err := json.Unmarshal(resp, &statsList); err == nil && len(statsList) > 0 {
		allHaveGroup := true
		foundUploaded := false
		for _, item := range statsList {
			g, ok := item["group"]
			if !ok || g == nil {
				allHaveGroup = false
			}
			if g == p.Hostname {
				foundUploaded = true
			}
		}
		assert("stats grouped by hostname: every row has group field", allHaveGroup, fmt.Sprintf("rows=%v", statsList))
		assert("stats grouped by hostname includes uploaded hostname", foundUploaded, fmt.Sprintf("want group=%v rows=%v", p.Hostname, statsList))
	}
}

func scenarioRepoWhitelist(serverURL string, tok factory.TokenBundle) {
	fmt.Println("\n=== Scenario: Repo whitelist (enforce=false) ===")

	p := factory.DefaultEditParams(3001, tok)
	p.RepoURL = "git@github.com:unknown-org/unknown-repo.git"
	dto := p.BuildDTO()
	body := factory.BuildBatchRequest(p.DeviceID, dto)
	headers := signedHeaders(tok, body)

	code, resp, _ := doRequest("POST", serverURL+"/api/v1/ai-track/edits", headers, body)
	assertStatus("Unknown repo enforce=false → 200", code, 200)
	m := parseJSON(resp)
	accepted, _ := m["accepted"].(float64)
	flagged, _ := m["flagged"].([]interface{})
	assert("unknown repo accepted or flagged (not hard-rejected)",
		int(accepted) > 0 || len(flagged) > 0,
		fmt.Sprintf("resp=%s", resp))
}

// ─── Main ─────────────────────────────────────────────────────────────────────

func main() {
	serverURL := os.Getenv("SERVER_URL")
	if serverURL == "" {
		serverURL = "http://localhost:8080"
	}
	adminKey := os.Getenv("ADMIN_KEY")
	if adminKey == "" {
		adminKey = "e2e-admin-key"
	}
	serverImpl := os.Getenv("SERVER_IMPL")
	if serverImpl == "" {
		serverImpl = "unknown"
	}

	fmt.Printf("\naitrack E2E — impl=%s url=%s\n", serverImpl, serverURL)
	fmt.Println(strings.Repeat("=", 60))

	// Wait for server (max 30s)
	fmt.Print("Waiting for server...")
	ready := false
	for i := 0; i < 30; i++ {
		code, _, _ := doRequest("GET", serverURL+"/api/v1/ai-track/stats",
			map[string]string{"Authorization": "Bearer dummy"}, nil)
		if code != 0 {
			fmt.Println(" ready")
			ready = true
			break
		}
		fmt.Print(".")
		time.Sleep(1 * time.Second)
	}
	if !ready {
		fmt.Println("\nServer did not become ready in 30s")
		os.Exit(1)
	}

	tok1, err := provisionToken(serverURL, adminKey, "e2e-user-1")
	if err != nil {
		fmt.Fprintf(os.Stderr, "FATAL: provision token 1: %v\n", err)
		os.Exit(1)
	}
	tok2, err := provisionToken(serverURL, adminKey, "e2e-user-2")
	if err != nil {
		fmt.Fprintf(os.Stderr, "FATAL: provision token 2: %v\n", err)
		os.Exit(1)
	}

	scenarioAdminTokenAuth(serverURL, adminKey)
	scenarioContractValidation(serverURL, tok1)
	scenarioHappyPath(serverURL, tok1)
	scenarioAnticheat(serverURL, tok2)
	scenarioHeartbeat(serverURL, tok1)
	scenarioRepoWhitelist(serverURL, tok2)
	scenarioStatsByHostname(serverURL, tok1)

	fmt.Printf("\n%s\n", strings.Repeat("=", 60))
	passed, failed := 0, 0
	for _, r := range allResults {
		if r.passed {
			passed++
		} else {
			failed++
		}
	}
	fmt.Printf("Results [%s]: %d passed, %d failed\n", serverImpl, passed, failed)
	if failed > 0 {
		fmt.Println("E2E FAILED")
		os.Exit(1)
	}
	fmt.Println("E2E PASSED")
}
