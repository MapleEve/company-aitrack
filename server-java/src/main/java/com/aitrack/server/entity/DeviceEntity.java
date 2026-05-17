package com.aitrack.server.entity;

import jakarta.persistence.*;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

@Entity
@Table(name = "devices", indexes = {
    @Index(name = "idx_devices_device_id", columnList = "device_id", unique = true)
})
@Data
@NoArgsConstructor
public class DeviceEntity {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "device_id", nullable = false, unique = true, length = 64)
    private String deviceId;

    @Column(name = "token_key", nullable = false, length = 16)
    private String tokenKey;

    @Column(length = 255)
    private String hostname;

    @Column(name = "client_version", length = 32)
    private String clientVersion;

    @Column(name = "last_heartbeat")
    private Instant lastHeartbeat;

    // JSON blob: {"claude":true,"codex":false,"cursor":false}
    @Column(name = "hooks_json", length = 256)
    private String hooksJson;

    @Column(name = "created_at", nullable = false)
    private Instant createdAt = Instant.now();
}
