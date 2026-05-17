package com.aitrack.server;

import com.aitrack.server.dto.DeviceInfo;
import com.aitrack.server.dto.StatsRow;
import com.aitrack.server.entity.DeviceEntity;
import com.aitrack.server.repository.DeviceRepository;
import com.aitrack.server.repository.EditRecordRepository;
import com.aitrack.server.service.StatsService;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockito.Mockito;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.Mockito.when;

class StatsServiceTest {

    private EditRecordRepository editRecordRepository;
    private DeviceRepository deviceRepository;
    private StatsService statsService;

    @BeforeEach
    void setUp() {
        editRecordRepository = Mockito.mock(EditRecordRepository.class);
        deviceRepository = Mockito.mock(DeviceRepository.class);
        statsService = new StatsService(editRecordRepository, deviceRepository);
    }

    private Object[] makeRow(String group, long edits, long added, long removed,
                              long accepted, long flagged, long rejected, Instant lastActive) {
        return new Object[]{group, edits, added, removed, accepted, flagged, rejected, lastActive};
    }

    @Test
    void getStats_groupByToken_returnsRows() {
        Instant now = Instant.now();
        when(editRecordRepository.aggregateByTokenKey())
                .thenReturn(List.<Object[]>of(makeRow("tok1…ef01", 10L, 100L, 20L, 8L, 1L, 1L, now)));

        List<StatsRow> rows = statsService.getStats("token");

        assertThat(rows).hasSize(1);
        StatsRow row = rows.get(0);
        assertThat(row.getGroup()).isEqualTo("tok1…ef01");
        assertThat(row.getEdits()).isEqualTo(10L);
        assertThat(row.getAddedLines()).isEqualTo(100L);
        assertThat(row.getRemovedLines()).isEqualTo(20L);
        assertThat(row.getAccepted()).isEqualTo(8L);
        assertThat(row.getFlagged()).isEqualTo(1L);
        assertThat(row.getRejected()).isEqualTo(1L);
        assertThat(row.getLastActive()).isEqualTo(now);
    }

    @Test
    void getStats_groupByRepo_delegatesToAggregateByRepo() {
        Instant now = Instant.now();
        when(editRecordRepository.aggregateByRepo())
                .thenReturn(List.<Object[]>of(makeRow("git@github.com:org/repo.git", 5L, 50L, 10L, 5L, 0L, 0L, now)));

        List<StatsRow> rows = statsService.getStats("repo");
        assertThat(rows).hasSize(1);
        assertThat(rows.get(0).getGroup()).contains("github.com");
    }

    @Test
    void getStats_groupByDevice_delegatesToAggregateByDevice() {
        Instant now = Instant.now();
        when(editRecordRepository.aggregateByDevice())
                .thenReturn(List.<Object[]>of(makeRow("device-001", 3L, 30L, 5L, 3L, 0L, 0L, now)));

        List<StatsRow> rows = statsService.getStats("device");
        assertThat(rows).hasSize(1);
        assertThat(rows.get(0).getGroup()).isEqualTo("device-001");
    }

    @Test
    void getStats_groupByHostname_delegatesToAggregateByHostname() {
        Instant now = Instant.now();
        when(editRecordRepository.aggregateByHostname())
                .thenReturn(List.<Object[]>of(makeRow("MacBook-Pro.local", 7L, 70L, 14L, 6L, 1L, 0L, now)));

        List<StatsRow> rows = statsService.getStats("hostname");
        assertThat(rows).hasSize(1);
        assertThat(rows.get(0).getGroup()).isEqualTo("MacBook-Pro.local");
        assertThat(rows.get(0).getEdits()).isEqualTo(7L);
    }

    @Test
    void getStats_unknownGroupBy_defaultsToTokenKey() {
        when(editRecordRepository.aggregateByTokenKey()).thenReturn(List.of());

        List<StatsRow> rows = statsService.getStats("unknown");
        assertThat(rows).isEmpty();
    }

    @Test
    void getStats_nullLastActive_defaultsToNull() {
        // r[7] = null should be handled as null Instant
        when(editRecordRepository.aggregateByTokenKey())
                .thenReturn(List.<Object[]>of(makeRow("tok", 1L, 10L, 2L, 1L, 0L, 0L, null)));

        List<StatsRow> rows = statsService.getStats("token");
        assertThat(rows.get(0).getLastActive()).isNull();
    }

    @Test
    void getStats_nullNumericFields_defaultToZero() {
        // Null in numeric positions r[2]-r[6] should map to 0
        Object[] row = new Object[]{"tok", 1L, null, null, null, null, null, null};
        when(editRecordRepository.aggregateByTokenKey()).thenReturn(List.<Object[]>of(row));

        List<StatsRow> rows = statsService.getStats("token");
        StatsRow r = rows.get(0);
        assertThat(r.getAddedLines()).isEqualTo(0L);
        assertThat(r.getRemovedLines()).isEqualTo(0L);
        assertThat(r.getAccepted()).isEqualTo(0L);
        assertThat(r.getFlagged()).isEqualTo(0L);
        assertThat(r.getRejected()).isEqualTo(0L);
    }

    @Test
    void getDevices_returnsDeviceInfoList() {
        DeviceEntity active = new DeviceEntity();
        active.setDeviceId("dev-active");
        active.setTokenKey("tok…0001");
        active.setHostname("dev-machine.local");
        active.setClientVersion("1.0.0");
        active.setLastHeartbeat(Instant.now());
        active.setHooksJson("{\"claude\":true}");

        DeviceEntity silent = new DeviceEntity();
        silent.setDeviceId("dev-silent");
        silent.setTokenKey("tok…0002");
        silent.setHostname(null);
        silent.setClientVersion("0.9.0");
        silent.setLastHeartbeat(Instant.now().minus(8, ChronoUnit.DAYS));
        silent.setHooksJson(null);

        when(deviceRepository.findAll()).thenReturn(List.of(active, silent));

        List<DeviceInfo> devices = statsService.getDevices();

        assertThat(devices).hasSize(2);
        DeviceInfo activeInfo = devices.stream().filter(d -> d.getDeviceId().equals("dev-active")).findFirst().orElseThrow();
        DeviceInfo silentInfo = devices.stream().filter(d -> d.getDeviceId().equals("dev-silent")).findFirst().orElseThrow();

        assertThat(activeInfo.isSilent()).isFalse();
        assertThat(activeInfo.getHostname()).isEqualTo("dev-machine.local");
        assertThat(activeInfo.getHooksJson()).contains("claude");
        assertThat(silentInfo.isSilent()).isTrue();
        assertThat(silentInfo.getHostname()).isNull();
    }

    @Test
    void getDevices_nullLastHeartbeat_markedSilent() {
        DeviceEntity device = new DeviceEntity();
        device.setDeviceId("dev-null-hb");
        device.setTokenKey("tok…0003");
        device.setLastHeartbeat(null);

        when(deviceRepository.findAll()).thenReturn(List.of(device));

        List<DeviceInfo> devices = statsService.getDevices();
        assertThat(devices.get(0).isSilent()).isTrue();
    }
}
