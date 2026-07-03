package com.aitrack.server.domain.model;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.Data;

@Data
public class UsageRollupItem {
    @JsonProperty("device_id")
    private String deviceId;
    private String day;
    private String agent;
    private String model;
    private String account;
    @JsonProperty("usage_basis")
    private String usageBasis;
    @JsonProperty("tokens_in")
    private long tokensIn;
    @JsonProperty("tokens_out")
    private long tokensOut;
    @JsonProperty("tokens_cache_read")
    private long tokensCacheRead;
    @JsonProperty("tokens_cache_write")
    private long tokensCacheWrite;
    @JsonProperty("tokens_reasoning")
    private long tokensReasoning;
    @JsonProperty("message_count")
    private long messageCount;
    @JsonProperty("source_cost")
    private double sourceCost;
}
