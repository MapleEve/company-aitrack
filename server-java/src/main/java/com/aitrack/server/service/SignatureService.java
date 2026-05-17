package com.aitrack.server.service;

import org.springframework.stereotype.Service;

import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.security.InvalidKeyException;

/**
 * HMAC-SHA256 and SHA-256 helpers.
 * All hex output is lowercase — Rust client uses the same convention.
 */
@Service
public class SignatureService {

    private static final String HMAC_ALGO = "HmacSHA256";
    private static final String SHA256_ALGO = "SHA-256";

    public String sha256Hex(byte[] data) {
        try {
            MessageDigest md = MessageDigest.getInstance(SHA256_ALGO);
            return bytesToHex(md.digest(data));
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("SHA-256 not available", e);
        }
    }

    public String sha256Hex(String data) {
        return sha256Hex(data.getBytes(StandardCharsets.UTF_8));
    }

    public String hmacSha256Hex(String secret, String message) {
        try {
            Mac mac = Mac.getInstance(HMAC_ALGO);
            SecretKeySpec keySpec = new SecretKeySpec(
                secret.getBytes(StandardCharsets.UTF_8), HMAC_ALGO);
            mac.init(keySpec);
            return bytesToHex(mac.doFinal(message.getBytes(StandardCharsets.UTF_8)));
        } catch (NoSuchAlgorithmException | InvalidKeyException e) {
            throw new IllegalStateException("HMAC-SHA256 not available", e);
        }
    }

    /**
     * Compute the request-level X-AiTrack-Signature.
     * canonical = timestamp + "\n" + sha256_hex(rawBodyBytes)
     */
    public String computeRequestSignature(String hmacSecret, String timestamp, byte[] rawBodyBytes) {
        String bodyHash = sha256Hex(rawBodyBytes);
        String canonical = timestamp + "\n" + bodyHash;
        return hmacSha256Hex(hmacSecret, canonical);
    }

    /**
     * Compute the per-record record_sig (CONTRACT.md v1.1).
     * Canonical string (fields joined with "\n"):
     *   token_key, device_id, hostname, timestamp, tool, file_path, repo_url, current_sha,
     *   added_lines (decimal), removed_lines (decimal), sha256_hex(diff_hunk or "")
     *
     * Field order and "\n" separator MUST match the Rust client exactly.
     */
    public String computeRecordSig(
        String hmacSecret,
        String tokenKey,
        String deviceId,
        String hostname,
        String timestamp,
        String tool,
        String filePath,
        String repoUrl,
        String currentSha,
        long addedLines,
        long removedLines,
        String diffHunk
    ) {
        String diffHunkHash = sha256Hex(diffHunk != null ? diffHunk : "");
        String canonical = tokenKey + "\n"
            + deviceId + "\n"
            + hostname + "\n"
            + timestamp + "\n"
            + tool + "\n"
            + filePath + "\n"
            + repoUrl + "\n"
            + currentSha + "\n"
            + addedLines + "\n"
            + removedLines + "\n"
            + diffHunkHash;
        return hmacSha256Hex(hmacSecret, canonical);
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
