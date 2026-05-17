package com.aitrack.server.testkit;

import com.aitrack.server.dto.EditBatchRequest;
import com.aitrack.server.dto.EditDto;

import java.util.List;
import java.util.function.Consumer;

/**
 * Deterministic factory for EditBatchRequest test instances.
 */
public final class EditBatchRequestFactory {

    private EditBatchRequestFactory() {}

    /** Single-edit batch with all-valid defaults. */
    public static EditBatchRequest build() {
        EditBatchRequest req = new EditBatchRequest();
        req.setDeviceId(EditDtoFactory.DEFAULT_DEVICE_ID);
        req.setClientVersion("1.0.0");
        req.setEdits(List.of(EditDtoFactory.build()));
        return req;
    }

    /** Batch with a custom list of edits. */
    public static EditBatchRequest withEdits(List<EditDto> edits) {
        EditBatchRequest req = new EditBatchRequest();
        req.setDeviceId(EditDtoFactory.DEFAULT_DEVICE_ID);
        req.setClientVersion("1.0.0");
        req.setEdits(edits);
        return req;
    }

    /** Builder-style customisation. */
    public static EditBatchRequest with(Consumer<EditBatchRequest> customizer) {
        EditBatchRequest req = build();
        customizer.accept(req);
        return req;
    }
}
