package com.aitrack.server.application;

import com.aitrack.server.adapter.db.UsageDailyRollupRepository;
import com.aitrack.server.adapter.db.UsageSubscriptionSnapshotRepository;
import com.aitrack.server.domain.model.UsageRollupItem;
import com.aitrack.server.domain.model.UsageRollupRequest;
import com.aitrack.server.domain.model.UsageSubscriptionSnapshotEntity;
import com.aitrack.server.domain.model.UsageSubscriptionSnapshotRequest;
import com.aitrack.server.domain.model.UsageSummary;
import com.aitrack.server.domain.model.UsageSummaryItem;
import lombok.RequiredArgsConstructor;
import org.springframework.data.domain.PageRequest;
import org.springframework.http.HttpStatus;
import org.springframework.jdbc.core.BatchPreparedStatementSetter;
import org.springframework.jdbc.core.JdbcTemplate;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;
import org.springframework.web.server.ResponseStatusException;

import java.sql.PreparedStatement;
import java.sql.SQLException;
import java.sql.Timestamp;
import java.time.Instant;
import java.util.List;
import java.util.Locale;
import java.util.Objects;

@Service
@RequiredArgsConstructor
public class UsageService {

    private final UsageDailyRollupRepository dailyRollups;
    private final UsageSubscriptionSnapshotRepository subscriptionSnapshots;
    private final JdbcTemplate jdbcTemplate;

    @Transactional
    public void ingestRollups(String tokenKey, UsageRollupRequest request) {
        Instant updatedAt = Instant.now();
        List<RollupRow> rows = request.getItems().stream()
            .map(item -> RollupRow.from(tokenKey, item, updatedAt))
            .toList();
        jdbcTemplate.batchUpdate(rollupUpsertSql(), new BatchPreparedStatementSetter() {
            @Override
            public void setValues(PreparedStatement ps, int i) throws SQLException {
                rows.get(i).bind(ps);
            }

            @Override
            public int getBatchSize() {
                return rows.size();
            }
        });
    }

    @Transactional
    public void ingestSubscription(String tokenKey, UsageSubscriptionSnapshotRequest request) {
        String account = normalizeAccount(request.getAccount());
        UsageSubscriptionSnapshotEntity entity = subscriptionSnapshots
            .findByTokenKeyAndDeviceIdAndAgentAndAccount(
                tokenKey,
                request.getDeviceId(),
                request.getAgent(),
                account
            )
            .orElseGet(UsageSubscriptionSnapshotEntity::new);
        entity.setTokenKey(tokenKey);
        entity.setDeviceId(request.getDeviceId());
        entity.setAgent(request.getAgent());
        entity.setAccount(account);
        entity.setPlan(request.getPlan());
        entity.setQuotaSessionRemaining(request.getQuotaSessionRemaining());
        entity.setQuotaWeeklyRemaining(request.getQuotaWeeklyRemaining());
        entity.setQuotaResetAt(request.getQuotaResetAt());
        entity.setReaderStatus(request.getReaderStatus());
        entity.setSnapshottedAt(request.getSnapshottedAt());
        entity.setUpdatedAt(Instant.now());
        subscriptionSnapshots.save(entity);
    }

    @Transactional(readOnly = true)
    public UsageSummary summary(String tokenKey, String fromDay, String toDay, String agent, int limit) {
        String normalizedToken = blankToNull(tokenKey);
        String normalizedFrom = blankToNull(fromDay);
        String normalizedTo = blankToNull(toDay);
        String normalizedAgent = blankToNull(agent);

        UsageSummary summary = new UsageSummary();
        UsageDailyRollupRepository.UsageSummaryTotalsProjection totals = dailyRollups.findSummaryTotals(
            normalizedToken,
            normalizedFrom,
            normalizedTo,
            normalizedAgent
        );
        summary.setTokensIn(orZero(totals.getTokensIn()));
        summary.setTokensOut(orZero(totals.getTokensOut()));
        summary.setTokensCacheRead(orZero(totals.getTokensCacheRead()));
        summary.setTokensCacheWrite(orZero(totals.getTokensCacheWrite()));
        summary.setTokensReasoning(orZero(totals.getTokensReasoning()));
        summary.setMessageCount(orZero(totals.getMessageCount()));
        summary.setSourceCost(orZero(totals.getSourceCost()));
        summary.setTotalTokens(
            summary.getTokensIn()
                + summary.getTokensOut()
                + summary.getTokensCacheRead()
                + summary.getTokensCacheWrite()
                + summary.getTokensReasoning()
        );
        int capped = limit <= 0 ? 20 : Math.min(limit, 100);
        summary.setItems(dailyRollups.findSummaryItems(
                normalizedToken,
                normalizedFrom,
                normalizedTo,
                normalizedAgent,
                PageRequest.of(0, capped)
            ).stream()
            .map(item -> new UsageSummaryItem(
                item.getTokenKey(),
                item.getAgent(),
                item.getModel(),
                normalizeAccount(item.getAccount()),
                normalizeUsageBasis(item.getUsageBasis()),
                orZero(item.getTotalTokens()),
                orZero(item.getMessageCount()),
                orZero(item.getSourceCost())
            ))
            .toList());
        return summary;
    }

