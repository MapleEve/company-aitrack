package com.aitrack.server.application;

import com.aitrack.server.domain.model.EditBatchRequest;
import com.aitrack.server.domain.model.EditBatchResponse;
import com.aitrack.server.domain.model.EditDto;
import com.aitrack.server.domain.model.EditQueryResult;
import com.aitrack.server.domain.model.EditRecordView;
import com.aitrack.server.domain.model.EditRecordEntity;
import com.aitrack.server.domain.model.PageResult;
import com.aitrack.server.domain.model.TokenEntity;
import com.aitrack.server.domain.port.EditRecordPort;
import com.aitrack.server.domain.service.EditValidator;
import com.aitrack.server.domain.service.ValidationService;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Collectors;

@Service
@RequiredArgsConstructor
public class IngestService {

    private static final int MAX_STORED_DIFF_HUNK_CHARS = 8192;
    private static final int MAX_STORED_METADATA_CHARS = 4096;
    private static final int MAX_STORED_PROMPT_CHARS = 4096;

    private final ValidationService validationService;
    private final EditValidator editValidator;
    private final EditRecordPort editRecordPort;

    @Transactional
    public EditBatchResponse ingest(TokenEntity token, EditBatchRequest request) {
        List<EditDto> edits = request.getEdits();
        int acceptedCount = 0;
        List<EditBatchResponse.IndexedReason> rejected = new ArrayList<>();
        List<EditBatchResponse.IndexedReason> flagged = new ArrayList<>();
        boolean savedAny = false;

        for (int i = 0; i < edits.size(); i++) {
            EditDto edit = edits.get(i);

            // Guard: explicit null/blank check before any unboxing or HMAC computation.
            // Bean Validation is bypassed here because the controller receives raw byte[].
            String malformedReason = editValidator.validate(edit);
            if (malformedReason != null) {
                rejected.add(new EditBatchResponse.IndexedReason(i, malformedReason));
                continue;
            }

            ValidationService.ValidationResult result = validationService.validate(token, edit);

            switch (result.outcome()) {
                case REJECTED -> {
                    rejected.add(new EditBatchResponse.IndexedReason(i, String.join(",", result.reasons())));
                    // Rejected edits are not persisted
                }
                case FLAGGED -> {
                    flagged.add(new EditBatchResponse.IndexedReason(i, String.join(",", result.reasons())));
                    savedAny = saveEdit(token, edit, EditRecordEntity.RecordStatus.FLAGGED, result.reasons()) || savedAny;
                }
                case ACCEPTED -> {
                    acceptedCount++;
                    savedAny = saveEdit(token, edit, EditRecordEntity.RecordStatus.ACCEPTED, List.of()) || savedAny;
                }
            }
        }

        if (savedAny) {
            editRecordPort.applyRawRetention(Instant.now());
        }

        return new EditBatchResponse(acceptedCount, rejected, flagged);
    }

    public EditQueryResult queryEdits(String tokenKey, String repoUrl, int page, int size) {
        PageResult<EditRecordEntity> result = editRecordPort.findByFilters(tokenKey, repoUrl, page, size);
        List<EditRecordView> records = result.content().stream()
                .map(EditRecordView::from)
                .collect(Collectors.toList());
        return new EditQueryResult(
                result.totalElements(),
                page,
                size,
                records
        );
    }

    private boolean saveEdit(TokenEntity token, EditDto edit,
                          EditRecordEntity.RecordStatus status, List<String> flags) {
        if (editRecordPort.existsByRecordSig(edit.getRecordSig())) {
            return false;
        }
        EditRecordEntity entity = new EditRecordEntity();
        entity.setTokenKey(token.getTokenKey());
        entity.setDeviceId(edit.getDeviceId());
        entity.setHostname(edit.getHostname());
        entity.setTool(edit.getTool());
        entity.setToolVersion(edit.getToolVersion());
        entity.setProvider(edit.getProvider());
        entity.setModel(edit.getModel());
        entity.setSessionId(edit.getSessionId());
        entity.setRepoUrl(edit.getRepoUrl());
        entity.setBranch(edit.getBranch());
        entity.setCurrentSha(edit.getCurrentSha());
        entity.setFilePath(edit.getFilePath());
        entity.setAddedLines(edit.getAddedLines());
        entity.setRemovedLines(edit.getRemovedLines());
        entity.setDiffHunk(truncate(edit.getDiffHunk(), MAX_STORED_DIFF_HUNK_CHARS));
        entity.setMetadata(truncate(edit.getMetadata(), MAX_STORED_METADATA_CHARS));
        entity.setTimestamp(edit.getTimestamp());
        entity.setRecordSig(edit.getRecordSig());
        entity.setPromptSummary(truncate(edit.getPromptSummary(), MAX_STORED_PROMPT_CHARS));
        entity.setStatus(status);
        entity.setFlags(flags.isEmpty() ? null : String.join(",", flags));
        entity.setReceivedAt(Instant.now());
        editRecordPort.save(entity);
        return true;
    }

    private static String truncate(String value, int maxChars) {
        if (value == null || value.length() <= maxChars) {
            return value;
        }
        return value.substring(0, maxChars);
    }
}
