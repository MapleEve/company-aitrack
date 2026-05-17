package com.aitrack.server.testkit;

import com.aitrack.server.dto.EditBatchResponse;

import java.util.List;

/**
 * Factory for EditBatchResponse (upload response) test instances.
 */
public final class UploadResponseFactory {

    private UploadResponseFactory() {}

    /** All accepted, no rejected/flagged. */
    public static EditBatchResponse buildAllAccepted(int count) {
        return new EditBatchResponse(count, List.of(), List.of());
    }

    /** One rejected at index 0 with the given reason. */
    public static EditBatchResponse buildWithRejected(String reason) {
        return new EditBatchResponse(0,
                List.of(new EditBatchResponse.IndexedReason(0, reason)),
                List.of());
    }

    /** One flagged at index 0. */
    public static EditBatchResponse buildWithFlagged(String reason) {
        return new EditBatchResponse(0,
                List.of(),
                List.of(new EditBatchResponse.IndexedReason(0, reason)));
    }
}
