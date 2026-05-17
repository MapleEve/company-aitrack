package service_test

import (
	"database/sql"
	"testing"

	"github.com/aitrack/server/internal/db"
	"github.com/aitrack/server/internal/model"
	"github.com/aitrack/server/internal/service"
)

func openTestDB(t *testing.T) *sql.DB {
	t.Helper()
	database, err := db.Open(":memory:")
	if err != nil {
		t.Fatalf("open test db: %v", err)
	}
	t.Cleanup(func() { database.Close() })
	return database
}

func TestComputeTokenKey(t *testing.T) {
	cases := []struct {
		input string
		want  string
	}{
		{"aitrack_abcdef1234567890", "abcdef…7890"},
		{"aitrack_short", "short"},            // <= 10 chars after strip → no ellipsis
		{"rawtoken1234567890", "rawtok…7890"}, // no prefix to strip
	}
	for _, c := range cases {
		got := service.ComputeTokenKey(c.input)
		if got != c.want {
			t.Errorf("ComputeTokenKey(%q) = %q, want %q", c.input, got, c.want)
		}
	}
}

func TestTokenService_CreateAndFind(t *testing.T) {
	database := openTestDB(t)
	enc, _ := service.NewHmacSecretEncryptor("")
	sig := service.NewSignatureService()
	repo := service.NewTokenRepository(database)
	svc := service.NewTokenService(repo, sig, enc)

	resp, err := svc.CreateToken(&model.CreateTokenRequest{Owner: "alice", Note: "test"})
	if err != nil {
		t.Fatal(err)
	}
	if resp.Token == "" {
		t.Error("token should not be empty")
	}
	if resp.HmacSecret == "" {
		t.Error("hmac_secret should not be empty")
	}
	if resp.TokenKey == "" {
		t.Error("token_key should not be empty")
	}

	// Find returns decrypted token
	found, err := svc.FindActiveToken(resp.Token)
	if err != nil {
		t.Fatal(err)
	}
	if found == nil {
		t.Fatal("expected to find token")
	}
	if found.HmacSecret != resp.HmacSecret {
		t.Errorf("decrypted hmac_secret mismatch: got %q, want %q", found.HmacSecret, resp.HmacSecret)
	}
	if found.TokenKey != resp.TokenKey {
		t.Errorf("token_key mismatch: got %q, want %q", found.TokenKey, resp.TokenKey)
	}
}

func TestTokenService_FindInactiveToken(t *testing.T) {
	database := openTestDB(t)
	enc, _ := service.NewHmacSecretEncryptor("")
	sig := service.NewSignatureService()
	repo := service.NewTokenRepository(database)
	svc := service.NewTokenService(repo, sig, enc)

	resp, _ := svc.CreateToken(&model.CreateTokenRequest{Owner: "bob"})

	// Mark inactive
	database.Exec("UPDATE tokens SET active = 0 WHERE token_key = ?", resp.TokenKey)

	found, err := svc.FindActiveToken(resp.Token)
	if err != nil {
		t.Fatal(err)
	}
	if found != nil {
		t.Error("inactive token should return nil")
	}
}

func TestTokenService_FindNonExistentToken(t *testing.T) {
	database := openTestDB(t)
	enc, _ := service.NewHmacSecretEncryptor("")
	sig := service.NewSignatureService()
	repo := service.NewTokenRepository(database)
	svc := service.NewTokenService(repo, sig, enc)

	found, err := svc.FindActiveToken("aitrack_doesnotexist12345678")
	if err != nil {
		t.Fatal(err)
	}
	if found != nil {
		t.Error("non-existent token should return nil")
	}
}
