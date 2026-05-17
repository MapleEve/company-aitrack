package com.aitrack.server;

import com.aitrack.server.service.SignatureService;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

class SignatureServiceTest {

    private SignatureService service;

    @BeforeEach
    void setUp() {
        service = new SignatureService();
    }

    @Test
    void sha256Hex_knownInput() {
        // echo -n "hello" | sha256sum = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        String result = service.sha256Hex("hello");
        assertThat(result).isEqualTo("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    @Test
    void sha256Hex_emptyString() {
        // echo -n "" | sha256sum = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        String result = service.sha256Hex("");
        assertThat(result).isEqualTo("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    @Test
    void hmacSha256Hex_knownInput() {
        // Canonical HMAC-SHA256 test vector: key="key",
        // msg="The quick brown fox jumps over the lazy dog"
        // result: f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8
        String result = service.hmacSha256Hex(
            "key",
            "The quick brown fox jumps over the lazy dog"
        );
        assertThat(result).isEqualTo("f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8");
    }

    @Test
    void computeRecordSig_deterministicOutput() {
        String sig1 = service.computeRecordSig(
            "mysecret", "abc123…ef01", "device-uuid-001", "MacBook-Pro.local",
            "2026-05-17T10:00:00Z", "claude", "src/main.rs",
            "git@github.com:org/repo.git", "a1b2c3d4e5f6",
            10, 3, "@@ -1,3 +1,10 @@\n-old\n+new\n"
        );
        String sig2 = service.computeRecordSig(
            "mysecret", "abc123…ef01", "device-uuid-001", "MacBook-Pro.local",
            "2026-05-17T10:00:00Z", "claude", "src/main.rs",
            "git@github.com:org/repo.git", "a1b2c3d4e5f6",
            10, 3, "@@ -1,3 +1,10 @@\n-old\n+new\n"
        );
        assertThat(sig1).isEqualTo(sig2);
        assertThat(sig1).hasSize(64);
        assertThat(sig1).matches("[0-9a-f]+");
    }

    @Test
    void computeRecordSig_differentInputsDifferentOutputs() {
        String sig1 = service.computeRecordSig(
            "mysecret", "abc123…ef01", "device-uuid-001", "MacBook-Pro.local",
            "2026-05-17T10:00:00Z", "claude", "src/main.rs",
            "git@github.com:org/repo.git", "a1b2c3d4e5f6",
            10, 3, null
        );
        String sig2 = service.computeRecordSig(
            "mysecret", "abc123…ef01", "device-uuid-001", "MacBook-Pro.local",
            "2026-05-17T10:00:00Z", "claude", "src/main.rs",
            "git@github.com:org/repo.git", "a1b2c3d4e5f6",
            10, 4, null  // different removed_lines
        );
        assertThat(sig1).isNotEqualTo(sig2);
    }

    @Test
    void computeRecordSig_nullDiffHunkTreatedAsEmpty() {
        String sigNull = service.computeRecordSig(
            "sec", "tok", "dev", "host", "ts", "tool", "fp", "repo", "sha", 1, 1, null
        );
        String sigEmpty = service.computeRecordSig(
            "sec", "tok", "dev", "host", "ts", "tool", "fp", "repo", "sha", 1, 1, ""
        );
        assertThat(sigNull).isEqualTo(sigEmpty);
    }

    @Test
    void computeRequestSignature_deterministicOutput() {
        byte[] body = "{\"device_id\":\"x\"}".getBytes(java.nio.charset.StandardCharsets.UTF_8);
        String sig1 = service.computeRequestSignature("secret", "1715940000", body);
        String sig2 = service.computeRequestSignature("secret", "1715940000", body);
        assertThat(sig1).isEqualTo(sig2);
        assertThat(sig1).hasSize(64);
    }
}
