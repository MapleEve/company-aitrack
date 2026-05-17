package handler

import (
	"net/http"

	"github.com/aitrack/server/internal/service"
)

// StatsHandler handles GET /api/v1/ai-track/stats and GET /api/v1/ai-track/devices.
type StatsHandler struct {
	auth  *AuthMiddleware
	stats *service.StatsService
}

func NewStatsHandler(auth *AuthMiddleware, stats *service.StatsService) *StatsHandler {
	return &StatsHandler{auth: auth, stats: stats}
}

func (h *StatsHandler) Stats(w http.ResponseWriter, r *http.Request) {
	if h.auth.ResolveToken(w, r) == nil {
		return
	}
	groupBy := r.URL.Query().Get("group_by")
	if groupBy == "" {
		groupBy = "token"
	}
	rows, err := h.stats.GetStats(groupBy)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "stats query failed")
		return
	}
	writeJSON(w, http.StatusOK, rows)
}

func (h *StatsHandler) Devices(w http.ResponseWriter, r *http.Request) {
	if h.auth.ResolveToken(w, r) == nil {
		return
	}
	devices, err := h.stats.GetDevices()
	if err != nil {
		writeError(w, http.StatusInternalServerError, "devices query failed")
		return
	}
	writeJSON(w, http.StatusOK, devices)
}
