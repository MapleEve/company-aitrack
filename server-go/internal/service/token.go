package service

import (
	"crypto/rand"
	"database/sql"
	"encoding/hex"
	"fmt"
	"strings"
	"time"

	"github.com/aitrack/server/internal/model"
)

type TokenRepository struct {
	db *sql.DB
}

func NewTokenRepository(db *sql.DB) *TokenRepository {
	return &TokenRepository{db: db}
}

func (r *TokenRepository) Save(t *model.Token) error {
	_, err := r.db.Exec(
		`INSERT INTO tokens (token_hash, token_key, hmac_secret, owner, note, active, created_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		t.TokenHash, t.TokenKey, t.HmacSecret, t.Owner, t.Note,
		boolToInt(t.Active), t.CreatedAt.UTC().Format(time.RFC3339),
	)
	return err
}

func (r *TokenRepository) FindActiveByHash(hash string) (*model.Token, error) {
	row := r.db.QueryRow(
		`SELECT id, token_hash, token_key, hmac_secret, owner, note, active, created_at
		 FROM tokens WHERE token_hash = ? AND active = 1`, hash)
	return scanToken(row)
}

func scanToken(row *sql.Row) (*model.Token, error) {
	var t model.Token
	var createdAt string
	var active int
	err := row.Scan(&t.ID, &t.TokenHash, &t.TokenKey, &t.HmacSecret,
		&t.Owner, &t.Note, &active, &createdAt)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	t.Active = active == 1
	t.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
	return &t, nil
}

// TokenService issues and resolves tokens.
type TokenService struct {
	repo      *TokenRepository
	sig       *SignatureService
	encryptor *HmacSecretEncryptor
}

func NewTokenService(repo *TokenRepository, sig *SignatureService, enc *HmacSecretEncryptor) *TokenService {
	return &TokenService{repo: repo, sig: sig, encryptor: enc}
}

func (s *TokenService) CreateToken(req *model.CreateTokenRequest) (*model.CreateTokenResponse, error) {
	rawToken := "aitrack_" + randomHex(32)
	hmacSecret := randomHex(32)
	tokenHash := s.sig.SHA256HexStr(rawToken)
	tokenKey := ComputeTokenKey(rawToken)

	encrypted, err := s.encryptor.Encrypt(hmacSecret)
	if err != nil {
		return nil, fmt.Errorf("encrypt hmac_secret: %w", err)
	}

	t := &model.Token{
		TokenHash:  tokenHash,
		TokenKey:   tokenKey,
		HmacSecret: encrypted,
		Owner:      req.Owner,
		Note:       req.Note,
		Active:     true,
		CreatedAt:  time.Now().UTC(),
	}
	if err := s.repo.Save(t); err != nil {
		return nil, fmt.Errorf("save token: %w", err)
	}
	return &model.CreateTokenResponse{
		Token:      rawToken,
		HmacSecret: hmacSecret,
		TokenKey:   tokenKey,
	}, nil
}

// FindActiveToken resolves a raw Bearer token to a Token with decrypted hmac_secret.
func (s *TokenService) FindActiveToken(rawToken string) (*model.Token, error) {
	hash := s.sig.SHA256HexStr(rawToken)
	t, err := s.repo.FindActiveByHash(hash)
	if err != nil {
		return nil, err
	}
	if t == nil {
		return nil, nil
	}
	plain, err := s.encryptor.Decrypt(t.HmacSecret)
	if err != nil {
		return nil, fmt.Errorf("decrypt hmac_secret: %w", err)
	}
	t.HmacSecret = plain
	return t, nil
}

// ComputeTokenKey strips "aitrack_" prefix then returns first-6 + "…" + last-4.
func ComputeTokenKey(rawToken string) string {
	stripped := rawToken
	if strings.HasPrefix(rawToken, "aitrack_") {
		stripped = rawToken[len("aitrack_"):]
	}
	if len(stripped) <= 10 {
		return stripped
	}
	return stripped[:6] + "…" + stripped[len(stripped)-4:]
}

func randomHex(n int) string {
	b := make([]byte, n)
	rand.Read(b)
	return hex.EncodeToString(b)
}

func boolToInt(b bool) int {
	if b {
		return 1
	}
	return 0
}
