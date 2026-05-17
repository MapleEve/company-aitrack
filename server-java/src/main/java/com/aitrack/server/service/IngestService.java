package com.aitrack.server.service;

import com.aitrack.server.dto.EditBatchRequest;
import com.aitrack.server.dto.EditBatchResponse;
import com.aitrack.server.dto.EditDto;
import com.aitrack.server.dto.EditQueryResult;
import com.aitrack.server.dto.EditRecordView;
import com.aitrack.server.entity.EditRecordEntity;
import com.aitrack.server.entity.TokenEntity;
import com.aitrack.server.repository.EditRecordRepository;
import lombok.RequiredArgsConstructor;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.Pageable;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Collectors;

@Service
@RequiredArgsConstructor
public class IngestService {

    private final ValidationService validationService;
    private final EditValidator editValidator;
    private final EditRecordRepository editRecordRepository;

    @Transactional
    public EditBatchResponse ingest(TokenEntity token, EditBatchRequest request) {
        List<EditDto> edits = request.getEdits();
        int acceptedCount = 0;
        List<EditBatchResponse.IndexedReason> rejected = new ArrayList<>();
        List<EditBatchResponse.IndexedReason> flagged = new ArrayList<>();

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
                    saveEdit(token, edit, EditRecordEntity.RecordStatus.FLAGGED, result.reasons());
                }
                case ACCEPTED -> {
                    acceptedCount++;
                    saveEdit(token, edit, EditRecordEntity.RecordStatus.ACCEPTED, List.of());
                }
            }
        }

        return new EditBatchResponse(acceptedCount, rejected, flagged);
    }

    public EditQueryResult queryEdits(String tokenKey, String repoUrl, Pageable pageable) {
        Page<EditRecordEntity> page = editRecordRepository.findByFilters(tokenKey, repoUrl, pageable);
        List<EditRecordView> records = page.getContent().stream()
                .map(EditRecordView::from)
                .collect(Collectors.toList());
        return new EditQueryResult(
                page.getTotalElements(),
                page.getNumber(),
                page.getSize(),
                records
        );
    }

    private void saveEdit(TokenEntity token, EditDto edit,
                          EditRecordEntity.RecordStatus status, List<String> flags) {
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
        entity.setDiffHunk(edit.getDiffHunk());
        entity.setMetadata(edit.getMetadata());
        entity.setTimestamp(edit.getTimestamp());
        entity.setRecordSig(edit.getRecordSig());
        entity.setStatus(status);
        entity.setFlags(flags.isEmpty() ? null : String.join(",", flags));
        entity.setReceivedAt(Instant.now());
        editRecordRepository.save(entity);
    }
}
