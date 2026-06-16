package com.aitrack.server.domain.model;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Data;
import lombok.NoArgsConstructor;

@Data
@NoArgsConstructor
@AllArgsConstructor
public class UsageSummaryItem {
    @JsonProperty("token_key")
    private String tokenKey;
    private String agent;
    private String model;
    private String account;
    @JsonProperty("total_tokens")
    private long totalTokens;
    @JsonProperty("message_count")
    private long messageCount;
    @JsonProperty("source_cost")
    private double sourceCost;
}
