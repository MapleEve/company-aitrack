package handler

import (
	"encoding/json"
	"net/http"
	"strings"

	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
)

// HeartbeatHandler handles POST /api/v1/ai-track/heartbeat.
type HeartbeatHandler struct {
	auth      *AuthMiddleware
	heartbeat *service.HeartbeatService
}

func NewHeartbeatHandler(auth *AuthMiddleware, hb *service.HeartbeatService) *HeartbeatHandler {
	return &HeartbeatHandler{auth: auth, heartbeat: hb}
}

func (h *HeartbeatHandler) Heartbeat(w http.ResponseWriter, r *http.Request) {
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

	var req model.HeartbeatRequest
	if err := json.Unmarshal(rawBody, &req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid JSON")
		return
	}
	if strings.TrimSpace(req.DeviceID) == "" {
		writeError(w, http.StatusBadRequest, "device_id is required")
		return
	}

	if err := h.heartbeat.RecordHeartbeat(token, &req); err != nil {
		writeError(w, http.StatusInternalServerError, "failed to record heartbeat")
		return
	}
	writeJSON(w, http.StatusOK, map[string]bool{"ok": true})
}
