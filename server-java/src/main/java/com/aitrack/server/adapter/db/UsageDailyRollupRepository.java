package com.aitrack.server.adapter.db;

import com.aitrack.server.domain.model.UsageDailyRollupEntity;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.Optional;

@Repository
public interface UsageDailyRollupRepository extends JpaRepository<UsageDailyRollupEntity, Long> {
    Optional<UsageDailyRollupEntity> findByTokenKeyAndDeviceIdAndDayAndAgentAndModelAndAccount(
        String tokenKey,
        String deviceId,
        String day,
        String agent,
        String model,
        String account
    );

    @Query("SELECT u FROM UsageDailyRollupEntity u WHERE (:tokenKey IS NULL OR u.tokenKey = :tokenKey) " +
           "AND (:fromDay IS NULL OR u.day >= :fromDay) " +
           "AND (:toDay IS NULL OR u.day <= :toDay) " +
           "AND (:agent IS NULL OR u.agent = :agent)")
    List<UsageDailyRollupEntity> findByFilters(
        @Param("tokenKey") String tokenKey,
        @Param("fromDay") String fromDay,
        @Param("toDay") String toDay,
        @Param("agent") String agent
    );
}
