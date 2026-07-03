package com.aitrack.server.application;

import com.aitrack.server.domain.model.EditBatchRequest;
import com.aitrack.server.domain.model.EditBatchResponse;
import com.aitrack.server.domain.model.EditDto;
import com.aitrack.server.domain.model.EditRecordEntity;
import com.aitrack.server.domain.model.TokenEntity;
import com.aitrack.server.domain.port.EditRecordPort;
import com.aitrack.server.domain.service.DiffConsistencyService;
import com.aitrack.server.domain.service.EditValidator;
import com.aitrack.server.domain.service.SignatureService;
import com.aitrack.server.domain.service.ValidationPolicy;
import com.aitrack.server.domain.service.ValidationService;
import com.aitrack.server.testkit.*;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;
import org.mockito.Mockito;

import java.time.Instant;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

class IngestServiceTest {

    private EditRecordPort editRecordRepository;
    private IngestService ingestService;
    private ValidationService validationService;
    private EditValidator editValidator;
    private TokenEntity token;

    @BeforeEach
    void setUp() {
        SignatureService sigService = new SignatureService();
        DiffConsistencyService diffService = new DiffConsistencyService();
        editRecordRepository = Mockito.mock(EditRecordPort.class);

        ValidationPolicy policy = new ValidationPolicy(30L, 5000L, List.of(), false);

        validationService = new ValidationService(sigService, diffService, editRecordRepository, policy);
        editValidator = new EditValidator();
        ingestService = new IngestService(validationService, editValidator, editRecordRepository);

        token = TokenEntityFactory.build();

        when(editRecordRepository.countByTokenKeyAndFilePathSince(anyString(), anyString(), any(Instant.class)))
                .thenReturn(0L);
        when(editRecordRepository.save(any(EditRecordEntity.class)))
                .thenAnswer(inv -> inv.getArgument(0));
    }

    @Test
    void ingest_singleValidEdit_accepted() {
        EditBatchRequest request = EditBatchRequestFactory.build();
        EditBatchResponse response = ingestService.ingest(token, request);

        assertThat(response.getAccepted()).isEqualTo(1);
        assertThat(response.getRejected()).isEmpty();
        assertThat(response.getFlagged()).isEmpty();
        verify(editRecordRepository, times(1)).save(any(EditRecordEntity.class));
    }

    @Test
    void ingest_successfulSave_appliesRawRetention() {
        EditBatchRequest request = EditBatchRequestFactory.build();

        ingestService.ingest(token, request);

        verify(editRecordRepository, times(1)).applyRawRetention(any(Instant.class));
    }

    @Test
    void ingest_multipleValidEdits_allAccepted() {
        List<EditDto> edits = List.of(
                EditDtoFactory.buildForTool("claude"),
                EditDtoFactory.buildForTool("codex"),
                EditDtoFactory.buildForTool("cursor")
        );
        EditBatchRequest request = EditBatchRequestFactory.withEdits(edits);
        EditBatchResponse response = ingestService.ingest(token, request);

        assertThat(response.getAccepted()).isEqualTo(3);
        assertThat(response.getRejected()).isEmpty();
        verify(editRecordRepository, times(3)).save(any(EditRecordEntity.class));
    }

    @Test
    void ingest_malformedEdit_rejected_notPersisted() {
        EditBatchRequest request = EditBatchRequestFactory.withEdits(List.of(TamperedFactory.nullTool()));
        EditBatchResponse response = ingestService.ingest(token, request);

        assertThat(response.getAccepted()).isEqualTo(0);
        assertThat(response.getRejected()).hasSize(1);
        assertThat(response.getRejected().get(0).getReason()).isEqualTo("malformed");
        assertThat(response.getRejected().get(0).getIndex()).isEqualTo(0);
        verify(editRecordRepository, never()).save(any(EditRecordEntity.class));
    }

    @Test
    void ingest_badSig_rejected_notPersisted() {
        EditBatchRequest request = EditBatchRequestFactory.withEdits(List.of(TamperedFactory.badRecordSig()));
        EditBatchResponse response = ingestService.ingest(token, request);

        assertThat(response.getAccepted()).isEqualTo(0);
        assertThat(response.getRejected()).hasSize(1);
        assertThat(response.getRejected().get(0).getReason()).contains("sig_mismatch");
        verify(editRecordRepository, never()).save(any(EditRecordEntity.class));
    }

    @Test
    void ingest_oversizedEdit_flaggedAndPersisted() {
        EditBatchRequest request = EditBatchRequestFactory.withEdits(List.of(TamperedFactory.oversizedAddedLines()));
        EditBatchResponse response = ingestService.ingest(token, request);

        assertThat(response.getAccepted()).isEqualTo(0);
        assertThat(response.getFlagged()).hasSize(1);
        assertThat(response.getFlagged().get(0).getReason()).contains("oversized");
        verify(editRecordRepository, times(1)).save(any(EditRecordEntity.class));
    }

