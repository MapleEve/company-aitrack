package com.aitrack.server.domain.model;

import jakarta.persistence.*;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

@Entity
@Table(name = "usage_subscription_snapshots",
    uniqueConstraints = @UniqueConstraint(name = "uk_usage_subscription_snapshot", columnNames = {
        "token_key", "device_id", "agent", "account"
    }),
    indexes = {
        @Index(name = "idx_usage_subscription_token_key", columnList = "token_key")
    })
@Data
@NoArgsConstructor
public class UsageSubscriptionSnapshotEntity {
    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "token_key", nullable = false, length = 32)
    private String tokenKey;

    @Column(name = "device_id", nullable = false, length = 64)
    private String deviceId;

    @Column(nullable = false, length = 64)
    private String agent;

    @Column(nullable = false, length = 255)
    private String account = "";

    @Column(length = 255)
    private String plan;

    @Column(name = "quota_session_remaining")
    private Long quotaSessionRemaining;

    @Column(name = "quota_weekly_remaining")
    private Long quotaWeeklyRemaining;

    @Column(name = "quota_reset_at")
    private String quotaResetAt;

    @Column(name = "reader_status", nullable = false, length = 64)
    private String readerStatus;

    @Column(name = "snapshotted_at", nullable = false)
    private String snapshottedAt;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt = Instant.now();
}
