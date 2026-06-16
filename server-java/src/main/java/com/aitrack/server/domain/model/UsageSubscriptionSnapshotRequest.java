package com.aitrack.server.domain.model;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.Data;

@Data
public class UsageSubscriptionSnapshotRequest {
    @JsonProperty("device_id")
    private String deviceId;
    private String agent;
    private String account;
    private String plan;
    @JsonProperty("quota_session_remaining")
    private Long quotaSessionRemaining;
    @JsonProperty("quota_weekly_remaining")
    private Long quotaWeeklyRemaining;
    @JsonProperty("quota_reset_at")
    private String quotaResetAt;
    @JsonProperty("reader_status")
    private String readerStatus;
    @JsonProperty("snapshotted_at")
    private String snapshottedAt;
}
