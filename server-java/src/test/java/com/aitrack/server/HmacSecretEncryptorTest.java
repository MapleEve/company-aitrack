package com.aitrack.server;

import com.aitrack.server.config.AiTrackProperties;
import com.aitrack.server.service.HmacSecretEncryptor;
import org.junit.jupiter.api.Test;

import java.util.Base64;

import static org.assertj.core.api.Assertions.assertThat;
import static org.assertj.core.api.Assertions.assertThatThrownBy;

class HmacSecretEncryptorTest {

    private static final String VALID_SECRET_B64;

    static {
        // 32 random bytes encoded in Base64
        byte[] key = new byte[32];
        for (int i = 0; i < 32; i++) key[i] = (byte) (i + 1);
        VALID_SECRET_B64 = Base64.getEncoder().encodeToString(key);
    }

    private HmacSecretEncryptor encryptorWithKey(String b64Key) {
        AiTrackProperties props = new AiTrackProperties();
        props.setSecretKey(b64Key);
        return new HmacSecretEncryptor(props);
    }

    // --- Encryption with real key ---

    @Test
    void encryptDecrypt_roundTrip_restoresOriginal() {
        HmacSecretEncryptor enc = encryptorWithKey(VALID_SECRET_B64);
        String plaintext = "my-hmac-secret-value";
        String ciphertext = enc.encrypt(plaintext);
        String decrypted = enc.decrypt(ciphertext);
        assertThat(decrypted).isEqualTo(plaintext);
    }

    @Test
    void encrypt_producesDifferentCiphertextsEachCall_dueToRandomIV() {
        HmacSecretEncryptor enc = encryptorWithKey(VALID_SECRET_B64);
        String plain = "same-secret";
        String c1 = enc.encrypt(plain);
        String c2 = enc.encrypt(plain);
        // Each call uses a fresh random IV, so ciphertexts should differ
        assertThat(c1).isNotEqualTo(c2);
    }

    @Test
    void encrypt_isBase64Encoded() {
        HmacSecretEncryptor enc = encryptorWithKey(VALID_SECRET_B64);
        String ciphertext = enc.encrypt("test");
        // Must be valid Base64
        byte[] decoded = Base64.getDecoder().decode(ciphertext);
        // IV (12 bytes) + tag (16 bytes) + ciphertext; minimum length = 28 for empty plaintext
        assertThat(decoded.length).isGreaterThanOrEqualTo(28);
    }

    @Test
    void encryptDecrypt_emptyPlaintext() {
        HmacSecretEncryptor enc = encryptorWithKey(VALID_SECRET_B64);
        String ciphertext = enc.encrypt("");
        assertThat(enc.decrypt(ciphertext)).isEqualTo("");
    }

    @Test
    void encryptDecrypt_longPlaintext() {
        HmacSecretEncryptor enc = encryptorWithKey(VALID_SECRET_B64);
        String long64hex = "a".repeat(64);
        assertThat(enc.decrypt(enc.encrypt(long64hex))).isEqualTo(long64hex);
    }

    // --- Plain fallback (no key configured) ---

    @Test
    void encrypt_noKey_returnsPlainPrefix() {
        HmacSecretEncryptor enc = encryptorWithKey("");
        String result = enc.encrypt("my-secret");
        assertThat(result).startsWith("plain:");
        assertThat(result).isEqualTo("plain:my-secret");
    }

    @Test
    void decrypt_plainPrefix_returnsRawValue() {
        HmacSecretEncryptor enc = encryptorWithKey("");
        String result = enc.decrypt("plain:my-secret");
        assertThat(result).isEqualTo("my-secret");
    }

    @Test
    void decrypt_plainPrefix_noKeyRequired() {
        // Even if no key is configured, "plain:" prefix is handled directly
        HmacSecretEncryptor enc = encryptorWithKey(null);
        assertThat(enc.decrypt("plain:secret-value")).isEqualTo("secret-value");
    }

    // --- Error cases ---

    @Test
    void decrypt_encryptedValueButNoKey_throwsIllegalState() {
        HmacSecretEncryptor enc = encryptorWithKey("");
        // Build a real ciphertext with a key, then try to decrypt without the key
        HmacSecretEncryptor encWithKey = encryptorWithKey(VALID_SECRET_B64);
        String ciphertext = encWithKey.encrypt("secret");
        assertThatThrownBy(() -> enc.decrypt(ciphertext))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("not configured");
    }

    @Test
    void resolveKey_wrongByteLength_throwsIllegalState() {
        // 16 bytes = 128-bit key — only 32 bytes (AES-256) is accepted
        byte[] shortKey = new byte[16];
        String shortB64 = Base64.getEncoder().encodeToString(shortKey);
        HmacSecretEncryptor enc = encryptorWithKey(shortB64);
        assertThatThrownBy(() -> enc.encrypt("test"))
                .isInstanceOf(IllegalStateException.class)
                .hasMessageContaining("32 bytes");
    }

    @Test
    void decrypt_corruptedBase64_throwsIllegalState() {
        HmacSecretEncryptor enc = encryptorWithKey(VALID_SECRET_B64);
        // Not a valid ciphertext (too short, and not the plain: prefix)
        assertThatThrownBy(() -> enc.decrypt("notvalid!!!!"))
                .isInstanceOf(Exception.class);
    }
}
