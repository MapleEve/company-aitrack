// Package app is the composition root. It wires infrastructure adapters,
// driven adapters, domain services, application use cases and HTTP handlers
// into a ready-to-serve http.Handler.
package app

import (
	"fmt"
	"net/http"

	dbadapter "github.com/aitrack/server/internal/adapter/db"
	"github.com/aitrack/server/internal/adapter/handler"
	"github.com/aitrack/server/internal/application"
	"github.com/aitrack/server/internal/domain/service"
	"github.com/aitrack/server/internal/infrastructure/config"
	"github.com/aitrack/server/internal/infrastructure/db"
)

// Build wires all dependencies and returns an http.Handler ready to serve.
// The caller owns closing the DB.
func Build(cfg *config.Config) (http.Handler, func(), error) {
	if cfg.DB.DatabaseURL == "" {
		return nil, nil, fmt.Errorf("DATABASE_URL is required")
	}

	database, err := db.Open(cfg.DB.DatabaseURL)
	if err != nil {
		return nil, nil, fmt.Errorf("open db: %w", err)
	}
	cleanup := func() { database.Close() }

	enc, err := service.NewHmacSecretEncryptor(cfg.SecretKey)
	if err != nil {
		cleanup()
		return nil, nil, fmt.Errorf("encryptor: %w", err)
	}

	// Domain services.
	sig := service.NewSignatureService()
	diff := service.NewDiffConsistencyService()
	ev := service.NewEditValidator()

	// Driven adapters (persistence).
	tokenAdapter := dbadapter.NewTokenAdapter(database)
	editAdapter := dbadapter.NewEditRecordAdapter(database)
	deviceAdapter := dbadapter.NewDeviceAdapter(database)

	// Map infrastructure config onto the domain validation policy.
	policy := service.ValidationPolicy{
		RateLimitPerHour:  cfg.RateLimitPerHour,
		MaxAddedLines:     cfg.MaxAddedLines,
		RepoWhitelistURLs: cfg.RepoWhitelist.URLs,
		EnforceWhitelist:  cfg.RepoWhitelist.Enforce,
	}
	validationSvc := service.NewValidationService(sig, diff, editAdapter, policy)

	// Application use cases.
	tokenSvc := application.NewTokenService(tokenAdapter, sig, enc)
	ingestSvc := application.NewIngestService(validationSvc, ev, editAdapter)
	heartbeatSvc := application.NewHeartbeatService(deviceAdapter)
	statsSvc := application.NewStatsService(editAdapter, deviceAdapter)

	// Driving adapters (HTTP).
	auth := handler.NewAuthMiddleware(tokenSvc, sig, cfg)
	adminH := handler.NewAdminHandler(tokenSvc, cfg)
	editsH := handler.NewEditsHandler(auth, ingestSvc)
	hbH := handler.NewHeartbeatHandler(auth, heartbeatSvc)
	statsH := handler.NewStatsHandler(auth, statsSvc)
	searchH := handler.NewSearchHandler(database, cfg.AdminKey, true)
	similarH := handler.NewSimilarHandler(database, cfg.AdminKey, true)
	profileH := handler.NewProfileHandler(database, cfg.AdminKey)
	return handler.NewRouter(adminH, editsH, hbH, statsH, searchH, similarH, profileH), cleanup, nil
}
