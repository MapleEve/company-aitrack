package com.aitrack.server.testkit;

import com.aitrack.server.entity.TokenEntity;

import java.time.Instant;
import java.util.function.Consumer;

/**
 * Deterministic factory for TokenEntity test instances.
 * hmacSecret is already decrypted (as returned by TokenService.findActiveToken).
 */
public final class TokenEntityFactory {

    public static final String DEFAULT_HMAC_SECRET = EditDtoFactory.DEFAULT_HMAC_SECRET;
    public static final String DEFAULT_TOKEN_KEY    = EditDtoFactory.DEFAULT_TOKEN_KEY;
    public static final String DEFAULT_TOKEN_HASH   = "a".repeat(64);
    public static final String DEFAULT_OWNER        = "test-owner";

    private TokenEntityFactory() {}

    /** Returns an active token entity with plaintext hmacSecret (already decrypted). */
    public static TokenEntity build() {
        TokenEntity t = new TokenEntity();
        t.setId(1L);
        t.setTokenHash(DEFAULT_TOKEN_HASH);
        t.setTokenKey(DEFAULT_TOKEN_KEY);
        t.setHmacSecret(DEFAULT_HMAC_SECRET);
        t.setOwner(DEFAULT_OWNER);
        t.setNote("test token");
        t.setActive(true);
        t.setCreatedAt(Instant.parse("2026-01-01T00:00:00Z"));
        return t;
    }

    /** Builder-style customisation. */
    public static TokenEntity with(Consumer<TokenEntity> customizer) {
        TokenEntity t = build();
        customizer.accept(t);
        return t;
    }
}
