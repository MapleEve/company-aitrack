package service

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/base64"
	"errors"
	"fmt"
	"io"
)

const (
	plainPrefix = "plain:"
	ivBytes     = 12
	tagBits     = 128
)

// HmacSecretEncryptor encrypts/decrypts hmac_secret using AES-256-GCM.
// Storage format (Base64): [12-byte IV][ciphertext + 16-byte GCM tag].
// Falls back to "plain:" prefix when no key is configured (dev only).
type HmacSecretEncryptor struct {
	secretKey []byte // nil = dev mode
}

func NewHmacSecretEncryptor(b64Key string) (*HmacSecretEncryptor, error) {
	if b64Key == "" {
		return &HmacSecretEncryptor{}, nil
	}
	key, err := base64.StdEncoding.DecodeString(b64Key)
	if err != nil {
		return nil, fmt.Errorf("decode secret_key: %w", err)
	}
	if len(key) != 32 {
		return nil, fmt.Errorf("secret_key must decode to 32 bytes (AES-256); got %d", len(key))
	}
	return &HmacSecretEncryptor{secretKey: key}, nil
}

func (e *HmacSecretEncryptor) Encrypt(plaintext string) (string, error) {
	if e.secretKey == nil {
		return plainPrefix + plaintext, nil
	}
	block, err := aes.NewCipher(e.secretKey)
	if err != nil {
		return "", err
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return "", err
	}
	iv := make([]byte, ivBytes)
	if _, err := io.ReadFull(rand.Reader, iv); err != nil {
		return "", err
	}
	ciphertext := gcm.Seal(nil, iv, []byte(plaintext), nil)
	result := append(iv, ciphertext...)
	return base64.StdEncoding.EncodeToString(result), nil
}

func (e *HmacSecretEncryptor) Decrypt(stored string) (string, error) {
	if len(stored) >= len(plainPrefix) && stored[:len(plainPrefix)] == plainPrefix {
		return stored[len(plainPrefix):], nil
	}
	if e.secretKey == nil {
		return "", errors.New("secret_key not configured but hmac_secret is encrypted")
	}
	raw, err := base64.StdEncoding.DecodeString(stored)
	if err != nil {
		return "", fmt.Errorf("base64 decode: %w", err)
	}
	if len(raw) < ivBytes {
		return "", errors.New("ciphertext too short")
	}
	iv := raw[:ivBytes]
	ciphertext := raw[ivBytes:]

	block, err := aes.NewCipher(e.secretKey)
	if err != nil {
		return "", err
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return "", err
	}
	plaintext, err := gcm.Open(nil, iv, ciphertext, nil)
	if err != nil {
		return "", fmt.Errorf("gcm open: %w", err)
	}
	return string(plaintext), nil
}
