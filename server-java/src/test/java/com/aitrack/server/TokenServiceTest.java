package com.aitrack.server;

import com.aitrack.server.service.TokenService;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

class TokenServiceTest {

    @Test
    void computeTokenKey_normalToken() {
        // "aitrack_" + 32-hex = "aitrack_" + "abcdef1234567890abcdef1234567890"
        // stripped = "abcdef1234567890abcdef1234567890" (32 chars)
        // first6 = "abcdef", last4 = "7890"
        String key = TokenService.computeTokenKey("aitrack_abcdef1234567890abcdef1234567890");
        assertThat(key).isEqualTo("abcdef…7890");
    }

    @Test
    void computeTokenKey_shortStrippedPart() {
        String key = TokenService.computeTokenKey("aitrack_short");
        // stripped = "short" (5 chars, <= 10), returned as-is
        assertThat(key).isEqualTo("short");
    }

    @Test
    void computeTokenKey_noPrefixFallback() {
        // no "aitrack_" prefix — use raw token
        String key = TokenService.computeTokenKey("rawtoken1234567890");
        // length > 10, first6 + "…" + last4
        assertThat(key).isEqualTo("rawtok…7890");
    }
}
