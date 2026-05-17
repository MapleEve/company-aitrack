package service_test

import (
	"encoding/base64"
	"testing"

	"github.com/aitrack/server/internal/service"
)

func TestEncryptorDevMode(t *testing.T) {
	enc, err := service.NewHmacSecretEncryptor("")
	if err != nil {
		t.Fatal(err)
	}
	stored, err := enc.Encrypt("mysecret")
	if err != nil {
		t.Fatal(err)
	}
	if stored != "plain:mysecret" {
		t.Errorf("dev mode should prefix plain:, got %s", stored)
	}
	plain, err := enc.Decrypt("plain:mysecret")
	if err != nil {
		t.Fatal(err)
	}
	if plain != "mysecret" {
		t.Errorf("decrypt plain: expected mysecret, got %s", plain)
	}
}

func TestEncryptorAES256GCM(t *testing.T) {
	// 32-byte key base64-encoded
	key := make([]byte, 32)
	for i := range key {
		key[i] = byte(i)
	}
	b64Key := base64.StdEncoding.EncodeToString(key)

	enc, err := service.NewHmacSecretEncryptor(b64Key)
	if err != nil {
		t.Fatal(err)
	}

	plaintext := "supersecrethmac"
	stored, err := enc.Encrypt(plaintext)
	if err != nil {
		t.Fatal(err)
	}
	// Must not be plain: prefixed
	if len(stored) < 6 || stored[:6] == "plain:" {
		t.Error("should be encrypted, not plain:")
	}

	recovered, err := enc.Decrypt(stored)
	if err != nil {
		t.Fatal(err)
	}
	if recovered != plaintext {
		t.Errorf("decrypt returned %q, want %q", recovered, plaintext)
	}
}

func TestEncryptorDifferentCiphertexts(t *testing.T) {
	key := make([]byte, 32)
	b64Key := base64.StdEncoding.EncodeToString(key)
	enc, _ := service.NewHmacSecretEncryptor(b64Key)

	c1, _ := enc.Encrypt("hello")
	c2, _ := enc.Encrypt("hello")
	// Each encrypt call uses a random IV, so ciphertexts must differ
	if c1 == c2 {
		t.Error("two encryptions of the same plaintext must differ (random IV)")
	}
}

func TestEncryptorBadKey(t *testing.T) {
	_, err := service.NewHmacSecretEncryptor("not-valid-base64!!!")
	if err == nil {
		t.Error("expected error for invalid base64 key")
	}

	// Wrong key length (not 32 bytes)
	shortKey := base64.StdEncoding.EncodeToString([]byte("tooshort"))
	_, err = service.NewHmacSecretEncryptor(shortKey)
	if err == nil {
		t.Error("expected error for key that is not 32 bytes")
	}
}

func TestEncryptorDecryptWithoutKey(t *testing.T) {
	enc, _ := service.NewHmacSecretEncryptor("")
	// Encrypted value without a key should fail
	_, err := enc.Decrypt("SomeBase64EncryptedValue=")
	if err == nil {
		t.Error("expected error decrypting without key")
	}
}
