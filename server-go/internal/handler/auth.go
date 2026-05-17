package handler

import (
	"io"
	"math"
	"net/http"
	"strconv"
	"time"

	"github.com/aitrack/server/internal/config"
	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
)

// AuthMiddleware provides request-level auth helpers (steps 1-3).
type AuthMiddleware struct {
	tokenSvc *service.TokenService
	sig      *service.SignatureService
	cfg      *config.Config
}

func NewAuthMiddleware(ts *service.TokenService, sig *service.SignatureService, cfg *config.Config) *AuthMiddleware {
	return &AuthMiddleware{tokenSvc: ts, sig: sig, cfg: cfg}
}

// ResolveToken extracts and validates the Bearer token. Returns the active token or writes 401.
func (a *AuthMiddleware) ResolveToken(w http.ResponseWriter, r *http.Request) *model.Token {
	authHeader := r.Header.Get("Authorization")
	if len(authHeader) < 8 || authHeader[:7] != "Bearer " {
		writeError(w, http.StatusUnauthorized, "missing bearer token")
		return nil
	}
	rawToken := authHeader[7:]
	token, err := a.tokenSvc.FindActiveToken(rawToken)
	if err != nil || token == nil {
		writeError(w, http.StatusUnauthorized, "invalid or inactive token")
		return nil
	}
	return token
}

// ValidateRequestSignature validates X-AiTrack-Timestamp and X-AiTrack-Signature (steps 2-3).
// rawBody must already be read from the request.
func (a *AuthMiddleware) ValidateRequestSignature(w http.ResponseWriter, r *http.Request, token *model.Token, rawBody []byte) bool {
	tsHeader := r.Header.Get("X-AiTrack-Timestamp")
	if tsHeader == "" {
		writeError(w, http.StatusUnauthorized, "missing X-AiTrack-Timestamp")
		return false
	}
	ts, err := strconv.ParseInt(tsHeader, 10, 64)
	if err != nil {
		writeError(w, http.StatusUnauthorized, "invalid X-AiTrack-Timestamp")
		return false
	}
	nowSec := time.Now().Unix()
	if int64(math.Abs(float64(nowSec-ts))) > a.cfg.TimestampWindowSeconds {
		writeError(w, http.StatusUnauthorized, "timestamp out of window")
		return false
	}
	sigHeader := r.Header.Get("X-AiTrack-Signature")
	if sigHeader == "" {
		writeError(w, http.StatusUnauthorized, "missing X-AiTrack-Signature")
		return false
	}
	expected := a.sig.ComputeRequestSignature(token.HmacSecret, tsHeader, rawBody)
	if !service.ConstantTimeEqual(expected, sigHeader) {
		writeError(w, http.StatusUnauthorized, "invalid X-AiTrack-Signature")
		return false
	}
	return true
}

// ReadBody reads the full request body (needed for HMAC over raw bytes).
func ReadBody(w http.ResponseWriter, r *http.Request) ([]byte, bool) {
	body, err := io.ReadAll(r.Body)
	if err != nil {
		writeError(w, http.StatusBadRequest, "failed to read body")
		return nil, false
	}
	return body, true
}
