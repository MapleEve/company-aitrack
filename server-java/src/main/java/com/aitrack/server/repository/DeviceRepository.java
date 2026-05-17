package com.aitrack.server.repository;

import com.aitrack.server.entity.DeviceEntity;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.time.Instant;
import java.util.List;
import java.util.Optional;

@Repository
public interface DeviceRepository extends JpaRepository<DeviceEntity, Long> {
    Optional<DeviceEntity> findByDeviceId(String deviceId);
    List<DeviceEntity> findByLastHeartbeatBefore(Instant threshold);
}
