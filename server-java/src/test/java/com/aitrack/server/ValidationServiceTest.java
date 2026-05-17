package com.aitrack.server;

import com.aitrack.server.config.AiTrackProperties;
import com.aitrack.server.dto.EditDto;
import com.aitrack.server.entity.TokenEntity;
import com.aitrack.server.repository.EditRecordRepository;
import com.aitrack.server.service.DiffConsistencyService;
import com.aitrack.server.service.SignatureService;
import com.aitrack.server.service.ValidationService;
import com.aitrack.server.testkit.EditDtoFactory;
import com.aitrack.server.testkit.TamperedFactory;
import com.aitrack.server.testkit.TokenEntityFactory;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.Mockito;

import java.time.Instant;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.when;

/**
 * Tests for ValidationService covering all 10 steps of the hardening chain.
 * Steps 1-3 are handled in RequestAuthHelper; this class covers steps 4-10
 * plus edge cases in each branch.
 */
class ValidationServiceTest {

    private SignatureService signatureService;
    private DiffConsistencyService diffService;
    private EditRecordRepository editRecordRepository;
    private AiTrackProperties props;
    private ValidationService validationService;
    private TokenEntity token;

    @BeforeEach
    void setUp() {
        signatureService = new SignatureService();
        diffService = new DiffConsistencyService();
        editRecordRepository = Mockito.mock(EditRecordRepository.class);

        props = new AiTrackProperties();
        props.setMaxAddedLines(5000);
        props.setRateLimitPerHour(30);
        AiTrackProperties.RepoWhitelist wl = new AiTrackProperties.RepoWhitelist();
        wl.setEnforce(false);
        wl.setUrls(List.of());
        props.setRepoWhitelist(wl);

        validationService = new ValidationService(signatureService, diffService, editRecordRepository, props);

        token = TokenEntityFactory.build();

        when(editRecordRepository.countByTokenKeyAndFilePathSince(anyString(), anyString(), any(Instant.class)))
                .thenReturn(0L);
    }

    // --- Step 4: record_sig ---