    @Test
    void ingest_mixedBatch_correctCounts() {
        // edit[0] = valid, edit[1] = bad sig, edit[2] = oversized
        List<EditDto> edits = List.of(
                EditDtoFactory.build(),
                TamperedFactory.badRecordSig(),
                TamperedFactory.oversizedAddedLines()
        );
        EditBatchRequest request = EditBatchRequestFactory.withEdits(edits);
        EditBatchResponse response = ingestService.ingest(token, request);

        assertThat(response.getAccepted()).isEqualTo(1);
        assertThat(response.getRejected()).hasSize(1);
        assertThat(response.getRejected().get(0).getIndex()).isEqualTo(1);
        assertThat(response.getFlagged()).hasSize(1);
        assertThat(response.getFlagged().get(0).getIndex()).isEqualTo(2);
    }

    @Test
    void ingest_savedEntity_hasCorrectFields() {
        EditBatchRequest request = EditBatchRequestFactory.build();
        ingestService.ingest(token, request);

        ArgumentCaptor<EditRecordEntity> captor = ArgumentCaptor.forClass(EditRecordEntity.class);
        verify(editRecordRepository).save(captor.capture());
        EditRecordEntity saved = captor.getValue();

        assertThat(saved.getTokenKey()).isEqualTo(TokenEntityFactory.DEFAULT_TOKEN_KEY);
        assertThat(saved.getDeviceId()).isEqualTo(EditDtoFactory.DEFAULT_DEVICE_ID);
        assertThat(saved.getTool()).isEqualTo(EditDtoFactory.DEFAULT_TOOL);
        assertThat(saved.getProvider()).isEqualTo("anthropic");
        assertThat(saved.getFilePath()).isEqualTo(EditDtoFactory.DEFAULT_FILE_PATH);
        assertThat(saved.getAddedLines()).isEqualTo(EditDtoFactory.DEFAULT_ADDED);
        assertThat(saved.getRemovedLines()).isEqualTo(EditDtoFactory.DEFAULT_REMOVED);
        assertThat(saved.getStatus()).isEqualTo(EditRecordEntity.RecordStatus.ACCEPTED);
        assertThat(saved.getFlags()).isNull();
    }

    @Test
    void ingest_flaggedEntity_hasFlags() {
        EditBatchRequest request = EditBatchRequestFactory.withEdits(List.of(TamperedFactory.oversizedAddedLines()));
        ingestService.ingest(token, request);

        ArgumentCaptor<EditRecordEntity> captor = ArgumentCaptor.forClass(EditRecordEntity.class);
        verify(editRecordRepository).save(captor.capture());
        EditRecordEntity saved = captor.getValue();

        assertThat(saved.getStatus()).isEqualTo(EditRecordEntity.RecordStatus.FLAGGED);
        assertThat(saved.getFlags()).contains("oversized");
    }

    @Test
    void ingest_rateLimited_rejected_notPersisted() {
        when(editRecordRepository.countByTokenKeyAndFilePathSince(anyString(), anyString(), any(Instant.class)))
                .thenReturn(30L);

        EditBatchRequest request = EditBatchRequestFactory.build();
        EditBatchResponse response = ingestService.ingest(token, request);

        assertThat(response.getRejected()).hasSize(1);
        assertThat(response.getRejected().get(0).getReason()).contains("rate_limited");
        verify(editRecordRepository, never()).save(any(EditRecordEntity.class));
    }

    @Test
    void ingest_duplicateRecordSig_isAcceptedButNotPersistedAgain() {
        EditDto edit = EditDtoFactory.build();
        when(editRecordRepository.existsByRecordSig(edit.getRecordSig())).thenReturn(true);

        EditBatchResponse response = ingestService.ingest(token, EditBatchRequestFactory.withEdits(List.of(edit)));

        assertThat(response.getAccepted()).isEqualTo(1);
        verify(editRecordRepository, never()).save(any(EditRecordEntity.class));
        verify(editRecordRepository, never()).applyRawRetention(any(Instant.class));
    }

    @Test
    void ingest_savedEntity_truncatesLargeStoredTextFields() {
        String diffHunk = "@@ -1,0 +1,0 @@\n" + " context\n".repeat(9000);
        String metadata = "m".repeat(5000);
        String promptSummary = "p".repeat(5000);
        SignatureService sig = new SignatureService();
        EditDto edit = EditDtoFactory.with(e -> {
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

        ingestService.ingest(token, EditBatchRequestFactory.withEdits(List.of(edit)));

        ArgumentCaptor<EditRecordEntity> captor = ArgumentCaptor.forClass(EditRecordEntity.class);
        verify(editRecordRepository).save(captor.capture());
        EditRecordEntity saved = captor.getValue();
        assertThat(saved.getDiffHunk()).hasSize(8192);
        assertThat(saved.getMetadata()).hasSize(4096);
        assertThat(saved.getPromptSummary()).hasSize(4096);
    }
}
