package handler

import (
	"encoding/json"
	"net/http"
	"strings"

	"github.com/aitrack/server/internal/config"
	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
)

// AdminHandler handles POST /admin/tokens.
type AdminHandler struct {
	tokenSvc *service.TokenService
	cfg      *config.Config
}

func NewAdminHandler(ts *service.TokenService, cfg *config.Config) *AdminHandler {
	return &AdminHandler{tokenSvc: ts, cfg: cfg}
}

func (h *AdminHandler) CreateToken(w http.ResponseWriter, r *http.Request) {
	if !h.verifyAdminKey(w, r) {
		return
	}

	var req model.CreateTokenRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON")
		return
	}
	if strings.TrimSpace(req.Owner) == "" {
		writeError(w, http.StatusBadRequest, "owner is required")
		return
	}

	resp, err := h.tokenSvc.CreateToken(&req)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to create token")
		return
	}
	writeJSON(w, http.StatusOK, resp)
}

func (h *AdminHandler) verifyAdminKey(w http.ResponseWriter, r *http.Request) bool {
	configured := h.cfg.AdminKey
	if configured == "" {
		writeError(w, http.StatusServiceUnavailable,
			"admin-key is not configured; set AITRACK_ADMIN_KEY before using this endpoint")
		return false
	}
	provided := strings.TrimSpace(r.Header.Get("X-Admin-Key"))
	if !service.ConstantTimeEqual(configured, provided) {
		writeError(w, http.StatusForbidden, "invalid X-Admin-Key")
		return false
	}
	return true
}
