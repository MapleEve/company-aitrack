package com.aitrack.server.adapter.handler;

import com.aitrack.server.adapter.db.EditRecordRepository;
import com.aitrack.server.adapter.db.TokenRepository;
import com.aitrack.server.application.TokenService;
import com.aitrack.server.domain.model.EditRecordEntity;
import com.aitrack.server.domain.model.TokenEntity;
import com.aitrack.server.domain.service.SignatureService;
import com.aitrack.server.infrastructure.config.AiTrackServerApplication;
import com.aitrack.server.testkit.EditDtoFactory;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.annotation.DirtiesContext;
import org.springframework.test.web.servlet.MockMvc;

import java.time.Instant;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@SpringBootTest(classes = AiTrackServerApplication.class)
@AutoConfigureMockMvc
@DirtiesContext(classMode = DirtiesContext.ClassMode.AFTER_EACH_TEST_METHOD)
class StatsControllerTest {

    @Autowired MockMvc mockMvc;
    @Autowired TokenRepository tokenRepository;
    @Autowired EditRecordRepository editRecordRepository;
    @Autowired SignatureService signatureService;

    private static final String RAW_TOKEN = "aitrack_" + "c".repeat(64);
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

    // --- GET /stats ---

    @Test
    void stats_withValidToken_200() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/stats")
                .header("Authorization", "Bearer " + RAW_TOKEN))
                .andExpect(status().isOk())
                .andExpect(content().contentTypeCompatibleWith("application/json"));
    }

    @Test
    void stats_noToken_401() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/stats"))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void stats_groupByRepo_200() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/stats")
                .param("group_by", "repo")
                .header("Authorization", "Bearer " + RAW_TOKEN))
                .andExpect(status().isOk());
    }

    @Test
    void stats_groupByDevice_200() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/stats")
                .param("group_by", "device")
                .header("Authorization", "Bearer " + RAW_TOKEN))
                .andExpect(status().isOk());
    }

    @Test
    void stats_groupByTool_returnsDynamicToolName() throws Exception {
        seedRecordWithTool("custom-tool-alpha");

        mockMvc.perform(get("/api/v1/ai-track/stats")
                .param("group_by", "tool")
                .header("Authorization", "Bearer " + RAW_TOKEN))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$[0].group").value("custom-tool-alpha"));
    }

    @Test
    void stats_groupByUnknown_200_defaultsToToken() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/stats")
                .param("group_by", "unknown_value")
                .header("Authorization", "Bearer " + RAW_TOKEN))
                .andExpect(status().isOk());
    }

    @Test
    void stats_emptyDb_returnsEmptyArray() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/stats")
                .header("Authorization", "Bearer " + RAW_TOKEN))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$").isArray());
    }

    // --- GET /devices ---

    @Test
    void devices_withValidToken_200() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/devices")
                .header("Authorization", "Bearer " + RAW_TOKEN))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$").isArray());
    }

    @Test
    void devices_noToken_401() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/devices"))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void devices_invalidToken_401() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/devices")
                .header("Authorization", "Bearer aitrack_invalid"))
                .andExpect(status().isUnauthorized());
    }

    private void seedRecordWithTool(String tool) {
        EditRecordEntity record = new EditRecordEntity();
        record.setTokenKey(TokenService.computeTokenKey(RAW_TOKEN));
        record.setDeviceId(EditDtoFactory.DEFAULT_DEVICE_ID);
        record.setHostname(EditDtoFactory.DEFAULT_HOSTNAME);
        record.setTool(tool);
        record.setToolVersion("custom-version");
        record.setProvider("custom-provider");
        record.setSessionId("sess-tool-stats");
        record.setRepoUrl(EditDtoFactory.DEFAULT_REPO_URL);
        record.setBranch("main");
        record.setCurrentSha(EditDtoFactory.DEFAULT_SHA);
        record.setFilePath(EditDtoFactory.DEFAULT_FILE_PATH);
        record.setAddedLines(4L);
        record.setRemovedLines(1L);
        record.setDiffHunk(EditDtoFactory.DEFAULT_DIFF_HUNK);
        record.setTimestamp(EditDtoFactory.DEFAULT_TIMESTAMP);
        record.setRecordSig("a".repeat(64));
        record.setStatus(EditRecordEntity.RecordStatus.ACCEPTED);
        record.setReceivedAt(Instant.now());
        editRecordRepository.save(record);
    }
}
