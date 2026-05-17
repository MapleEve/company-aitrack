package com.aitrack.server.controller;

import com.aitrack.server.config.RequestAuthHelper;
import com.aitrack.server.dto.HeartbeatRequest;
import com.aitrack.server.entity.TokenEntity;
import com.aitrack.server.service.HeartbeatService;
import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

import java.io.IOException;
import java.util.Map;

@RestController
@RequestMapping("/api/v1/ai-track")
@RequiredArgsConstructor
public class HeartbeatController {

    private final RequestAuthHelper authHelper;
    private final HeartbeatService heartbeatService;
    private final ObjectMapper objectMapper;

    @PostMapping("/heartbeat")
    public ResponseEntity<Map<String, Boolean>> heartbeat(
        HttpServletRequest httpRequest,
        @RequestBody byte[] rawBody
    ) throws IOException {
        // Steps 1-3
        TokenEntity token = authHelper.resolveToken(httpRequest);
        authHelper.validateRequestSignature(httpRequest, token, rawBody);

        HeartbeatRequest req = objectMapper.readValue(rawBody, HeartbeatRequest.class);

        // Guard: device_id is required for all heartbeat operations
        if (req.getDeviceId() == null || req.getDeviceId().isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "device_id is required");
        }

        heartbeatService.recordHeartbeat(token, req);
        return ResponseEntity.ok(Map.of("ok", true));
    }
}
