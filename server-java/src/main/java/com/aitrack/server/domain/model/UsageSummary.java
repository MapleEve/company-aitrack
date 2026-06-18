package com.aitrack.server.domain.model;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.Data;

import java.util.ArrayList;
import java.util.List;

@Data
public class UsageSummary {
    @JsonProperty("total_tokens")
    private long totalTokens;
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
    private List<UsageSummaryItem> items = new ArrayList<>();
}
