package com.aitrack.server.service;

import com.aitrack.server.config.AiTrackProperties;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;

import javax.crypto.Cipher;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;
import javax.crypto.spec.SecretKeySpec;
import java.security.SecureRandom;
import java.util.Base64;

/**
 * AES-256-GCM encryption for hmac_secret stored in the tokens table.
 *
 * Storage format (Base64-encoded):  [12-byte IV][ciphertext + 16-byte GCM tag]
 *
 * Key source: aitrack.secret-key in application.yml (Base64, 32 bytes = 256 bits).
 *
 * WHY: hmac_secret must be readable server-side to recompute record_sig, so it
 * cannot be one-way hashed. Encrypting at rest ensures a database dump alone does
 * not expose the signing keys — an attacker also needs the application secret-key.
 *
 * PRODUCTION NOTE: For stronger guarantees, replace secret-key with a KMS-backed
 * key (AWS KMS, GCP Cloud KMS, HashiCorp Vault). The encrypt/decrypt interface here
 * is intentionally thin so the implementation can be swapped without touching callers.
 *
 * If aitrack.secret-key is not configured (dev/test), encryption is skipped and the
 * raw value is stored with a "plain:" prefix so callers can detect and handle it.
 * Set secret-key in production — the fallback exists only for local development.
 */
@Service
@RequiredArgsConstructor
public class HmacSecretEncryptor {

    private static final String ALGORITHM = "AES/GCM/NoPadding";
    private static final int IV_BYTES = 12;
    private static final int TAG_BITS = 128;
    private static final String PLAIN_PREFIX = "plain:";
    private static final SecureRandom SECURE_RANDOM = new SecureRandom();

    private final AiTrackProperties props;

    /**
     * Encrypts the raw hmac_secret. Returns a Base64 string: [IV][ciphertext+tag].
     * Falls back to "plain:{value}" if secret-key is not configured.
     */
    public String encrypt(String plaintext) {
        SecretKey key = resolveKey();
        if (key == null) {
            // Dev/test fallback — never use in production
            return PLAIN_PREFIX + plaintext;
        }
        try {
            byte[] iv = new byte[IV_BYTES];
            SECURE_RANDOM.nextBytes(iv);
            Cipher cipher = Cipher.getInstance(ALGORITHM);
            cipher.init(Cipher.ENCRYPT_MODE, key, new GCMParameterSpec(TAG_BITS, iv));
            byte[] ciphertext = cipher.doFinal(plaintext.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            byte[] result = new byte[iv.length + ciphertext.length];
            System.arraycopy(iv, 0, result, 0, iv.length);
            System.arraycopy(ciphertext, 0, result, iv.length, ciphertext.length);
            return Base64.getEncoder().encodeToString(result);
        } catch (Exception e) {
            throw new IllegalStateException("hmac_secret encryption failed", e);
        }
    }

    /**
     * Decrypts a value produced by {@link #encrypt}.
     * Handles the "plain:" dev fallback transparently.
     */
    public String decrypt(String stored) {
        if (stored.startsWith(PLAIN_PREFIX)) {
            return stored.substring(PLAIN_PREFIX.length());
        }
        SecretKey key = resolveKey();
        if (key == null) {
            throw new IllegalStateException(
                "aitrack.secret-key is not configured but stored hmac_secret is encrypted");
        }
        try {
            byte[] raw = Base64.getDecoder().decode(stored);
            byte[] iv = new byte[IV_BYTES];
            System.arraycopy(raw, 0, iv, 0, IV_BYTES);
            byte[] ciphertext = new byte[raw.length - IV_BYTES];
            System.arraycopy(raw, IV_BYTES, ciphertext, 0, ciphertext.length);
            Cipher cipher = Cipher.getInstance(ALGORITHM);
            cipher.init(Cipher.DECRYPT_MODE, key, new GCMParameterSpec(TAG_BITS, iv));
            return new String(cipher.doFinal(ciphertext), java.nio.charset.StandardCharsets.UTF_8);
        } catch (Exception e) {
            throw new IllegalStateException("hmac_secret decryption failed", e);
        }
    }

    private SecretKey resolveKey() {
        String b64Key = props.getSecretKey();
        if (b64Key == null || b64Key.isBlank()) {
            return null;
        }
        byte[] keyBytes = Base64.getDecoder().decode(b64Key);
        if (keyBytes.length != 32) {
            throw new IllegalStateException(
                "aitrack.secret-key must decode to exactly 32 bytes (AES-256); got " + keyBytes.length);
        }
        return new SecretKeySpec(keyBytes, "AES");
    }
}
