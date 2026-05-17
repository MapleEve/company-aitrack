package handler

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

// NewRouter wires all handlers into a chi router.
func NewRouter(admin *AdminHandler, edits *EditsHandler, hb *HeartbeatHandler, stats *StatsHandler) http.Handler {
	r := chi.NewRouter()
	r.Use(middleware.Recoverer)
	r.Use(middleware.RealIP)

	r.Post("/admin/tokens", admin.CreateToken)

	r.Route("/api/v1/ai-track", func(r chi.Router) {
		r.Post("/edits", edits.SubmitEdits)
		r.Get("/edits", edits.QueryEdits)
		r.Post("/heartbeat", hb.Heartbeat)
		r.Get("/stats", stats.Stats)
		r.Get("/devices", stats.Devices)
	})

	return r
}
