package com.aitrack.server;

import com.aitrack.server.config.AiTrackProperties;
import com.aitrack.server.dto.EditDto;
import com.aitrack.server.entity.TokenEntity;
import com.aitrack.server.repository.EditRecordRepository;
import com.aitrack.server.service.DiffConsistencyService;
import com.aitrack.server.service.SignatureService;
import com.aitrack.server.service.ValidationService;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.Mockito;

import java.time.Instant;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.*;
import static org.mockito.Mockito.when;

class ValidationChainTest {

    private SignatureService signatureService;
    private DiffConsistencyService diffService;
    private EditRecordRepository editRecordRepository;
    private AiTrackProperties props;
    private ValidationService validationService;

    private TokenEntity token;
    private static final String HMAC_SECRET = "testsecret";
    private static final String TOKEN_KEY = "abcdef…7890";

    @BeforeEach
    void setUp() {
        signatureService = new SignatureService();
        diffService = new DiffConsistencyService();
        editRecordRepository = Mockito.mock(EditRecordRepository.class);
        props = new AiTrackProperties();
        props.setMaxAddedLines(5000);
        props.setRateLimitPerHour(30);
        props.setRepoWhitelist(new AiTrackProperties.RepoWhitelist());

        validationService = new ValidationService(signatureService, diffService, editRecordRepository, props);

        token = new TokenEntity();
        token.setHmacSecret(HMAC_SECRET);
        token.setTokenKey(TOKEN_KEY);

        // Default: under rate limit
        when(editRecordRepository.countByTokenKeyAndFilePathSince(anyString(), anyString(), any(Instant.class)))
            .thenReturn(0L);
    }

    private static final String DEVICE_ID = "device-001";
    private static final String HOSTNAME = "test-host.local";

    private EditDto makeValidEdit(String diffHunk, long added, long removed) {
        EditDto edit = new EditDto();
        edit.setTool("claude");
        edit.setProvider("anthropic");
        edit.setSessionId("sess-001");
        edit.setRepoUrl("git@github.com:org/repo.git");
        edit.setBranch("main");
        edit.setCurrentSha("abc123");
        edit.setFilePath("src/main.rs");
        edit.setAddedLines(added);
        edit.setRemovedLines(removed);
        edit.setDiffHunk(diffHunk);
        edit.setTimestamp("2026-05-17T10:00:00Z");
        edit.setDeviceId(DEVICE_ID);
        edit.setHostname(HOSTNAME);

        String sig = signatureService.computeRecordSig(
            HMAC_SECRET, TOKEN_KEY, DEVICE_ID, HOSTNAME, "2026-05-17T10:00:00Z",
            "claude", "src/main.rs", "git@github.com:org/repo.git", "abc123",
            added, removed, diffHunk
        );
        edit.setRecordSig(sig);
        return edit;
    }

    @Test
    void step4_validSignature_passes() {
        EditDto edit = makeValidEdit("@@ -1,1 +1,1 @@\n-old\n+new\n", 1, 1);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.ACCEPTED);
    }

    @Test
    void step4_tampered_sig_rejected() {
        EditDto edit = makeValidEdit(null, 5, 3);
        edit.setRecordSig("0".repeat(64));  // wrong sig
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.REJECTED);
        assertThat(result.reasons()).contains("sig_mismatch");
    }

    @Test
    void step5_diffInconsistent_flagged() {
        // diff claims 1 added and 1 removed, but passes added=100
        String diff = "@@ -1,1 +1,1 @@\n-old\n+new\n";
        EditDto edit = makeValidEdit(diff, 100, 1);  // sig computed with 100
        // sig is valid but diff says only 1 added
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.FLAGGED);
        assertThat(result.reasons()).contains("diff_inconsistent");
    }

    @Test
    void step8_oversized_flagged() {
        // added_lines > 5000
        EditDto edit = makeValidEdit(null, 5001, 0);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.FLAGGED);
        assertThat(result.reasons()).contains("oversized");
    }

    @Test
    void step9_rateLimited_rejected() {
        when(editRecordRepository.countByTokenKeyAndFilePathSince(anyString(), anyString(), any(Instant.class)))
            .thenReturn(30L);  // at threshold

        EditDto edit = makeValidEdit(null, 5, 3);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.REJECTED);
        assertThat(result.reasons()).contains("rate_limited");
    }

    @Test
    void step7_absoluteFilePath_notFlagged() {
        // Absolute paths are normal (macOS worktrees, cloud-synced dirs) and
        // are NOT a path-injection signal — they must not be flagged.
        EditDto edit = new EditDto();
        edit.setTool("claude");
        edit.setProvider("anthropic");
        edit.setSessionId("sess-001");
        edit.setRepoUrl("git@github.com:org/repo.git");
        edit.setBranch("main");
        edit.setCurrentSha("abc123");
        edit.setFilePath("/Users/dev/work/repo/src/file.rs");  // absolute, no traversal
        edit.setAddedLines(1L);
        edit.setRemovedLines(0L);
        edit.setDiffHunk(null);
        edit.setTimestamp("2026-05-17T10:00:00Z");
        edit.setDeviceId(DEVICE_ID);
        edit.setHostname(HOSTNAME);

        String sig = signatureService.computeRecordSig(
            HMAC_SECRET, TOKEN_KEY, DEVICE_ID, HOSTNAME, "2026-05-17T10:00:00Z",
            "claude", "/Users/dev/work/repo/src/file.rs", "git@github.com:org/repo.git",
            "abc123", 1, 0, null
        );
        edit.setRecordSig(sig);

        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.reasons()).doesNotContain("path_mismatch");
    }

    @Test
    void step7_pathTraversal_flagged() {
        // A ".." traversal sequence is a genuine injection indicator.
        EditDto edit = new EditDto();
        edit.setTool("claude");
        edit.setProvider("anthropic");
        edit.setSessionId("sess-001");
        edit.setRepoUrl("git@github.com:org/repo.git");
        edit.setBranch("main");
        edit.setCurrentSha("abc123");
        edit.setFilePath("../../etc/passwd");  // path traversal
        edit.setAddedLines(1L);
        edit.setRemovedLines(0L);
        edit.setDiffHunk(null);
        edit.setTimestamp("2026-05-17T10:00:00Z");
        edit.setDeviceId(DEVICE_ID);
        edit.setHostname(HOSTNAME);

        String sig = signatureService.computeRecordSig(
            HMAC_SECRET, TOKEN_KEY, DEVICE_ID, HOSTNAME, "2026-05-17T10:00:00Z",
            "claude", "../../etc/passwd", "git@github.com:org/repo.git",
            "abc123", 1, 0, null
        );
        edit.setRecordSig(sig);

        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.FLAGGED);
        assertThat(result.reasons()).contains("path_mismatch");
    }
}
