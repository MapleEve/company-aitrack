package com.aitrack.server.testkit;

import com.aitrack.server.dto.CreateTokenRequest;

import java.util.function.Consumer;

/**
 * Deterministic factory for CreateTokenRequest test instances.
 */
public final class CreateTokenRequestFactory {

    private CreateTokenRequestFactory() {}

    public static CreateTokenRequest build() {
        CreateTokenRequest req = new CreateTokenRequest();
        req.setOwner("test-owner");
        req.setNote("test token for unit tests");
        return req;
    }

    public static CreateTokenRequest with(Consumer<CreateTokenRequest> customizer) {
        CreateTokenRequest req = build();
        customizer.accept(req);
        return req;
    }
}
