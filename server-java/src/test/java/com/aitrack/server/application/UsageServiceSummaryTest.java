package com.aitrack.server.application;

import com.aitrack.server.adapter.db.UsageDailyRollupRepository;
import com.aitrack.server.domain.model.UsageDailyRollupEntity;
import com.aitrack.server.domain.model.UsageRollupItem;
import com.aitrack.server.domain.model.UsageRollupRequest;
import com.aitrack.server.domain.model.UsageSummary;
import com.aitrack.server.infrastructure.config.AiTrackServerApplication;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.data.domain.PageRequest;
import org.springframework.test.annotation.DirtiesContext;

import java.time.Instant;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

@SpringBootTest(classes = AiTrackServerApplication.class)
@DirtiesContext(classMode = DirtiesContext.ClassMode.AFTER_EACH_TEST_METHOD)
class UsageServiceSummaryTest {

    private static final String TOKEN_KEY = "summary-token";
    private static final String OTHER_TOKEN_KEY = "other-summary-token";

    @Autowired UsageDailyRollupRepository dailyRollups;
    @Autowired UsageService usageService;

    @Test
    void repositoryAggregatesTotalsAndPagesSummaryItems() {
        seedSummaryRows();

        var totals = dailyRollups.findSummaryTotals(
            TOKEN_KEY,
            "2026-06-01",
            "2026-06-30",
            "codex"
        );
        var items = dailyRollups.findSummaryItems(
            TOKEN_KEY,
            "2026-06-01",
            "2026-06-30",
            "codex",
            PageRequest.of(0, 1)
        );

        assertThat(totals.getTokensIn()).isEqualTo(200);
        assertThat(totals.getMessageCount()).isEqualTo(3);
        assertThat(totals.getSourceCost()).isEqualTo(2.00);
        assertThat(items).hasSize(1);
        assertThat(items.get(0).getModel()).isEqualTo("gpt-5");
        assertThat(items.get(0).getTotalTokens()).isEqualTo(130);
    }

    @Test
    void summaryLimitOneKeepsTotalsAcrossAllMatchingGroupsAndAppliesFilters() {
        seedSummaryRows();

        UsageSummary summary = usageService.summary(
            TOKEN_KEY,
            "2026-06-01",
            "2026-06-30",
            "codex",
            1
        );

        assertThat(summary.getTotalTokens()).isEqualTo(200);
        assertThat(summary.getTokensIn()).isEqualTo(200);
        assertThat(summary.getMessageCount()).isEqualTo(3);
        assertThat(summary.getSourceCost()).isEqualTo(2.00);
        assertThat(summary.getItems()).hasSize(1);
        assertThat(summary.getItems().get(0).getModel()).isEqualTo("gpt-5");
        assertThat(summary.getItems().get(0).getTotalTokens()).isEqualTo(130);
    }

    @Test
    void ingestRollupsBatchUpsertsWithoutGrowingDuplicateIdentityRows() {
        usageService.ingestRollups(TOKEN_KEY, request(item(10, 20, 2, 0.10)));
        usageService.ingestRollups(TOKEN_KEY, request(item(11, 22, -5, -1.00)));

        assertThat(dailyRollups.count()).isEqualTo(1);
        UsageDailyRollupEntity stored = dailyRollups
            .findByTokenKeyAndDeviceIdAndDayAndAgentAndModelAndAccountAndUsageBasis(
                TOKEN_KEY,
                "usage-device-java",
                "2026-06-16",
                "codex",
                "gpt-5",
                "",
                "native"
            )
            .orElseThrow();
        assertThat(stored.getTokensIn()).isEqualTo(11);
        assertThat(stored.getTokensOut()).isEqualTo(22);
        assertThat(stored.getMessageCount()).isZero();
        assertThat(stored.getSourceCost()).isZero();
    }

    private void seedSummaryRows() {
        dailyRollups.save(row(TOKEN_KEY, "device-top-1", "2026-06-10", "codex", "gpt-5", "local", 100, 1, 1.00));
        dailyRollups.save(row(TOKEN_KEY, "device-top-2", "2026-06-11", "codex", "gpt-5", "local", 30, 1, 0.30));
        dailyRollups.save(row(TOKEN_KEY, "device-second", "2026-06-12", "codex", "gpt-4o", "local", 70, 1, 0.70));
        dailyRollups.save(row(TOKEN_KEY, "device-other-agent", "2026-06-12", "cursor", "gpt-5", "local", 1_000, 1, 10.00));
        dailyRollups.save(row(TOKEN_KEY, "device-other-day", "2026-07-01", "codex", "gpt-5", "local", 2_000, 1, 20.00));
        dailyRollups.save(row(OTHER_TOKEN_KEY, "device-other-token", "2026-06-12", "codex", "gpt-5", "local", 3_000, 1, 30.00));
    }

    private static UsageRollupRequest request(UsageRollupItem item) {
        UsageRollupRequest request = new UsageRollupRequest();
        request.setItems(List.of(item));
        return request;
    }

    private static UsageRollupItem item(long tokensIn, long tokensOut, long messageCount, double sourceCost) {
        UsageRollupItem item = new UsageRollupItem();
        item.setDeviceId("usage-device-java");
        item.setDay("2026-06-16");
        item.setAgent("codex");
        item.setModel("gpt-5");
        item.setTokensIn(tokensIn);
        item.setTokensOut(tokensOut);
        item.setMessageCount(messageCount);
        item.setSourceCost(sourceCost);
        return item;
    }

    private static UsageDailyRollupEntity row(
        String tokenKey,
        String deviceId,
        String day,
        String agent,
        String model,
        String account,
        long tokensIn,
        long messageCount,
        double sourceCost
    ) {
        UsageDailyRollupEntity entity = new UsageDailyRollupEntity();
        entity.setTokenKey(tokenKey);
        entity.setDeviceId(deviceId);
        entity.setDay(day);
        entity.setAgent(agent);
        entity.setModel(model);
        entity.setAccount(account);
        entity.setTokensIn(tokensIn);
        entity.setMessageCount(messageCount);
        entity.setSourceCost(sourceCost);
        entity.setUpdatedAt(Instant.now());
        return entity;
    }
}
