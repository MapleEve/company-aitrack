package com.aitrack.server.controller;

import com.aitrack.server.config.RequestAuthHelper;
import com.aitrack.server.dto.DeviceInfo;
import com.aitrack.server.dto.StatsRow;
import com.aitrack.server.service.StatsService;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.List;

@RestController
@RequestMapping("/api/v1/ai-track")
@RequiredArgsConstructor
public class StatsController {

    private final RequestAuthHelper authHelper;
    private final StatsService statsService;

    @GetMapping("/stats")
    public ResponseEntity<List<StatsRow>> stats(
        HttpServletRequest httpRequest,
        @RequestParam(defaultValue = "token") String group_by
    ) {
        authHelper.resolveToken(httpRequest);
        return ResponseEntity.ok(statsService.getStats(group_by));
    }

    @GetMapping("/devices")
    public ResponseEntity<List<DeviceInfo>> devices(HttpServletRequest httpRequest) {
        authHelper.resolveToken(httpRequest);
        return ResponseEntity.ok(statsService.getDevices());
    }
}
