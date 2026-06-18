package handler

import (
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"net/http"
	"strconv"
	"strings"
	"time"

	"github.com/aitrack/server/internal/application"
	"github.com/aitrack/server/internal/domain/model"
)

// UsageHandler handles scalar usage ingestion and summary endpoints.
type UsageHandler struct {
	auth  *AuthMiddleware
	usage *application.UsageService
}

// NewUsageHandler constructs the usage handler adapter.
func NewUsageHandler(auth *AuthMiddleware, usage *application.UsageService) *UsageHandler {
	return &UsageHandler{auth: auth, usage: usage}
}

// SubmitRollups handles POST /api/v1/ai-track/usage/rollup.
func (h *UsageHandler) SubmitRollups(w http.ResponseWriter, r *http.Request) {
	rawBody, token, ok := h.readUsageBody(w, r)
	if !ok {
		return
	}

	var req model.UsageRollupRequest
	if err := json.Unmarshal(rawBody, &req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON")
		return
	}
	if len(req.Items) == 0 {
		writeError(w, http.StatusBadRequest, "items array is required and must not be empty")
		return
	}
	if len(req.Items) > 500 {
		writeError(w, http.StatusRequestEntityTooLarge, "items array exceeds maximum batch size of 500")
		return
	}
	for _, item := range req.Items {
		if strings.TrimSpace(item.DeviceID) == "" || strings.TrimSpace(item.Day) == "" || strings.TrimSpace(item.Agent) == "" || strings.TrimSpace(item.Model) == "" {
			writeError(w, http.StatusBadRequest, "device_id, day, agent, and model are required")
			return
		}
	}
	if err := h.usage.IngestRollups(token, &req); err != nil {
		writeError(w, http.StatusInternalServerError, "failed to store usage rollups")
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "accepted": len(req.Items)})
}

// SubmitSubscription handles POST /api/v1/ai-track/usage/subscription.
func (h *UsageHandler) SubmitSubscription(w http.ResponseWriter, r *http.Request) {
	rawBody, token, ok := h.readUsageBody(w, r)
	if !ok {
		return
	}

	var req model.UsageSubscriptionSnapshotRequest
	if err := json.Unmarshal(rawBody, &req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON")
		return
	}
	if strings.TrimSpace(req.DeviceID) == "" || strings.TrimSpace(req.Agent) == "" || strings.TrimSpace(req.ReaderStatus) == "" || strings.TrimSpace(req.SnapshottedAt) == "" {
		writeError(w, http.StatusBadRequest, "device_id, agent, reader_status, and snapshotted_at are required")
		return
	}
	if err := h.usage.IngestSubscription(token, &req); err != nil {
		writeError(w, http.StatusInternalServerError, "failed to store usage subscription snapshot")
		return
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}

// Summary handles GET /api/v1/ai-track/usage/summary.
func (h *UsageHandler) Summary(w http.ResponseWriter, r *http.Request) {
	token := h.auth.ResolveToken(w, r)
	if token == nil {
		return
	}
	q := r.URL.Query()
	tokenKey := q.Get("token_key")
	if tokenKey == "" {
		tokenKey = token.TokenKey
	}
	limit := parseUsageLimit(q.Get("limit"))
	summary, err := h.usage.Summary(tokenKey, q.Get("from_day"), q.Get("to_day"), q.Get("agent"), limit)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "query failed")
		return
	}
	writeJSON(w, http.StatusOK, summary)
}

func (h *UsageHandler) readUsageBody(w http.ResponseWriter, r *http.Request) ([]byte, *model.Token, bool) {
	rawBody, ok := ReadBody(w, r)
	if !ok {
		return nil, nil, false
	}
	if !validateUsageBodyDigest(w, r, rawBody) {
		return nil, nil, false
	}
	if !validateUsageBodyTimestamp(w, r) {
		return nil, nil, false
	}
	token := h.auth.ResolveToken(w, r)
	if token == nil {
		return nil, nil, false
	}
	if !h.auth.ValidateRequestSignature(w, r, token, rawBody) {
		return nil, nil, false
	}

	encoding := strings.TrimSpace(r.Header.Get("Content-Encoding"))
	switch {
	case encoding == "" || strings.EqualFold(encoding, "identity"):
		return rawBody, token, true
	case strings.EqualFold(encoding, "gzip"):
		reader, err := gzip.NewReader(bytes.NewReader(rawBody))
		if err != nil {
			writeError(w, http.StatusBadRequest, "invalid gzip body")
			return nil, nil, false
		}
		defer reader.Close()
		rawJSON, err := io.ReadAll(reader)
		if err != nil {
			writeError(w, http.StatusBadRequest, "invalid gzip body")
			return nil, nil, false
		}
		return rawJSON, token, true
	default:
		writeError(w, http.StatusBadRequest, "unsupported Content-Encoding")
		return nil, nil, false
	}
}

func validateUsageBodyDigest(w http.ResponseWriter, r *http.Request, rawBody []byte) bool {
	header := strings.TrimSpace(r.Header.Get("X-AiTrack-Body-Sha256"))
	if header == "" {
		writeError(w, http.StatusBadRequest, "missing X-AiTrack-Body-Sha256")
		return false
	}
	sum := sha256.Sum256(rawBody)
	expected := hex.EncodeToString(sum[:])
	if !strings.EqualFold(header, expected) {
		writeError(w, http.StatusBadRequest, "invalid X-AiTrack-Body-Sha256")
		return false
	}
	return true
}

func validateUsageBodyTimestamp(w http.ResponseWriter, r *http.Request) bool {
	header := strings.TrimSpace(r.Header.Get("X-AiTrack-Body-Timestamp"))
	if header == "" {
		writeError(w, http.StatusBadRequest, "missing X-AiTrack-Body-Timestamp")
		return false
	}
	ts, err := time.Parse(time.RFC3339, header)
	if err != nil {
		writeError(w, http.StatusBadRequest, "invalid X-AiTrack-Body-Timestamp")
		return false
	}
	if time.Since(ts) > 5*time.Minute || time.Until(ts) > 5*time.Minute {
		writeError(w, http.StatusBadRequest, "X-AiTrack-Body-Timestamp out of window")
		return false
	}
	return true
}

func parseUsageLimit(raw string) int {
	if raw == "" {
		return 20
	}
	n, err := strconv.Atoi(raw)
	if err != nil || n <= 0 {
		return 20
	}
	if n > 100 {
		return 100
	}
	return n
}
