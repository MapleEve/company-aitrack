package com.aitrack.server;

import com.aitrack.server.dto.HeartbeatRequest;
import com.aitrack.server.entity.DeviceEntity;
import com.aitrack.server.entity.TokenEntity;
import com.aitrack.server.repository.DeviceRepository;
import com.aitrack.server.service.HeartbeatService;
import com.aitrack.server.testkit.HeartbeatRequestFactory;
import com.aitrack.server.testkit.TokenEntityFactory;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.ArgumentCaptor;
import org.mockito.Mockito;

import java.time.Instant;
import java.util.Optional;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.Mockito.*;

class HeartbeatServiceTest {

    private DeviceRepository deviceRepository;
    private HeartbeatService heartbeatService;
    private TokenEntity token;

    @BeforeEach
    void setUp() {
        deviceRepository = Mockito.mock(DeviceRepository.class);
        ObjectMapper objectMapper = new ObjectMapper();
        heartbeatService = new HeartbeatService(deviceRepository, objectMapper);
        token = TokenEntityFactory.build();

        when(deviceRepository.save(any(DeviceEntity.class))).thenAnswer(inv -> inv.getArgument(0));
    }

    @Test
    void recordHeartbeat_newDevice_createsEntity() {
        when(deviceRepository.findByDeviceId(anyString())).thenReturn(Optional.empty());
        HeartbeatRequest req = HeartbeatRequestFactory.build();

        heartbeatService.recordHeartbeat(token, req);

        ArgumentCaptor<DeviceEntity> captor = ArgumentCaptor.forClass(DeviceEntity.class);
        verify(deviceRepository).save(captor.capture());
        DeviceEntity saved = captor.getValue();

        assertThat(saved.getDeviceId()).isEqualTo(req.getDeviceId());
        assertThat(saved.getTokenKey()).isEqualTo(token.getTokenKey());
        assertThat(saved.getClientVersion()).isEqualTo("1.0.0");
        assertThat(saved.getLastHeartbeat()).isNotNull();
        assertThat(saved.getHooksJson()).contains("claude");
    }

    @Test
    void recordHeartbeat_existingDevice_updatesHeartbeat() {
        DeviceEntity existing = new DeviceEntity();
        existing.setDeviceId(HeartbeatRequestFactory.build().getDeviceId());
        existing.setTokenKey(token.getTokenKey());
        existing.setCreatedAt(Instant.parse("2026-01-01T00:00:00Z"));
        existing.setLastHeartbeat(Instant.parse("2026-01-01T00:00:00Z"));
        existing.setClientVersion("0.9.0");

        when(deviceRepository.findByDeviceId(anyString())).thenReturn(Optional.of(existing));
        HeartbeatRequest req = HeartbeatRequestFactory.build();

        heartbeatService.recordHeartbeat(token, req);

        ArgumentCaptor<DeviceEntity> captor = ArgumentCaptor.forClass(DeviceEntity.class);
        verify(deviceRepository).save(captor.capture());
        DeviceEntity saved = captor.getValue();

        assertThat(saved.getClientVersion()).isEqualTo("1.0.0");
        assertThat(saved.getLastHeartbeat()).isAfter(Instant.parse("2026-01-01T00:00:00Z"));
    }

    @Test
    void recordHeartbeat_hooksStoredAsJson() {
        when(deviceRepository.findByDeviceId(anyString())).thenReturn(Optional.empty());
        HeartbeatRequest req = HeartbeatRequestFactory.build();

        heartbeatService.recordHeartbeat(token, req);

        ArgumentCaptor<DeviceEntity> captor = ArgumentCaptor.forClass(DeviceEntity.class);
        verify(deviceRepository).save(captor.capture());
        String hooksJson = captor.getValue().getHooksJson();

        assertThat(hooksJson).contains("\"claude\":true");
        assertThat(hooksJson).contains("\"codex\":false");
        assertThat(hooksJson).contains("\"cursor\":false");
    }

    @Test
    void recordHeartbeat_allHooksOff_storedCorrectly() {
        when(deviceRepository.findByDeviceId(anyString())).thenReturn(Optional.empty());
        HeartbeatRequest req = HeartbeatRequestFactory.buildAllHooksOff();

        heartbeatService.recordHeartbeat(token, req);

        ArgumentCaptor<DeviceEntity> captor = ArgumentCaptor.forClass(DeviceEntity.class);
        verify(deviceRepository).save(captor.capture());
        String hooksJson = captor.getValue().getHooksJson();

        assertThat(hooksJson).contains("\"claude\":false");
        assertThat(hooksJson).contains("\"codex\":false");
        assertThat(hooksJson).contains("\"cursor\":false");
    }

    @Test
    void recordHeartbeat_nullHooks_doesNotSetHooksJson() {
        when(deviceRepository.findByDeviceId(anyString())).thenReturn(Optional.empty());
        HeartbeatRequest req = HeartbeatRequestFactory.build();
        req.setHooks(null);

        heartbeatService.recordHeartbeat(token, req);

        ArgumentCaptor<DeviceEntity> captor = ArgumentCaptor.forClass(DeviceEntity.class);
        verify(deviceRepository).save(captor.capture());
        // hooks == null branch: hooksJson not set, remains null (new entity) or unchanged
        // The device was just created; no hooksJson set
        assertThat(captor.getValue().getHooksJson()).isNull();
    }
}
