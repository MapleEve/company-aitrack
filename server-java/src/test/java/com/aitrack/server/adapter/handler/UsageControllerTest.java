package com.aitrack.server.adapter.handler;

import com.aitrack.server.adapter.db.TokenRepository;
import com.aitrack.server.adapter.db.UsageSubscriptionSnapshotRepository;
import com.aitrack.server.application.TokenService;
import com.aitrack.server.domain.model.TokenEntity;
import com.aitrack.server.domain.service.SignatureService;
import com.aitrack.server.infrastructure.config.AiTrackServerApplication;
import com.aitrack.server.testkit.EditDtoFactory;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.test.annotation.DirtiesContext;
import org.springframework.test.web.servlet.MockMvc;

import java.io.ByteArrayOutputStream;
import java.time.Instant;
import java.util.Map;
import java.util.zip.GZIPOutputStream;

import static org.hamcrest.Matchers.is;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@SpringBootTest(classes = AiTrackServerApplication.class)
@AutoConfigureMockMvc
@DirtiesContext(classMode = DirtiesContext.ClassMode.AFTER_EACH_TEST_METHOD)
class UsageControllerTest {

    @Autowired MockMvc mockMvc;
    @Autowired ObjectMapper objectMapper;
    @Autowired TokenRepository tokenRepository;
    @Autowired UsageSubscriptionSnapshotRepository subscriptionRepository;
    @Autowired SignatureService signatureService;

    private static final String RAW_TOKEN = "aitrack_" + "d".repeat(64);
    private static final String HMAC_SECRET = EditDtoFactory.DEFAULT_HMAC_SECRET;

    @BeforeEach
    void seedToken() {
        TokenEntity token = new TokenEntity();
        token.setTokenHash(signatureService.sha256Hex(RAW_TOKEN));
        token.setTokenKey(TokenService.computeTokenKey(RAW_TOKEN));
        token.setHmacSecret("plain:" + HMAC_SECRET);
        token.setOwner("test");
        token.setActive(true);
        token.setCreatedAt(Instant.now());
        tokenRepository.save(token);
    }

    private org.springframework.test.web.servlet.request.MockHttpServletRequestBuilder signedUsageRequest(
        String path,
        byte[] body,
        boolean gzip
    ) throws Exception {
        byte[] transmitted = gzip ? gzip(body) : body;
        String ts = String.valueOf(Instant.now().getEpochSecond());
        String sig = signatureService.computeRequestSignature(HMAC_SECRET, ts, transmitted);
        var builder = post(path)
            .header("Authorization", "Bearer " + RAW_TOKEN)
            .header("X-AiTrack-Timestamp", ts)
            .header("X-AiTrack-Signature", sig)
            .header("X-AiTrack-Body-Sha256", signatureService.sha256Hex(transmitted))
            .header("X-AiTrack-Body-Timestamp", Instant.now().toString())
            .contentType(MediaType.APPLICATION_JSON)
            .content(transmitted);
        if (gzip) {
            builder.header("Content-Encoding", "gzip");
        }
        return builder;
    }

    @Test
    void rollup_identityJson_ingestsAndSummarizes() throws Exception {
        byte[] first = objectMapper.writeValueAsBytes(Map.of(
            "items", java.util.List.of(Map.ofEntries(
                Map.entry("device_id", "usage-device-java"),
                Map.entry("day", "2026-06-16"),
                Map.entry("agent", "codex"),
                Map.entry("model", "gpt-5"),
                Map.entry("account", "local"),
                Map.entry("tokens_in", 10),
                Map.entry("tokens_out", 20),
                Map.entry("tokens_cache_read", 3),
                Map.entry("tokens_cache_write", 4),
                Map.entry("tokens_reasoning", 5),
                Map.entry("message_count", 2),
                Map.entry("source_cost", 0.12)
            ))
        ));

        mockMvc.perform(signedUsageRequest("/api/v1/ai-track/usage/rollup", first, false))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.accepted", is(1)));

        byte[] second = objectMapper.writeValueAsBytes(Map.of(
            "items", java.util.List.of(Map.ofEntries(
                Map.entry("device_id", "usage-device-java"),
                Map.entry("day", "2026-06-16"),
                Map.entry("agent", "codex"),
                Map.entry("model", "gpt-5"),
                Map.entry("account", "local"),
                Map.entry("tokens_in", 11),
                Map.entry("tokens_out", 22),
                Map.entry("tokens_cache_read", 0),
                Map.entry("tokens_cache_write", 0),
                Map.entry("tokens_reasoning", 7),
                Map.entry("message_count", 3),
                Map.entry("source_cost", 0.25)
            ))
        ));

        mockMvc.perform(signedUsageRequest("/api/v1/ai-track/usage/rollup", second, false))
            .andExpect(status().isOk());

        mockMvc.perform(get("/api/v1/ai-track/usage/summary")
                .param("agent", "codex")
                .header("Authorization", "Bearer " + RAW_TOKEN))
            .andExpect(status().isOk())
            .andExpect(jsonPath("$.total_tokens", is(40)))
            .andExpect(jsonPath("$.tokens_in", is(11)))
            .andExpect(jsonPath("$.message_count", is(3)))
            .andExpect(jsonPath("$.source_cost", is(0.25)))
            .andExpect(jsonPath("$.items[0].agent", is("codex")));
    }

    @Test
    void rollup_digestMismatch_400() throws Exception {
        byte[] body = objectMapper.writeValueAsBytes(Map.of("items", java.util.List.of()));
        var request = signedUsageRequest("/api/v1/ai-track/usage/rollup", body, false)
            .header("X-AiTrack-Body-Sha256", "deadbeef");
        mockMvc.perform(request)
            .andExpect(status().isBadRequest());
    }

    @Test
    void subscription_gzip_ingests() throws Exception {
        byte[] body = objectMapper.writeValueAsBytes(Map.of(
            "device_id", "usage-device-java",
            "agent", "codex",
            "account", "local",
            "plan", "Pro",
            "quota_session_remaining", 70,
            "quota_weekly_remaining", 80,
            "quota_reset_at", "2026-06-16T10:00:00Z",
            "reader_status", "ok",
            "snapshotted_at", "2026-06-16T09:00:00Z"
        ));

        mockMvc.perform(signedUsageRequest("/api/v1/ai-track/usage/subscription", body, true))
            .andExpect(status().isOk());

        org.assertj.core.api.Assertions.assertThat(subscriptionRepository.findAll()).hasSize(1);
    }

    private static byte[] gzip(byte[] body) throws Exception {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        try (GZIPOutputStream gzip = new GZIPOutputStream(out)) {
            gzip.write(body);
        }
        return out.toByteArray();
    }
}
