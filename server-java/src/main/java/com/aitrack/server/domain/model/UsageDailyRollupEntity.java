package com.aitrack.server.domain.model;

import jakarta.persistence.*;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

@Entity
@Table(name = "usage_daily_rollups",
    uniqueConstraints = @UniqueConstraint(name = "uk_usage_daily_rollup", columnNames = {
        "token_key", "device_id", "\"day\"", "agent", "model", "account"
    }),
    indexes = {
        @Index(name = "idx_usage_daily_rollups_token_key", columnList = "token_key"),
        @Index(name = "idx_usage_daily_rollups_day", columnList = "\"day\""),
        @Index(name = "idx_usage_daily_rollups_agent", columnList = "agent")
    })
@Data
@NoArgsConstructor
public class UsageDailyRollupEntity {
    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "token_key", nullable = false, length = 32)
    private String tokenKey;

    @Column(name = "device_id", nullable = false, length = 64)
    private String deviceId;

    @Column(name = "\"day\"", nullable = false, length = 10)
    private String day;

    @Column(nullable = false, length = 64)
    private String agent;

    @Column(nullable = false, length = 255)
    private String model;

    @Column(nullable = false, length = 255)
    private String account = "";

    @Column(name = "tokens_in", nullable = false)
    private long tokensIn;

    @Column(name = "tokens_out", nullable = false)
    private long tokensOut;

    @Column(name = "tokens_cache_read", nullable = false)
    private long tokensCacheRead;

    @Column(name = "tokens_cache_write", nullable = false)
    private long tokensCacheWrite;

    @Column(name = "tokens_reasoning", nullable = false)
    private long tokensReasoning;

    @Column(name = "updated_at", nullable = false)
    private Instant updatedAt = Instant.now();
}
