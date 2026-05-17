package com.aitrack.server.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Data;

import java.util.List;

/**
 * Explicit pagination response for GET /api/v1/ai-track/edits.
 * Shape: {"total": N, "page": 0, "size": 20, "records": [...]}
 * Matches the Go server's EditQueryResult exactly.
 */
@Data
@AllArgsConstructor
public class EditQueryResult {
    private long total;
    private int page;
    private int size;
    private List<EditRecordView> records;
}