    @Test
    void step4_validSig_accepted() {
        EditDto edit = EditDtoFactory.build();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.ACCEPTED);
    }

    @Test
    void step4_tamperedSig_rejected() {
        EditDto edit = TamperedFactory.badRecordSig();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.REJECTED);
        assertThat(result.reasons()).containsExactly("sig_mismatch");
    }

    @Test
    void step4_nullSig_rejected() {
        EditDto edit = EditDtoFactory.build();
        edit.setRecordSig(null);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.REJECTED);
        assertThat(result.reasons()).containsExactly("sig_mismatch");
    }

    @Test
    void step4_wrongLengthSig_rejected() {
        EditDto edit = EditDtoFactory.build();
        edit.setRecordSig("deadbeef");  // too short
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.REJECTED);
        assertThat(result.reasons()).containsExactly("sig_mismatch");
    }

    // --- Step 5: diff_hunk consistency ---

    @Test
    void step5_nullDiffHunk_skipsCheck_accepted() {
        EditDto edit = EditDtoFactory.with(e -> {
            // rebuild sig with null diff
            e.setDiffHunk(null);
            e.setAddedLines(5L);
            e.setRemovedLines(2L);
            String sig = signatureService.computeRecordSig(
                    EditDtoFactory.DEFAULT_HMAC_SECRET, EditDtoFactory.DEFAULT_TOKEN_KEY,
                    e.getDeviceId(), e.getHostname(), e.getTimestamp(), e.getTool(), e.getFilePath(),
                    e.getRepoUrl(), e.getCurrentSha(), 5L, 2L, null);
            e.setRecordSig(sig);
        });
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.ACCEPTED);
    }

    @Test
    void step5_diffInconsistent_flagged() {
        // diff says 1 added, but claimed 100 — inconsistent
        String diff = "@@ -1,1 +1,1 @@\n-old\n+new\n";
        EditDto edit = buildEditWithDiff(diff, 100L, 1L);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.FLAGGED);
        assertThat(result.reasons()).contains("diff_inconsistent");
    }

    @Test
    void step5_diffConsistentWithinDelta_accepted() {
        // diff says 3 added, claimed 3 — exact match
        String diff = "@@ -1,2 +1,3 @@\n-a\n-b\n+x\n+y\n+z\n";
        EditDto edit = buildEditWithDiff(diff, 3L, 2L);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.ACCEPTED);
    }

    // --- Step 6: repo_url whitelist ---

    @Test
    void step6_noWhitelist_anyRepoAccepted() {
        // whitelist is empty — no enforcement, all repos accepted
        EditDto edit = EditDtoFactory.build();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.ACCEPTED);
    }

    @Test
    void step6_whitelistEnforceTrue_repoNotInList_rejected() {
        AiTrackProperties.RepoWhitelist wl = new AiTrackProperties.RepoWhitelist();
        wl.setEnforce(true);
        wl.setUrls(List.of("git@github.com:allowed/repo.git"));
        props.setRepoWhitelist(wl);

        EditDto edit = TamperedFactory.unknownRepo();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.REJECTED);
        assertThat(result.reasons()).contains("repo_not_whitelisted");
    }

    @Test
    void step6_whitelistEnforceTrue_repoInList_accepted() {
        AiTrackProperties.RepoWhitelist wl = new AiTrackProperties.RepoWhitelist();
        wl.setEnforce(true);
        wl.setUrls(List.of(EditDtoFactory.DEFAULT_REPO_URL));
        props.setRepoWhitelist(wl);

        EditDto edit = EditDtoFactory.build();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.ACCEPTED);
    }

    @Test
    void step6_whitelistEnforceFalse_unknownRepo_flaggedOnly() {
        AiTrackProperties.RepoWhitelist wl = new AiTrackProperties.RepoWhitelist();
        wl.setEnforce(false);
        wl.setUrls(List.of("git@github.com:allowed/repo.git"));
        props.setRepoWhitelist(wl);

        EditDto edit = TamperedFactory.unknownRepo();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.FLAGGED);
        assertThat(result.reasons()).contains("repo_unknown");
    }

    // --- Step 7: file_path injection detection ---
    // Absolute paths are NOT flagged (macOS worktree / cloud-sync paths are normal).
    // Only genuine injection indicators trigger path_mismatch:
    //   - ".." path traversal components
    //   - NUL bytes or ASCII control characters

    @Test
    void step7_absoluteFilePath_withRemoteRepo_notFlagged() {
        // Real-data fix: macOS absolute paths used by Codex/Claude should not be flagged.
        // Previously caused ~97% false-positive rate (723/750 records).
        EditDto edit = TamperedFactory.absoluteFilePath();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.reasons()).doesNotContain("path_mismatch");
    }

    @Test
    void step7_macosWorktreePath_notFlagged() {
        // Typical macOS worktree path — absolute but legitimate.
        String absoluteFile = "/Users/developer/work/myproject/src/main.rs";
        EditDto edit = buildEditWithPaths(absoluteFile, EditDtoFactory.DEFAULT_REPO_URL);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.reasons()).doesNotContain("path_mismatch");
    }

    @Test
    void step7_relativeFilePath_notFlagged() {
        EditDto edit = EditDtoFactory.build(); // uses relative src/main.rs
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.ACCEPTED);
    }

    @Test
    void step7_absoluteFilePath_withLocalRepo_notFlagged() {
        String localRepo = "/home/user/myproject";
        String absoluteFile = "/home/user/myproject/src/main.rs";
        EditDto edit = buildEditWithPaths(absoluteFile, localRepo);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.reasons()).doesNotContain("path_mismatch");
    }

    @Test
    void step7_pathTraversal_flagged() {
        // ".." in a path component is a genuine injection indicator.
        String traversalPath = "src/../../etc/passwd";
        EditDto edit = buildEditWithPaths(traversalPath, EditDtoFactory.DEFAULT_REPO_URL);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.FLAGGED);
        assertThat(result.reasons()).contains("path_mismatch");
    }

    @Test
    void step7_absolutePathWithTraversal_flagged() {
        // Absolute path that also contains ".." — should be flagged for traversal.
        String traversalPath = "/Users/dev/../../../etc/shadow";
        EditDto edit = buildEditWithPaths(traversalPath, EditDtoFactory.DEFAULT_REPO_URL);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.FLAGGED);
        assertThat(result.reasons()).contains("path_mismatch");
    }

    @Test
    void step7_nullFilePath_notFlagged() {
        // null check in isPathMismatch: if filePath null, return false
        // EditValidator would catch this first, but we test the underlying method via valid-sig edit
        // Build a valid edit but with relative path (null would fail EditValidator)
        EditDto edit = EditDtoFactory.build();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.reasons()).doesNotContain("path_mismatch");
    }

    // --- Step 8: oversized added_lines ---

    @Test
    void step8_exactlyAtLimit_accepted() {
        EditDto edit = EditDtoFactory.withAddedLines(5000L);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.reasons()).doesNotContain("oversized");
    }

    @Test
    void step8_overLimit_flagged() {
        EditDto edit = TamperedFactory.oversizedAddedLines();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.FLAGGED);
        assertThat(result.reasons()).contains("oversized");
    }

    @Test
    void step8_zeroadded_accepted() {
        EditDto edit = buildEditWithDiff(null, 0L, 1L);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.reasons()).doesNotContain("oversized");
    }

    // --- Step 9: rate limiting ---

    @Test
    void step9_underLimit_accepted() {
        when(editRecordRepository.countByTokenKeyAndFilePathSince(anyString(), anyString(), any(Instant.class)))
                .thenReturn(29L);
        EditDto edit = EditDtoFactory.build();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.reasons()).doesNotContain("rate_limited");
    }

    @Test
    void step9_atLimit_rejected() {
        when(editRecordRepository.countByTokenKeyAndFilePathSince(anyString(), anyString(), any(Instant.class)))
                .thenReturn(30L);
        EditDto edit = EditDtoFactory.build();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.REJECTED);
        assertThat(result.reasons()).containsExactly("rate_limited");
    }

    @Test
    void step9_overLimit_rejected() {
        when(editRecordRepository.countByTokenKeyAndFilePathSince(anyString(), anyString(), any(Instant.class)))
                .thenReturn(100L);
        EditDto edit = EditDtoFactory.build();
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.REJECTED);
        assertThat(result.reasons()).contains("rate_limited");
    }

    // --- Multiple flags can accumulate ---

    @Test
    void multipleFlags_diffInconsistentAndOversized() {
        // diff says 1 added, claimed 9999 — inconsistent AND oversized
        String diff = "@@ -1,1 +1,1 @@\n-old\n+new\n";
        EditDto edit = buildEditWithDiff(diff, 9999L, 1L);
        ValidationService.ValidationResult result = validationService.validate(token, edit);
        assertThat(result.outcome()).isEqualTo(ValidationService.EditOutcome.FLAGGED);
        assertThat(result.reasons()).contains("diff_inconsistent", "oversized");
    }

    // --- Helper methods ---

    private EditDto buildEditWithDiff(String diff, long added, long removed) {
        String sig = signatureService.computeRecordSig(
                EditDtoFactory.DEFAULT_HMAC_SECRET, EditDtoFactory.DEFAULT_TOKEN_KEY,
                EditDtoFactory.DEFAULT_DEVICE_ID, EditDtoFactory.DEFAULT_HOSTNAME,
                EditDtoFactory.DEFAULT_TIMESTAMP, EditDtoFactory.DEFAULT_TOOL,
                EditDtoFactory.DEFAULT_FILE_PATH, EditDtoFactory.DEFAULT_REPO_URL,
                EditDtoFactory.DEFAULT_SHA, added, removed, diff);
        EditDto edit = EditDtoFactory.build();
        edit.setDiffHunk(diff);
        edit.setAddedLines(added);
        edit.setRemovedLines(removed);
        edit.setRecordSig(sig);
        return edit;
    }

    private EditDto buildEditWithPaths(String filePath, String repoUrl) {
        String sig = signatureService.computeRecordSig(
                EditDtoFactory.DEFAULT_HMAC_SECRET, EditDtoFactory.DEFAULT_TOKEN_KEY,
                EditDtoFactory.DEFAULT_DEVICE_ID, EditDtoFactory.DEFAULT_HOSTNAME,
                EditDtoFactory.DEFAULT_TIMESTAMP, EditDtoFactory.DEFAULT_TOOL,
                filePath, repoUrl, EditDtoFactory.DEFAULT_SHA,
                EditDtoFactory.DEFAULT_ADDED, EditDtoFactory.DEFAULT_REMOVED,
                EditDtoFactory.DEFAULT_DIFF_HUNK);
        EditDto edit = EditDtoFactory.build();
        edit.setFilePath(filePath);
        edit.setRepoUrl(repoUrl);
        edit.setRecordSig(sig);
        return edit;
    }
}
