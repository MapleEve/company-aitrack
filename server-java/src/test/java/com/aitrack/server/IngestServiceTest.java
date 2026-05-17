package com.aitrack.server;

import com.aitrack.server.dto.EditBatchRequest;
import com.aitrack.server.dto.EditBatchResponse;
import com.aitrack.server.dto.EditDto;
import com.aitrack.server.entity.EditRecordEntity;
import com.aitrack.server.entity.TokenEntity;
import com.aitrack.server.repository.EditRecordRepository;
import com.aitrack.server.service.*;
import com.aitrack.server.config.AiTrackProperties;
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

    private EditRecordRepository editRecordRepository;
    private IngestService ingestService;
    private ValidationService validationService;
    private EditValidator editValidator;
    private TokenEntity token;

    @BeforeEach
    void setUp() {
        SignatureService sigService = new SignatureService();
        DiffConsistencyService diffService = new DiffConsistencyService();
        editRecordRepository = Mockito.mock(EditRecordRepository.class);

        AiTrackProperties props = new AiTrackProperties();
        props.setMaxAddedLines(5000);
        props.setRateLimitPerHour(30);
        AiTrackProperties.RepoWhitelist wl = new AiTrackProperties.RepoWhitelist();
        wl.setEnforce(false);
        wl.setUrls(List.of());
        props.setRepoWhitelist(wl);

        validationService = new ValidationService(sigService, diffService, editRecordRepository, props);
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
}
