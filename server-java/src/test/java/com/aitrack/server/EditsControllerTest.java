package com.aitrack.server;

import com.aitrack.server.dto.EditBatchRequest;
import com.aitrack.server.dto.EditDto;
import com.aitrack.server.entity.TokenEntity;
import com.aitrack.server.repository.TokenRepository;
import com.aitrack.server.service.SignatureService;
import com.aitrack.server.testkit.EditBatchRequestFactory;
import com.aitrack.server.testkit.EditDtoFactory;
import com.aitrack.server.testkit.TamperedFactory;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.http.MediaType;
import org.springframework.test.annotation.DirtiesContext;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.transaction.annotation.Transactional;

import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.List;

import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.*;

@SpringBootTest
@AutoConfigureMockMvc
@DirtiesContext(classMode = DirtiesContext.ClassMode.AFTER_EACH_TEST_METHOD)
class EditsControllerTest {

    @Autowired MockMvc mockMvc;
    @Autowired ObjectMapper objectMapper;
    @Autowired TokenRepository tokenRepository;
    @Autowired SignatureService signatureService;

    private static final String RAW_TOKEN = "aitrack_" + "a".repeat(64);
    private static final String HMAC_SECRET = EditDtoFactory.DEFAULT_HMAC_SECRET;

    @BeforeEach
    void seedToken() {
        TokenEntity token = new TokenEntity();
        token.setTokenHash(signatureService.sha256Hex(RAW_TOKEN));
        token.setTokenKey(com.aitrack.server.service.TokenService.computeTokenKey(RAW_TOKEN));
        // Store hmac_secret as "plain:" prefix (no encryption key configured in tests)
        token.setHmacSecret("plain:" + HMAC_SECRET);
        token.setOwner("test");
        token.setActive(true);
        token.setCreatedAt(Instant.now());
        tokenRepository.save(token);
    }

    private String validTokenKey() {
        return com.aitrack.server.service.TokenService.computeTokenKey(RAW_TOKEN);
    }

    private byte[] toBytes(Object obj) throws Exception {
        return objectMapper.writeValueAsBytes(obj);
    }

    private String makeRequestSig(byte[] body) {
        String ts = String.valueOf(Instant.now().getEpochSecond());
        return ts + "|" + signatureService.computeRequestSignature(HMAC_SECRET, ts, body);
    }

    /** Posts to /api/v1/ai-track/edits with proper auth headers. */
    private org.springframework.test.web.servlet.ResultActions postEdits(byte[] body) throws Exception {
        String ts = String.valueOf(Instant.now().getEpochSecond());
        String sig = signatureService.computeRequestSignature(HMAC_SECRET, ts, body);
        return mockMvc.perform(post("/api/v1/ai-track/edits")
                .header("Authorization", "Bearer " + RAW_TOKEN)
                .header("X-AiTrack-Timestamp", ts)
                .header("X-AiTrack-Signature", sig)
                .contentType(MediaType.APPLICATION_JSON)
                .content(body));
    }

    // --- Auth (steps 1-3) ---

