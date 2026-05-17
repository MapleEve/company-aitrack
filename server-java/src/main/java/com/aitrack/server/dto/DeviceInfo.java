package com.aitrack.server.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Data;

import java.time.Instant;

@Data
@AllArgsConstructor
public class DeviceInfo {
    @JsonProperty("device_id") private String deviceId;
    @JsonProperty("token_key") private String tokenKey;
    private String hostname;
    @JsonProperty("client_version") private String clientVersion;
    @JsonProperty("last_heartbeat") private Instant lastHeartbeat;
    @JsonProperty("hooks_json") private String hooksJson;
    private boolean silent;
}
