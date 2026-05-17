package com.aitrack.server;

import com.aitrack.server.service.SignatureService;
import com.aitrack.server.testkit.EditDtoFactory;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import java.nio.charset.StandardCharsets;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Tests that verify the record_sig canonical string matches CONTRACT.md v1.1 exactly.
 *
 * The canonical format is (v1.1):
 *   token_key + "\n" + device_id + "\n" + hostname + "\n" + timestamp + "\n" + tool + "\n"
 *   + file_path + "\n" + repo_url + "\n" + current_sha + "\n"
 *   + added_lines (decimal) + "\n" + removed_lines (decimal) + "\n"
 *   + sha256_hex(diff_hunk or "")
 */
class SignatureServiceCanonicalTest {

    private SignatureService service;

    @BeforeEach
    void setUp() {
        service = new SignatureService();
    }

    @Test
    void recordSig_canonicalStringMatchesContract() {
        // Pre-compute the expected HMAC manually by building the canonical string
        // per CONTRACT.md v1.1 and verifying the service produces the same result.
        String hmacSecret = "myhmac";
        String tokenKey   = "abc123…ef01";
        String deviceId   = "device-uuid-001";
        String hostname   = "MacBook-Pro.local";
        String timestamp  = "2026-05-17T10:00:00Z";
        String tool       = "claude";
        String filePath   = "src/main.rs";
        String repoUrl    = "git@github.com:org/repo.git";
        String currentSha = "a1b2c3d4e5f6";
        long added        = 10L;
        long removed      = 3L;
        String diffHunk   = "@@ -1,3 +1,10 @@\n-old\n+new\n";

        String diffHash = service.sha256Hex(diffHunk);

        // Build canonical string exactly as CONTRACT.md v1.1 specifies
        String canonical = tokenKey + "\n"
            + deviceId + "\n"
            + hostname + "\n"
            + timestamp + "\n"
            + tool + "\n"
            + filePath + "\n"
            + repoUrl + "\n"
            + currentSha + "\n"
            + added + "\n"
            + removed + "\n"
            + diffHash;

        String expectedSig = service.hmacSha256Hex(hmacSecret, canonical);
        String actualSig   = service.computeRecordSig(hmacSecret, tokenKey, deviceId, hostname,
                timestamp, tool, filePath, repoUrl, currentSha, added, removed, diffHunk);

        assertThat(actualSig).isEqualTo(expectedSig);
        assertThat(actualSig).hasSize(64);
        assertThat(actualSig).matches("[0-9a-f]+");
    }

    @Test
    void recordSig_nullDiffHunkUsesEmptyStringHash() {
        // sha256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        String emptyHash = service.sha256Hex("");
        assertThat(emptyHash).isEqualTo("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

        String sigWithNull  = service.computeRecordSig("s", "t", "d", "h", "ts", "tool", "fp", "r", "sha", 1, 1, null);
        String sigWithEmpty = service.computeRecordSig("s", "t", "d", "h", "ts", "tool", "fp", "r", "sha", 1, 1, "");
        assertThat(sigWithNull).isEqualTo(sigWithEmpty);
    }

    @Test
    void requestSignature_canonicalIsTimestampNewlineBodyHash() {
        String timestamp = "1715940000";
        byte[] body = "{\"device_id\":\"x\"}".getBytes(StandardCharsets.UTF_8);
        String bodyHash = service.sha256Hex(body);

        String canonical = timestamp + "\n" + bodyHash;
        String expectedSig = service.hmacSha256Hex("mysecret", canonical);
        String actualSig   = service.computeRequestSignature("mysecret", timestamp, body);

        assertThat(actualSig).isEqualTo(expectedSig);
    }

    @Test
    void sha256Hex_byteArrayAndStringOverloadAreIdentical() {
        String input = "test string for sha256";
        String fromString = service.sha256Hex(input);
        String fromBytes  = service.sha256Hex(input.getBytes(StandardCharsets.UTF_8));
        assertThat(fromString).isEqualTo(fromBytes);
    }

    @Test
    void recordSig_fieldOrderMatters() {
        // Swap tokenKey and deviceId — should produce a different sig
        String sigCorrectOrder = service.computeRecordSig(
                "secret", "tokenKey", "deviceId", "host", "ts", "tool", "fp", "repo", "sha", 1, 1, null);
        String sigSwapped = service.computeRecordSig(
                "secret", "deviceId", "tokenKey", "host", "ts", "tool", "fp", "repo", "sha", 1, 1, null);
        assertThat(sigCorrectOrder).isNotEqualTo(sigSwapped);
    }

    @Test
    void recordSig_hostnameAffectsSignature() {
        // Different hostnames must produce different sigs
        String sig1 = service.computeRecordSig("s", "t", "d", "host-a", "ts", "tool", "fp", "r", "sha", 1, 1, null);
        String sig2 = service.computeRecordSig("s", "t", "d", "host-b", "ts", "tool", "fp", "r", "sha", 1, 1, null);
        assertThat(sig1).isNotEqualTo(sig2);
    }

    @Test
    void recordSig_addedLinesAndRemovedLinesAreDecimalStrings() {
        // added=10, removed=3 should differ from added=1, removed=03 (both stringify differently)
        String sig10_3 = service.computeRecordSig("s", "t", "d", "h", "ts", "tool", "fp", "r", "sha", 10, 3, null);
        String sig1_3  = service.computeRecordSig("s", "t", "d", "h", "ts", "tool", "fp", "r", "sha", 1, 3, null);
        assertThat(sig10_3).isNotEqualTo(sig1_3);
    }
}
