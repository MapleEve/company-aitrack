package com.aitrack.server.application;

import com.aitrack.server.adapter.db.EditRecordRepository;
import com.aitrack.server.domain.model.EditBatchResponse;
import com.aitrack.server.domain.model.EditDto;
import com.aitrack.server.domain.model.EditRecordEntity;
import com.aitrack.server.domain.service.SignatureService;
import com.aitrack.server.infrastructure.config.AiTrackServerApplication;
import com.aitrack.server.testkit.EditBatchRequestFactory;
import com.aitrack.server.testkit.EditDtoFactory;
import com.aitrack.server.testkit.TokenEntityFactory;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.annotation.DirtiesContext;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

@SpringBootTest(classes = AiTrackServerApplication.class)
@DirtiesContext(classMode = DirtiesContext.ClassMode.AFTER_EACH_TEST_METHOD)
@Transactional
class RepositoryIngestServiceTest {

    @Autowired IngestService ingestService;
    @Autowired EditRecordRepository editRecordRepository;

    @BeforeEach
    void clearRows() {
        editRecordRepository.deleteAll();
    }

    @Test
    void ingestAppliesRawRetentionAfterSuccessfulSaveAndKeepsRecentTruncatedText() {
        Instant now = Instant.now();
        for (int i = 0; i < 12; i++) {
            editRecordRepository.save(seedRecord(
                String.format("%064x", i + 1),
                now.minusSeconds(31L * 24L * 60L * 60L + i),
                "@@ -1 +1 @@\n-old\n+new\n",
                "{\"old\":" + i + "}",
                "old prompt " + i
            ));
        }

        EditDto edit = largeValidEdit();
        EditBatchResponse response = ingestService.ingest(
            TokenEntityFactory.build(),
            EditBatchRequestFactory.withEdits(List.of(edit))
        );
        assertThat(response.getAccepted()).isEqualTo(1);
        assertThat(response.getRejected()).isEmpty();

        EditBatchResponse duplicate = ingestService.ingest(
            TokenEntityFactory.build(),
            EditBatchRequestFactory.withEdits(List.of(edit))
        );
        assertThat(duplicate.getAccepted()).isEqualTo(1);
        assertThat(editRecordRepository.count()).isEqualTo(13);

        for (int i = 0; i < 12; i++) {
            EditRecordEntity oldRecord = editRecordRepository.findByRecordSig(String.format("%064x", i + 1)).orElseThrow();
            assertThat(oldRecord.getDiffHunk()).isEqualTo(EditRecordRepository.RAW_FIELD_STRIPPED_MARKER);
            assertThat(oldRecord.getMetadata()).isEqualTo(EditRecordRepository.RAW_FIELD_STRIPPED_MARKER);
            assertThat(oldRecord.getPromptSummary()).isEqualTo(EditRecordRepository.RAW_FIELD_STRIPPED_MARKER);
            assertThat(oldRecord.getRecordSig()).isNotBlank();
            assertThat(oldRecord.getTokenKey()).isEqualTo(EditDtoFactory.DEFAULT_TOKEN_KEY);
            assertThat(oldRecord.getDeviceId()).isNotBlank();
            assertThat(oldRecord.getTool()).isEqualTo(EditDtoFactory.DEFAULT_TOOL);
            assertThat(oldRecord.getSessionId()).isNotBlank();
            assertThat(oldRecord.getRepoUrl()).isEqualTo(EditDtoFactory.DEFAULT_REPO_URL);
            assertThat(oldRecord.getFilePath()).isEqualTo(EditDtoFactory.DEFAULT_FILE_PATH);
            assertThat(oldRecord.getAddedLines()).isEqualTo(EditDtoFactory.DEFAULT_ADDED);
            assertThat(oldRecord.getRemovedLines()).isEqualTo(EditDtoFactory.DEFAULT_REMOVED);
            assertThat(oldRecord.getStatus()).isEqualTo(EditRecordEntity.RecordStatus.ACCEPTED);
            assertThat(oldRecord.getReceivedAt()).isNotNull();
        }

        EditRecordEntity recent = editRecordRepository.findByRecordSig(edit.getRecordSig()).orElseThrow();
        assertThat(recent.getDiffHunk()).hasSize(8192);
        assertThat(recent.getMetadata()).hasSize(4096);
        assertThat(recent.getPromptSummary()).hasSize(4096);
    }

