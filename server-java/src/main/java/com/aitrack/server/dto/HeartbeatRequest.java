package com.aitrack.server.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.NotBlank;
import lombok.Data;

@Data
public class HeartbeatRequest {
    @NotBlank @JsonProperty("device_id") private String deviceId;
    private String hostname;
    @JsonProperty("token_key_masked") private String tokenKeyMasked;
    @JsonProperty("client_version") private String clientVersion;
    private long ts;
    private HooksStatus hooks;

    @JsonProperty("pending_count")
    private int pendingCount;

    @Data
    public static class HooksStatus {
        private boolean claude;
        private boolean codex;
        private boolean cursor;
    }
}
