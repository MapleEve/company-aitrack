package com.aitrack.server.controller;

import com.aitrack.server.config.RequestAuthHelper;
import com.aitrack.server.dto.EditBatchRequest;
import com.aitrack.server.dto.EditBatchResponse;
import com.aitrack.server.dto.EditQueryResult;
import com.aitrack.server.entity.TokenEntity;
import com.aitrack.server.service.IngestService;
import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

import java.io.IOException;

@RestController
@RequestMapping("/api/v1/ai-track")
@RequiredArgsConstructor
public class EditsController {

    private final RequestAuthHelper authHelper;
    private final IngestService ingestService;
    private final ObjectMapper objectMapper;

    @PostMapping("/edits")
    public ResponseEntity<EditBatchResponse> submitEdits(
        HttpServletRequest httpRequest,
        @RequestBody byte[] rawBody
    ) throws IOException {
        // Steps 1-3: token auth + timestamp + signature
        TokenEntity token = authHelper.resolveToken(httpRequest);
        authHelper.validateRequestSignature(httpRequest, token, rawBody);

        // Deserialize after signature check to ensure body integrity
        EditBatchRequest request = objectMapper.readValue(rawBody, EditBatchRequest.class);

        // Guard: edits array must be present and non-empty
        if (request.getEdits() == null || request.getEdits().isEmpty()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "edits array is required and must not be empty");
        }

        // Steps 4-10 per edit, then persist
        EditBatchResponse response = ingestService.ingest(token, request);
        return ResponseEntity.ok(response);
    }

    /**
     * Paginated edit query endpoint.
     * Returns {"total": N, "page": P, "size": S, "records": [...]} — matches Go server shape.
     */
    @GetMapping("/edits")
    public ResponseEntity<EditQueryResult> queryEdits(
        HttpServletRequest httpRequest,
        @RequestParam(required = false) String token_key,
        @RequestParam(required = false) String repo,
        @RequestParam(defaultValue = "0") int page,
        @RequestParam(defaultValue = "20") int size
    ) {
        // Auth: token must be valid, but no signature required for GET
        authHelper.resolveToken(httpRequest);
        var pageable = org.springframework.data.domain.PageRequest.of(
            Math.max(0, page), Math.min(100, Math.max(1, size)),
            org.springframework.data.domain.Sort.by("receivedAt").descending()
        );
        EditQueryResult result = ingestService.queryEdits(token_key, repo, pageable);
        return ResponseEntity.ok(result);
    }
}
