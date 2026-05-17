package com.aitrack.server.service;

import com.aitrack.server.dto.CreateTokenRequest;
import com.aitrack.server.dto.CreateTokenResponse;
import com.aitrack.server.entity.TokenEntity;
import com.aitrack.server.repository.TokenRepository;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.security.SecureRandom;
import java.util.Optional;

@Service
@RequiredArgsConstructor
public class TokenService {

    private final TokenRepository tokenRepository;
    private final SignatureService signatureService;
    private final HmacSecretEncryptor encryptor;
    private static final SecureRandom SECURE_RANDOM = new SecureRandom();

    /**
     * Issues a new token + hmac_secret pair. Returns plaintext token (only shown once).
     * The hmac_secret is AES-256-GCM encrypted before persisting.
     */
    @Transactional
    public CreateTokenResponse createToken(CreateTokenRequest req) {
        String rawToken = "aitrack_" + randomHex(32);
        String hmacSecret = randomHex(32);
        String tokenHash = signatureService.sha256Hex(rawToken);
        String tokenKey = computeTokenKey(rawToken);

        TokenEntity entity = new TokenEntity();
        entity.setTokenHash(tokenHash);
        entity.setTokenKey(tokenKey);
        entity.setHmacSecret(encryptor.encrypt(hmacSecret));  // encrypted at rest
        entity.setOwner(req.getOwner());
        entity.setNote(req.getNote());
        entity.setActive(true);
        tokenRepository.save(entity);

        return new CreateTokenResponse(rawToken, hmacSecret, tokenKey);
    }

    /**
     * Looks up an active token by its SHA-256 hash.
     * Returns a TokenEntity whose hmacSecret field has been decrypted to plaintext,
     * ready for use in HMAC computation by callers.
     */
    public Optional<TokenEntity> findActiveToken(String rawToken) {
        String hash = signatureService.sha256Hex(rawToken);
        return tokenRepository.findByTokenHashAndActiveTrue(hash)
            .map(this::withDecryptedSecret);
    }

    /**
     * Returns a copy of the entity with hmacSecret decrypted to plaintext.
     * The entity is not re-persisted — this is a read-path transformation only.
     */
    private TokenEntity withDecryptedSecret(TokenEntity stored) {
        String plainSecret = encryptor.decrypt(stored.getHmacSecret());
        // Avoid mutating the JPA-managed entity; create a detached copy for callers
        TokenEntity copy = new TokenEntity();
        copy.setId(stored.getId());
        copy.setTokenHash(stored.getTokenHash());
        copy.setTokenKey(stored.getTokenKey());
        copy.setHmacSecret(plainSecret);
        copy.setOwner(stored.getOwner());
        copy.setNote(stored.getNote());
        copy.setActive(stored.isActive());
        copy.setCreatedAt(stored.getCreatedAt());
        return copy;
    }

    /**
     * token_key = strip "aitrack_" prefix, then first-6 + "…" + last-4
     */
    public static String computeTokenKey(String rawToken) {
        String stripped = rawToken.startsWith("aitrack_")
            ? rawToken.substring("aitrack_".length())
            : rawToken;
        if (stripped.length() <= 10) {
            return stripped;
        }
        return stripped.substring(0, 6) + "…" + stripped.substring(stripped.length() - 4);
    }

    private static String randomHex(int bytes) {
        byte[] buf = new byte[bytes];
        SECURE_RANDOM.nextBytes(buf);
        StringBuilder sb = new StringBuilder(bytes * 2);
        for (byte b : buf) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