    @Test
    void repositoryRetentionPolicyStripsRowsOutsideNewestWindowWithoutDeletingRows() {
        Instant now = Instant.now();
        for (int i = 0; i < 5; i++) {
            editRecordRepository.save(seedRecord(
                String.format("%064x", i + 100),
                now.minusSeconds(i),
                "@@ -1 +1 @@\n-old-" + i + "\n+new-" + i + "\n",
                "{\"window\":" + i + "}",
                "prompt window " + i
            ));
        }

        int stripped = editRecordRepository.applyRawRetentionWithPolicy(now, 365, 2);

        assertThat(stripped).isEqualTo(3);
        assertThat(editRecordRepository.count()).isEqualTo(5);
        for (int i = 0; i < 5; i++) {
            EditRecordEntity record = editRecordRepository.findByRecordSig(String.format("%064x", i + 100)).orElseThrow();
            if (i < 2) {
                assertThat(record.getDiffHunk()).isNotEqualTo(EditRecordRepository.RAW_FIELD_STRIPPED_MARKER);
                assertThat(record.getMetadata()).isNotEqualTo(EditRecordRepository.RAW_FIELD_STRIPPED_MARKER);
                assertThat(record.getPromptSummary()).isNotEqualTo(EditRecordRepository.RAW_FIELD_STRIPPED_MARKER);
            } else {
                assertThat(record.getDiffHunk()).isEqualTo(EditRecordRepository.RAW_FIELD_STRIPPED_MARKER);
                assertThat(record.getMetadata()).isEqualTo(EditRecordRepository.RAW_FIELD_STRIPPED_MARKER);
                assertThat(record.getPromptSummary()).isEqualTo(EditRecordRepository.RAW_FIELD_STRIPPED_MARKER);
                assertThat(record.getRecordSig()).isNotBlank();
                assertThat(record.getAddedLines()).isEqualTo(EditDtoFactory.DEFAULT_ADDED);
                assertThat(record.getRemovedLines()).isEqualTo(EditDtoFactory.DEFAULT_REMOVED);
            }
        }
    }

    private static EditDto largeValidEdit() {
        String diffHunk = "@@ -1,0 +1,0 @@\n" + " context\n".repeat(9000);
        String metadata = "m".repeat(5000);
        String promptSummary = "p".repeat(5000);
        SignatureService sig = new SignatureService();
        return EditDtoFactory.with(e -> {
            e.setAddedLines(0L);
            e.setRemovedLines(0L);
            e.setDiffHunk(diffHunk);
            e.setMetadata(metadata);
            e.setPromptSummary(promptSummary);
            e.setRecordSig(sig.computeRecordSig(
                EditDtoFactory.DEFAULT_HMAC_SECRET,
                EditDtoFactory.DEFAULT_TOKEN_KEY,
                EditDtoFactory.DEFAULT_DEVICE_ID,
                EditDtoFactory.DEFAULT_HOSTNAME,
                EditDtoFactory.DEFAULT_TIMESTAMP,
                EditDtoFactory.DEFAULT_TOOL,
                EditDtoFactory.DEFAULT_FILE_PATH,
                EditDtoFactory.DEFAULT_REPO_URL,
                EditDtoFactory.DEFAULT_SHA,
                0L,
                0L,
                diffHunk
            ));
        });
    }

    private static EditRecordEntity seedRecord(
        String recordSig,
        Instant receivedAt,
        String diffHunk,
        String metadata,
        String promptSummary
    ) {
        EditRecordEntity entity = new EditRecordEntity();
        entity.setTokenKey(EditDtoFactory.DEFAULT_TOKEN_KEY);
        entity.setDeviceId(EditDtoFactory.DEFAULT_DEVICE_ID + "-" + recordSig.substring(0, 4));
        entity.setHostname(EditDtoFactory.DEFAULT_HOSTNAME);
        entity.setTool(EditDtoFactory.DEFAULT_TOOL);
        entity.setToolVersion("claude-code");
        entity.setProvider("anthropic");
        entity.setSessionId("sess-" + recordSig.substring(0, 8));
        entity.setRepoUrl(EditDtoFactory.DEFAULT_REPO_URL);
        entity.setBranch("main");
        entity.setCurrentSha(EditDtoFactory.DEFAULT_SHA);
        entity.setFilePath(EditDtoFactory.DEFAULT_FILE_PATH);
        entity.setAddedLines(EditDtoFactory.DEFAULT_ADDED);
        entity.setRemovedLines(EditDtoFactory.DEFAULT_REMOVED);
        entity.setDiffHunk(diffHunk);
        entity.setMetadata(metadata);
        entity.setTimestamp(EditDtoFactory.DEFAULT_TIMESTAMP);
        entity.setRecordSig(recordSig);
        entity.setPromptSummary(promptSummary);
        entity.setStatus(EditRecordEntity.RecordStatus.ACCEPTED);
        entity.setFlags(null);
        entity.setReceivedAt(receivedAt);
        return entity;
    }
}
