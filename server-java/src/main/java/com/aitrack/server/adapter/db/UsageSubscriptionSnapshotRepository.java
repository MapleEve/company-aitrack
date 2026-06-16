package com.aitrack.server.adapter.db;

import com.aitrack.server.domain.model.UsageSubscriptionSnapshotEntity;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;

@Repository
public interface UsageSubscriptionSnapshotRepository extends JpaRepository<UsageSubscriptionSnapshotEntity, Long> {
    Optional<UsageSubscriptionSnapshotEntity> findByTokenKeyAndDeviceIdAndAgentAndAccount(
        String tokenKey,
        String deviceId,
        String agent,
        String account
    );
}
