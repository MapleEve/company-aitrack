package com.aitrack.server.application;

import com.aitrack.server.adapter.db.UsageDailyRollupRepository;
import com.aitrack.server.adapter.db.UsageSubscriptionSnapshotRepository;
import com.aitrack.server.domain.model.UsageDailyRollupEntity;
import com.aitrack.server.domain.model.UsageRollupItem;
import com.aitrack.server.domain.model.UsageRollupRequest;
import com.aitrack.server.domain.model.UsageSubscriptionSnapshotEntity;
import com.aitrack.server.domain.model.UsageSubscriptionSnapshotRequest;
import com.aitrack.server.domain.model.UsageSummary;
import com.aitrack.server.domain.model.UsageSummaryItem;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.Map;

@Service
@RequiredArgsConstructor
public class UsageService {

    private final UsageDailyRollupRepository dailyRollups;
    private final UsageSubscriptionSnapshotRepository subscriptionSnapshots;

    @Transactional
    public void ingestRollups(String tokenKey, UsageRollupRequest request) {
        for (UsageRollupItem item : request.getItems()) {
            String account = normalizeAccount(item.getAccount());
            UsageDailyRollupEntity entity = dailyRollups
                .findByTokenKeyAndDeviceIdAndDayAndAgentAndModelAndAccount(
                    tokenKey,
                    item.getDeviceId(),
                    item.getDay(),
                    item.getAgent(),
                    item.getModel(),
                    account
                )
                .orElseGet(UsageDailyRollupEntity::new);
            entity.setTokenKey(tokenKey);
            entity.setDeviceId(item.getDeviceId());
            entity.setDay(item.getDay());
            entity.setAgent(item.getAgent());
            entity.setModel(item.getModel());
            entity.setAccount(account);
            entity.setTokensIn(item.getTokensIn());
            entity.setTokensOut(item.getTokensOut());
            entity.setTokensCacheRead(item.getTokensCacheRead());
            entity.setTokensCacheWrite(item.getTokensCacheWrite());
            entity.setTokensReasoning(item.getTokensReasoning());
            entity.setMessageCount(Math.max(0, item.getMessageCount()));
            entity.setSourceCost(Math.max(0.0, item.getSourceCost()));
            entity.setUpdatedAt(Instant.now());
            dailyRollups.save(entity);
        }
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
        Map<String, UsageSummaryItem> grouped = new LinkedHashMap<>();
        for (UsageDailyRollupEntity row : dailyRollups.findByFilters(
            normalizedToken, normalizedFrom, normalizedTo, normalizedAgent)) {
            summary.setTokensIn(summary.getTokensIn() + row.getTokensIn());
            summary.setTokensOut(summary.getTokensOut() + row.getTokensOut());
            summary.setTokensCacheRead(summary.getTokensCacheRead() + row.getTokensCacheRead());
            summary.setTokensCacheWrite(summary.getTokensCacheWrite() + row.getTokensCacheWrite());
            summary.setTokensReasoning(summary.getTokensReasoning() + row.getTokensReasoning());
            summary.setMessageCount(summary.getMessageCount() + row.getMessageCount());
            summary.setSourceCost(summary.getSourceCost() + row.getSourceCost());

            String account = normalizeAccount(row.getAccount());
            String key = row.getTokenKey() + "\u0000" + row.getAgent() + "\u0000" + row.getModel() + "\u0000" + account;
            UsageSummaryItem item = grouped.computeIfAbsent(key, ignored ->
                new UsageSummaryItem(row.getTokenKey(), row.getAgent(), row.getModel(), account, 0, 0, 0.0));
            item.setTotalTokens(item.getTotalTokens() + total(row));
            item.setMessageCount(item.getMessageCount() + row.getMessageCount());
            item.setSourceCost(item.getSourceCost() + row.getSourceCost());
        }
        summary.setTotalTokens(
            summary.getTokensIn()
                + summary.getTokensOut()
                + summary.getTokensCacheRead()
                + summary.getTokensCacheWrite()
                + summary.getTokensReasoning()
        );
        int capped = limit <= 0 ? 20 : Math.min(limit, 100);
        summary.setItems(grouped.values().stream()
            .sorted(Comparator.comparingLong(UsageSummaryItem::getTotalTokens).reversed())
            .limit(capped)
            .toList());
        return summary;
    }

    private static long total(UsageDailyRollupEntity row) {
        return row.getTokensIn()
            + row.getTokensOut()
            + row.getTokensCacheRead()
            + row.getTokensCacheWrite()
            + row.getTokensReasoning();
    }

    private static String normalizeAccount(String account) {
        return account == null || account.isBlank() ? "" : account;
    }

    private static String blankToNull(String raw) {
        return raw == null || raw.isBlank() ? null : raw;
    }
}
