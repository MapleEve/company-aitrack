package com.aitrack.server.testkit;

import com.aitrack.server.domain.model.HeartbeatRequest;

import java.util.LinkedHashMap;
import java.util.Map;
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

        req.setHooks(hooks(true, false, false));

        return req;
    }

    /** Build with all hooks disabled (simulates hook removal scenario). */
    public static HeartbeatRequest buildAllHooksOff() {
        HeartbeatRequest req = build();
        req.setHooks(hooks(false, false, false));
        return req;
    }

    public static HeartbeatRequest with(Consumer<HeartbeatRequest> customizer) {
        HeartbeatRequest req = build();
        customizer.accept(req);
        return req;
    }

    private static Map<String, Boolean> hooks(boolean claude, boolean codex, boolean cursor) {
        Map<String, Boolean> hooks = new LinkedHashMap<>();
        hooks.put("claude", claude);
        hooks.put("codex", codex);
        hooks.put("cursor", cursor);
        return hooks;
    }
}
