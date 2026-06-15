package com.aitrack.server.application;

import com.aitrack.server.domain.model.DeviceInfo;
import com.aitrack.server.domain.model.StatsRow;
import com.aitrack.server.domain.model.DeviceEntity;
import com.aitrack.server.domain.port.DevicePort;
import com.aitrack.server.domain.port.EditRecordPort;
import lombok.RequiredArgsConstructor;
import org.springframework.stereotype.Service;

import java.time.Instant;
import java.time.temporal.ChronoUnit;
import java.util.List;
import java.util.stream.Collectors;

@Service
@RequiredArgsConstructor
public class StatsService {

    private final EditRecordPort editRecordPort;
    private final DevicePort deviceRepository;

    public List<StatsRow> getStats(String groupBy) {
        List<Object[]> rows = switch (groupBy) {
            case "repo" -> editRecordPort.aggregateByRepo();
            case "device" -> editRecordPort.aggregateByDevice();
            case "hostname" -> editRecordPort.aggregateByHostname();
            case "tool" -> editRecordPort.aggregateByTool();
            default -> editRecordPort.aggregateByTokenKey();
        };

        return rows.stream()
            .map(r -> new StatsRow(
                (String) r[0],
                ((Number) r[1]).longValue(),
                r[2] != null ? ((Number) r[2]).longValue() : 0,
                r[3] != null ? ((Number) r[3]).longValue() : 0,
                r[4] != null ? ((Number) r[4]).longValue() : 0,
                r[5] != null ? ((Number) r[5]).longValue() : 0,
                r[6] != null ? ((Number) r[6]).longValue() : 0,
                r[7] != null ? (Instant) r[7] : null
            ))
            .collect(Collectors.toList());
    }

    public List<DeviceInfo> getDevices() {
        Instant silentThreshold = Instant.now().minus(7, ChronoUnit.DAYS);
        return deviceRepository.findAll().stream()
            .map(d -> new DeviceInfo(
                d.getDeviceId(),
                d.getTokenKey(),
                d.getHostname(),
                d.getClientVersion(),
                d.getLastHeartbeat(),
                d.getHooksJson(),
                d.getLastHeartbeat() == null || d.getLastHeartbeat().isBefore(silentThreshold)
            ))
            .collect(Collectors.toList());
    }
}