    @Test
    void missingAuthHeader_401() throws Exception {
        byte[] body = toBytes(EditBatchRequestFactory.build());
        mockMvc.perform(post("/api/v1/ai-track/edits")
                .contentType(MediaType.APPLICATION_JSON)
                .content(body))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void invalidToken_401() throws Exception {
        byte[] body = toBytes(EditBatchRequestFactory.build());
        String ts = String.valueOf(Instant.now().getEpochSecond());
        mockMvc.perform(post("/api/v1/ai-track/edits")
                .header("Authorization", "Bearer aitrack_invalid_token")
                .header("X-AiTrack-Timestamp", ts)
                .header("X-AiTrack-Signature", "fakesig")
                .contentType(MediaType.APPLICATION_JSON)
                .content(body))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void missingTimestampHeader_401() throws Exception {
        byte[] body = toBytes(EditBatchRequestFactory.build());
        mockMvc.perform(post("/api/v1/ai-track/edits")
                .header("Authorization", "Bearer " + RAW_TOKEN)
                .contentType(MediaType.APPLICATION_JSON)
                .content(body))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void invalidSignature_401() throws Exception {
        byte[] body = toBytes(EditBatchRequestFactory.build());
        String ts = String.valueOf(Instant.now().getEpochSecond());
        mockMvc.perform(post("/api/v1/ai-track/edits")
                .header("Authorization", "Bearer " + RAW_TOKEN)
                .header("X-AiTrack-Timestamp", ts)
                .header("X-AiTrack-Signature", "0".repeat(64))
                .contentType(MediaType.APPLICATION_JSON)
                .content(body))
                .andExpect(status().isUnauthorized());
    }

    @Test
    void expiredTimestamp_401() throws Exception {
        byte[] body = toBytes(EditBatchRequestFactory.build());
        // timestamp 10 minutes ago — outside the 300-second window
        String ts = String.valueOf(Instant.now().getEpochSecond() - 600);
        String sig = signatureService.computeRequestSignature(HMAC_SECRET, ts, body);
        mockMvc.perform(post("/api/v1/ai-track/edits")
                .header("Authorization", "Bearer " + RAW_TOKEN)
                .header("X-AiTrack-Timestamp", ts)
                .header("X-AiTrack-Signature", sig)
                .contentType(MediaType.APPLICATION_JSON)
                .content(body))
                .andExpect(status().isUnauthorized());
    }

    // --- Request-level validation ---

    @Test
    void emptyEditsArray_400() throws Exception {
        byte[] body = toBytes(TamperedFactory.emptyEdits());
        postEdits(body).andExpect(status().isBadRequest());
    }

    // --- Valid edit submission ---

    @Test
    void validSingleEdit_200_accepted() throws Exception {
        // Build an EditDto whose record_sig uses the seeded token's key and hmac_secret
        String tokenKey = validTokenKey();
        EditDto edit = buildSignedEdit(tokenKey);
        EditBatchRequest req = EditBatchRequestFactory.withEdits(List.of(edit));
        byte[] body = toBytes(req);

        postEdits(body)
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.accepted").value(1))
                .andExpect(jsonPath("$.rejected").isEmpty())
                .andExpect(jsonPath("$.flagged").isEmpty());
    }

    @Test
    void editWithBadRecordSig_200_rejected() throws Exception {
        EditDto edit = EditDtoFactory.build();
        edit.setRecordSig("0".repeat(64));
        EditBatchRequest req = EditBatchRequestFactory.withEdits(List.of(edit));
        byte[] body = toBytes(req);

        postEdits(body)
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.accepted").value(0))
                .andExpect(jsonPath("$.rejected[0].reason").value("sig_mismatch"));
    }

    @Test
    void editWithMissingRequiredField_200_malformed() throws Exception {
        EditDto edit = TamperedFactory.nullTool();
        EditBatchRequest req = EditBatchRequestFactory.withEdits(List.of(edit));
        byte[] body = toBytes(req);

        postEdits(body)
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.rejected[0].reason").value("malformed"));
    }

    // --- GET /edits ---

    @Test
    void getEdits_withValidToken_200_snakeCaseShape() throws Exception {
        // First ingest one edit so records list is non-empty
        String tokenKey = validTokenKey();
        EditDto edit = buildSignedEdit(tokenKey);
        EditBatchRequest req = EditBatchRequestFactory.withEdits(List.of(edit));
        postEdits(toBytes(req));

        String ts = String.valueOf(Instant.now().getEpochSecond());
        mockMvc.perform(get("/api/v1/ai-track/edits?page=0&size=10")
                .header("Authorization", "Bearer " + RAW_TOKEN)
                .header("X-AiTrack-Timestamp", ts))
                .andExpect(status().isOk())
                // Pagination envelope: {total, page, size, records}
                .andExpect(jsonPath("$.total").exists())
                .andExpect(jsonPath("$.page").value(0))
                .andExpect(jsonPath("$.size").value(10))
                .andExpect(jsonPath("$.records").isArray())
                // First record uses snake_case keys
                .andExpect(jsonPath("$.records[0].file_path").exists())
                .andExpect(jsonPath("$.records[0].added_lines").exists())
                .andExpect(jsonPath("$.records[0].token_key").exists())
                .andExpect(jsonPath("$.records[0].received_at").exists());
    }

    @Test
    void getEdits_emptyDb_hasZeroTotal() throws Exception {
        String ts = String.valueOf(Instant.now().getEpochSecond());
        mockMvc.perform(get("/api/v1/ai-track/edits")
                .header("Authorization", "Bearer " + RAW_TOKEN)
                .header("X-AiTrack-Timestamp", ts))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.total").value(0))
                .andExpect(jsonPath("$.records").isArray());
    }

    @Test
    void getEdits_noToken_401() throws Exception {
        mockMvc.perform(get("/api/v1/ai-track/edits"))
                .andExpect(status().isUnauthorized());
    }

    // --- Helper ---

    private EditDto buildSignedEdit(String tokenKey) {
        SignatureService sig = new SignatureService();
        String ts = EditDtoFactory.DEFAULT_TIMESTAMP;
        String deviceId = EditDtoFactory.DEFAULT_DEVICE_ID;
        String tool = EditDtoFactory.DEFAULT_TOOL;
        String filePath = EditDtoFactory.DEFAULT_FILE_PATH;
        String repoUrl = EditDtoFactory.DEFAULT_REPO_URL;
        String sha = EditDtoFactory.DEFAULT_SHA;
        long added = EditDtoFactory.DEFAULT_ADDED;
        long removed = EditDtoFactory.DEFAULT_REMOVED;
        String diff = EditDtoFactory.DEFAULT_DIFF_HUNK;

        String hostname = EditDtoFactory.DEFAULT_HOSTNAME;

        EditDto edit = new EditDto();
        edit.setTool(tool);
        edit.setToolVersion("claude-code");
        edit.setProvider("anthropic");
        edit.setSessionId("sess-test-001");
        edit.setRepoUrl(repoUrl);
        edit.setBranch("main");
        edit.setCurrentSha(sha);
        edit.setFilePath(filePath);
        edit.setAddedLines(added);
        edit.setRemovedLines(removed);
        edit.setDiffHunk(diff);
        edit.setTimestamp(ts);
        edit.setDeviceId(deviceId);
        edit.setHostname(hostname);

        String recordSig = sig.computeRecordSig(HMAC_SECRET, tokenKey, deviceId, hostname, ts,
                tool, filePath, repoUrl, sha, added, removed, diff);
        edit.setRecordSig(recordSig);
        return edit;
    }
}
