package com.aitrack.server.domain.model;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.NotBlank;
import lombok.Data;

import java.util.Map;

@Data
public class HeartbeatRequest {
    @NotBlank @JsonProperty("device_id") private String deviceId;
    private String hostname;
    @JsonProperty("token_key_masked") private String tokenKeyMasked;
    @JsonProperty("client_version") private String clientVersion;
    private long ts;
    private Map<String, Boolean> hooks;

    @JsonProperty("pending_count")
    private int pendingCount;
}