    private static long orZero(Long value) {
        return value == null ? 0 : value;
    }

    private static double orZero(Double value) {
        return value == null ? 0.0 : value;
    }

    private static String normalizeAccount(String account) {
        return account == null || account.isBlank() ? "" : account;
    }

    private static String normalizeUsageBasis(String usageBasis) {
        if (usageBasis == null || usageBasis.isBlank()) {
            return "native";
        }
        String normalized = usageBasis.trim();
        if ("native".equals(normalized) || "local_derived".equals(normalized)) {
            return normalized;
        }
        throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "usage_basis must be native or local_derived");
    }

    private static String blankToNull(String raw) {
        return raw == null || raw.isBlank() ? null : raw;
    }

    private String rollupUpsertSql() {
        if (isPostgres()) {
            return """
                INSERT INTO usage_daily_rollups (
                    token_key, device_id, "day", agent, model, account, usage_basis,
                    tokens_in, tokens_out, tokens_cache_read, tokens_cache_write, tokens_reasoning,
                    message_count, source_cost, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT (token_key, device_id, "day", agent, model, account, usage_basis)
                DO UPDATE SET
                    tokens_in = EXCLUDED.tokens_in,
                    tokens_out = EXCLUDED.tokens_out,
                    tokens_cache_read = EXCLUDED.tokens_cache_read,
                    tokens_cache_write = EXCLUDED.tokens_cache_write,
                    tokens_reasoning = EXCLUDED.tokens_reasoning,
                    message_count = EXCLUDED.message_count,
                    source_cost = EXCLUDED.source_cost,
                    updated_at = EXCLUDED.updated_at
                """;
        }
        return """
            MERGE INTO usage_daily_rollups (
                token_key, device_id, "day", agent, model, account, usage_basis,
                tokens_in, tokens_out, tokens_cache_read, tokens_cache_write, tokens_reasoning,
                message_count, source_cost, updated_at
            ) KEY(token_key, device_id, "day", agent, model, account, usage_basis)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """;
    }

    private boolean isPostgres() {
        try (var connection = Objects.requireNonNull(jdbcTemplate.getDataSource()).getConnection()) {
            return connection.getMetaData()
                .getDatabaseProductName()
                .toLowerCase(Locale.ROOT)
                .contains("postgres");
        } catch (SQLException e) {
            throw new IllegalStateException("failed to detect usage rollup database dialect", e);
        }
    }

    private record RollupRow(
        String tokenKey,
        String deviceId,
        String day,
        String agent,
        String model,
        String account,
        String usageBasis,
        long tokensIn,
        long tokensOut,
        long tokensCacheRead,
        long tokensCacheWrite,
        long tokensReasoning,
        long messageCount,
        double sourceCost,
        Instant updatedAt
    ) {
        static RollupRow from(String tokenKey, UsageRollupItem item, Instant updatedAt) {
            return new RollupRow(
                tokenKey,
                item.getDeviceId(),
                item.getDay(),
                item.getAgent(),
                item.getModel(),
                normalizeAccount(item.getAccount()),
                normalizeUsageBasis(item.getUsageBasis()),
                item.getTokensIn(),
                item.getTokensOut(),
                item.getTokensCacheRead(),
                item.getTokensCacheWrite(),
                item.getTokensReasoning(),
                Math.max(0, item.getMessageCount()),
                Math.max(0.0, item.getSourceCost()),
                updatedAt
            );
        }

        void bind(PreparedStatement ps) throws SQLException {
            ps.setString(1, tokenKey);
            ps.setString(2, deviceId);
            ps.setString(3, day);
            ps.setString(4, agent);
            ps.setString(5, model);
            ps.setString(6, account);
            ps.setString(7, usageBasis);
            ps.setLong(8, tokensIn);
            ps.setLong(9, tokensOut);
            ps.setLong(10, tokensCacheRead);
            ps.setLong(11, tokensCacheWrite);
            ps.setLong(12, tokensReasoning);
            ps.setLong(13, messageCount);
            ps.setDouble(14, sourceCost);
            ps.setTimestamp(15, Timestamp.from(updatedAt));
        }
    }
}
