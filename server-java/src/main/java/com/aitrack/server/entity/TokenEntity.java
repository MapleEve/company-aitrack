package com.aitrack.server.entity;

import jakarta.persistence.*;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

@Entity
@Table(name = "tokens", indexes = {
    @Index(name = "idx_tokens_hash", columnList = "token_hash", unique = true)
})
@Data
@NoArgsConstructor
public class TokenEntity {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    // SHA-256 hex of the raw token — never store plaintext
    @Column(name = "token_hash", nullable = false, unique = true, length = 64)
    private String tokenHash;

    // token_key = "aitrack_" prefix stripped, then first-6 + "…" + last-4
    @Column(name = "token_key", nullable = false, length = 16)
    private String tokenKey;

    /**
     * AES-256-GCM encrypted hmac_secret (Base64: [12-byte IV][ciphertext+16-byte GCM tag]).
     * Encrypted by HmacSecretEncryptor using aitrack.secret-key.
     *
     * -- SENSITIVE -- Database dump alone does not expose signing keys without the app key.
     *
     * This value must be decrypted at read time via HmacSecretEncryptor.decrypt() before
     * passing to SignatureService. TokenService handles encrypt/decrypt transparently.
     *
     * Production: set aitrack.secret-key to a 32-byte Base64 value (openssl rand -base64 32).
     * For stronger guarantees replace HmacSecretEncryptor with a KMS-backed implementation.
     */
    @Column(name = "hmac_secret", nullable = false, length = 128)
    private String hmacSecret;

    @Column(nullable = false, length = 128)
    private String owner;

    @Column(length = 256)
    private String note;

    @Column(nullable = false)
    private boolean active = true;

    @Column(name = "created_at", nullable = false)
    private Instant createdAt = Instant.now();
}
