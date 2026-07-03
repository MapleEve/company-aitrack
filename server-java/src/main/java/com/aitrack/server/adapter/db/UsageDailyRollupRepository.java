package com.aitrack.server.adapter.db;

import com.aitrack.server.domain.model.UsageDailyRollupEntity;
import org.springframework.data.domain.Pageable;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;

@Repository
public interface UsageDailyRollupRepository extends JpaRepository<UsageDailyRollupEntity, Long> {
    Optional<UsageDailyRollupEntity> findByTokenKeyAndDeviceIdAndDayAndAgentAndModelAndAccountAndUsageBasis(
        String tokenKey,
        String deviceId,
        String day,
        String agent,
        String model,
        String account,
        String usageBasis
    );

    @Query("""
        SELECT
            COALESCE(SUM(u.tokensIn), 0) AS tokensIn,
            COALESCE(SUM(u.tokensOut), 0) AS tokensOut,
            COALESCE(SUM(u.tokensCacheRead), 0) AS tokensCacheRead,
            COALESCE(SUM(u.tokensCacheWrite), 0) AS tokensCacheWrite,
            COALESCE(SUM(u.tokensReasoning), 0) AS tokensReasoning,
            COALESCE(SUM(u.messageCount), 0) AS messageCount,
            COALESCE(SUM(u.sourceCost), 0.0) AS sourceCost
        FROM UsageDailyRollupEntity u
        WHERE (:tokenKey IS NULL OR u.tokenKey = :tokenKey)
            AND (:fromDay IS NULL OR u.day >= :fromDay)
            AND (:toDay IS NULL OR u.day <= :toDay)
            AND (:agent IS NULL OR u.agent = :agent)
        """)
    UsageSummaryTotalsProjection findSummaryTotals(
        @Param("tokenKey") String tokenKey,
        @Param("fromDay") String fromDay,
        @Param("toDay") String toDay,
        @Param("agent") String agent
    );

    @Query("""
        SELECT
            u.tokenKey AS tokenKey,
            u.agent AS agent,
            u.model AS model,
            u.account AS account,
            u.usageBasis AS usageBasis,
            (
                SUM(u.tokensIn)
                + SUM(u.tokensOut)
                + SUM(u.tokensCacheRead)
                + SUM(u.tokensCacheWrite)
                + SUM(u.tokensReasoning)
            ) AS totalTokens,
            SUM(u.messageCount) AS messageCount,
            SUM(u.sourceCost) AS sourceCost
        FROM UsageDailyRollupEntity u
        WHERE (:tokenKey IS NULL OR u.tokenKey = :tokenKey)
            AND (:fromDay IS NULL OR u.day >= :fromDay)
            AND (:toDay IS NULL OR u.day <= :toDay)
            AND (:agent IS NULL OR u.agent = :agent)
        GROUP BY u.tokenKey, u.agent, u.model, u.account, u.usageBasis
        ORDER BY (
            SUM(u.tokensIn)
            + SUM(u.tokensOut)
            + SUM(u.tokensCacheRead)
            + SUM(u.tokensCacheWrite)
            + SUM(u.tokensReasoning)
        ) DESC
        """)
    List<UsageSummaryItemProjection> findSummaryItems(
        @Param("tokenKey") String tokenKey,
        @Param("fromDay") String fromDay,
        @Param("toDay") String toDay,
        @Param("agent") String agent,
        Pageable pageable
    );

    interface UsageSummaryTotalsProjection {
        Long getTokensIn();
        Long getTokensOut();
        Long getTokensCacheRead();
        Long getTokensCacheWrite();
        Long getTokensReasoning();
        Long getMessageCount();
        Double getSourceCost();
    }

    interface UsageSummaryItemProjection {
        String getTokenKey();
        String getAgent();
        String getModel();
        String getAccount();
        String getUsageBasis();
        Long getTotalTokens();
        Long getMessageCount();
        Double getSourceCost();
    }
}
