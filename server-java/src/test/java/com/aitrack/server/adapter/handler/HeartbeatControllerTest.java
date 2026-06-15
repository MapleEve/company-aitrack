package com.aitrack.server.adapter.handler;

import com.aitrack.server.adapter.db.TokenRepository;
import com.aitrack.server.adapter.db.DeviceRepository;
import com.aitrack.server.application.TokenService;
import com.aitrack.server.domain.model.HeartbeatRequest;
import com.aitrack.server.domain.model.TokenEntity;
import com.aitrack.server.domain.service.SignatureService;
import com.aitrack.server.infrastructure.config.AiTrackServerApplication;
import com.aitrack.server.testkit.EditDtoFactory;
import com.aitrack.server.testkit.HeartbeatRequestFactory;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.test.annotation.DirtiesContext;
import org.springframework.test.web.servlet.MockMvc;

import java.time.Instant;
import java.util.LinkedHashMap;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@SpringBootTest(classes = AiTrackServerApplication.class)
@AutoConfigureMockMvc
@DirtiesContext(classMode = DirtiesContext.ClassMode.AFTER_EACH_TEST_METHOD)
class HeartbeatControllerTest {

    @Autowired MockMvc mockMvc;
    @Autowired ObjectMapper objectMapper;
    @Autowired TokenRepository tokenRepository;
    @Autowired DeviceRepository deviceRepository;
    @Autowired SignatureService signatureService;

    private static final String RAW_TOKEN = "aitrack_" + "b".repeat(64);
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

    private org.springframework.test.web.servlet.ResultActions postHeartbeat(Object body) throws Exception {
        byte[] bodyBytes = objectMapper.writeValueAsBytes(body);
        String ts = String.valueOf(Instant.now().getEpochSecond());
        String sig = signatureService.computeRequestSignature(HMAC_SECRET, ts, bodyBytes);
        return mockMvc.perform(post("/api/v1/ai-track/heartbeat")
                .header("Authorization", "Bearer " + RAW_TOKEN)
                .header("X-AiTrack-Timestamp", ts)
                .header("X-AiTrack-Signature", sig)
                .contentType(MediaType.APPLICATION_JSON)
                .content(bodyBytes));
    }

    @Test
    void validHeartbeat_200_ok() throws Exception {
        postHeartbeat(HeartbeatRequestFactory.build())
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.ok").value(true));
    }

    @Test
    void missingAuthHeader_401() throws Exception {
        byte[] body = objectMapper.writeValueAsBytes(HeartbeatRequestFactory.build());
        mockMvc.perform(post("/api/v1/ai-track/heartbeat")
                .contentType(MediaType.APPLICATION_JSON)
                .content(body))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void invalidSignature_401() throws Exception {
        byte[] body = objectMapper.writeValueAsBytes(HeartbeatRequestFactory.build());
        String ts = String.valueOf(Instant.now().getEpochSecond());
        mockMvc.perform(post("/api/v1/ai-track/heartbeat")
                .header("Authorization", "Bearer " + RAW_TOKEN)
                .header("X-AiTrack-Timestamp", ts)
                .header("X-AiTrack-Signature", "badsig")
                .contentType(MediaType.APPLICATION_JSON)
                .content(body))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void missingDeviceId_400() throws Exception {
        HeartbeatRequest req = HeartbeatRequestFactory.build();
        req.setDeviceId(null);
        postHeartbeat(req)
                .andExpect(status().isBadRequest());
    }

    @Test
    void blankDeviceId_400() throws Exception {
        HeartbeatRequest req = HeartbeatRequestFactory.build();
        req.setDeviceId("  ");
        postHeartbeat(req)
                .andExpect(status().isBadRequest());
    }

    @Test
    void heartbeat_allHooksOff_200() throws Exception {
        postHeartbeat(HeartbeatRequestFactory.buildAllHooksOff())
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.ok").value(true));
    }

    @Test
    void heartbeat_arbitraryHookKeys_persistsHooksJson() throws Exception {
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("device_id", "device-dynamic-hooks");
        body.put("hostname", EditDtoFactory.DEFAULT_HOSTNAME);
        body.put("token_key_masked", EditDtoFactory.DEFAULT_TOKEN_KEY);
        body.put("client_version", "1.0.0");
        body.put("ts", 1715940000L);
        body.put("pending_count", 3);

        Map<String, Boolean> hooks = new LinkedHashMap<>();
        hooks.put("claude", true);
        hooks.put("opencode", true);
        hooks.put("trae", false);
        body.put("hooks", hooks);

        postHeartbeat(body)
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.ok").value(true));

        String hooksJson = deviceRepository.findByDeviceId("device-dynamic-hooks")
                .orElseThrow()
                .getHooksJson();
        assertThat(hooksJson).contains("\"claude\":true");
        assertThat(hooksJson).contains("\"opencode\":true");
        assertThat(hooksJson).contains("\"trae\":false");
        assertThat(hooksJson).doesNotContain("\"codex\"");
        assertThat(hooksJson).doesNotContain("\"cursor\"");
    }

    @Test
    void heartbeat_secondCall_updatesDevice() throws Exception {
        // Two heartbeats for same device — both should succeed
        postHeartbeat(HeartbeatRequestFactory.build())
                .andExpect(status().isOk());
        postHeartbeat(HeartbeatRequestFactory.build())
                .andExpect(status().isOk());
    }

    @Test
    void oversizedBody_413() throws Exception {
        // Body larger than the 8 MiB max-request-body-bytes limit
        byte[] hugeBody = new byte[9_437_184];
        java.util.Arrays.fill(hugeBody, (byte) 'x');
        String ts = String.valueOf(Instant.now().getEpochSecond());
        mockMvc.perform(post("/api/v1/ai-track/heartbeat")
                .header("Authorization", "Bearer " + RAW_TOKEN)
                .header("X-AiTrack-Timestamp", ts)
                .header("X-AiTrack-Signature", "irrelevant")
                .contentType(org.springframework.http.MediaType.APPLICATION_JSON)
                .content(hugeBody))
                .andExpect(status().isPayloadTooLarge());
    }
}
