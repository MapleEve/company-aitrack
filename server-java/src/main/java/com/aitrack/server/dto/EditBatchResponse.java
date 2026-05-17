package com.aitrack.server.dto;

import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.util.List;

@Data
@NoArgsConstructor
@AllArgsConstructor
public class EditBatchResponse {
    private int accepted;
    private List<IndexedReason> rejected;
    private List<IndexedReason> flagged;

    @Data
    @NoArgsConstructor
    @AllArgsConstructor
    public static class IndexedReason {
        private int index;
        private String reason;
    }
}
