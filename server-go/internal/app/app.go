package app

import (
	"fmt"
	"net/http"

	"github.com/aitrack/server/internal/config"
	"github.com/aitrack/server/internal/db"
	"github.com/aitrack/server/internal/handler"
	"github.com/aitrack/server/internal/service"
)

// Build wires all dependencies and returns an http.Handler ready to serve.
// The caller owns closing the DB.
func Build(cfg *config.Config) (http.Handler, func(), error) {
	database, err := db.Open(cfg.DB.Path)
	if err != nil {
		return nil, nil, fmt.Errorf("open db: %w", err)
	}
	cleanup := func() { database.Close() }

	enc, err := service.NewHmacSecretEncryptor(cfg.SecretKey)
	if err != nil {
		cleanup()
		return nil, nil, fmt.Errorf("encryptor: %w", err)
	}

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

	return handler.NewRouter(adminH, editsH, hbH, statsH), cleanup, nil
}
