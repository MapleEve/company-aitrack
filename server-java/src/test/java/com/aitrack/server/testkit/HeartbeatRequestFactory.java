package com.aitrack.server.testkit;

import com.aitrack.server.dto.HeartbeatRequest;

import java.util.function.Consumer;

/**
 * Deterministic factory for HeartbeatRequest test instances.
 */
public final class HeartbeatRequestFactory {

    private HeartbeatRequestFactory() {}

    public static HeartbeatRequest build() {
        HeartbeatRequest req = new HeartbeatRequest();
        req.setDeviceId(EditDtoFactory.DEFAULT_DEVICE_ID);
        req.setHostname(EditDtoFactory.DEFAULT_HOSTNAME);
        req.setTokenKeyMasked(EditDtoFactory.DEFAULT_TOKEN_KEY);
        req.setClientVersion("1.0.0");
        req.setTs(1715940000L);
        req.setPendingCount(3);

        HeartbeatRequest.HooksStatus hooks = new HeartbeatRequest.HooksStatus();
        hooks.setClaude(true);
        hooks.setCodex(false);
        hooks.setCursor(false);
        req.setHooks(hooks);

        return req;
    }

    /** Build with all hooks disabled (simulates hook removal scenario). */
    public static HeartbeatRequest buildAllHooksOff() {
        HeartbeatRequest req = build();
        HeartbeatRequest.HooksStatus hooks = new HeartbeatRequest.HooksStatus();
        hooks.setClaude(false);
        hooks.setCodex(false);
        hooks.setCursor(false);
        req.setHooks(hooks);
        return req;
    }

    public static HeartbeatRequest with(Consumer<HeartbeatRequest> customizer) {
        HeartbeatRequest req = build();
        customizer.accept(req);
        return req;
    }
}
