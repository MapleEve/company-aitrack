package handler

import (
	"encoding/json"
	"net/http"
	"strconv"

	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
)

// EditsHandler handles POST /api/v1/ai-track/edits and GET /api/v1/ai-track/edits.
type EditsHandler struct {
	auth   *AuthMiddleware
	ingest *service.IngestService
}

func NewEditsHandler(auth *AuthMiddleware, ingest *service.IngestService) *EditsHandler {
	return &EditsHandler{auth: auth, ingest: ingest}
}

func (h *EditsHandler) SubmitEdits(w http.ResponseWriter, r *http.Request) {
	rawBody, ok := ReadBody(w, r)
	if !ok {
		return
	}

	token := h.auth.ResolveToken(w, r)
	if token == nil {
		return
	}
	if !h.auth.ValidateRequestSignature(w, r, token, rawBody) {
		return
	}

	var req model.EditBatchRequest
	if err := json.Unmarshal(rawBody, &req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON")
		return
	}
	if len(req.Edits) == 0 {
		writeError(w, http.StatusBadRequest, "edits array is required and must not be empty")
		return
	}

	resp := h.ingest.Ingest(token, &req)
	writeJSON(w, http.StatusOK, resp)
}

func (h *EditsHandler) QueryEdits(w http.ResponseWriter, r *http.Request) {
	token := h.auth.ResolveToken(w, r)
	if token == nil {
		return
	}

	q := r.URL.Query()
	tokenKey := q.Get("token_key")
	repo := q.Get("repo")
	page := parseInt(q.Get("page"), 0)
	size := clampInt(parseInt(q.Get("size"), 20), 1, 100)

	result, err := h.ingest.QueryEdits(tokenKey, repo, page, size)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "query failed")
		return
	}
	writeJSON(w, http.StatusOK, result)
}

func parseInt(s string, defaultVal int) int {
	if s == "" {
		return defaultVal
	}
	n, err := strconv.Atoi(s)
	if err != nil {
		return defaultVal
	}
	return n
}

func clampInt(v, min, max int) int {
	if v < min {
		return min
	}
	if v > max {
		return max
	}
	return v
}
