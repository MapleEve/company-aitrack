package com.aitrack.server.adapter.handler;

import com.aitrack.server.application.UsageService;
import com.aitrack.server.domain.model.TokenEntity;
import com.aitrack.server.domain.model.UsageRollupItem;
import com.aitrack.server.domain.model.UsageRollupRequest;
import com.aitrack.server.domain.model.UsageSubscriptionSnapshotRequest;
import com.aitrack.server.domain.model.UsageSummary;
import com.aitrack.server.domain.service.SignatureService;
import com.aitrack.server.infrastructure.config.AiTrackProperties;
import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.servlet.http.HttpServletRequest;
import lombok.RequiredArgsConstructor;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.time.Duration;
import java.time.Instant;
import java.util.Map;
import java.util.zip.GZIPInputStream;

@RestController
@RequestMapping("/api/v1/ai-track/usage")
@RequiredArgsConstructor
public class UsageController {

    private final RequestAuthHelper authHelper;
    private final UsageService usageService;
    private final ObjectMapper objectMapper;
    private final AiTrackProperties props;
    private final SignatureService signatureService;

    @PostMapping("/rollup")
    public ResponseEntity<Map<String, Object>> submitRollups(
        HttpServletRequest httpRequest,
        @RequestBody byte[] transmittedBody
    ) throws IOException {
        byte[] rawJson = verifyAndDecode(httpRequest, transmittedBody);
        TokenEntity token = resolveAndVerifySignature(httpRequest, transmittedBody);
        UsageRollupRequest request = objectMapper.readValue(rawJson, UsageRollupRequest.class);
        validateRollup(request);
        usageService.ingestRollups(token.getTokenKey(), request);
        return ResponseEntity.ok(Map.of("ok", true, "accepted", request.getItems().size()));
    }

    @PostMapping("/subscription")
    public ResponseEntity<Map<String, Boolean>> submitSubscription(
        HttpServletRequest httpRequest,
        @RequestBody byte[] transmittedBody
    ) throws IOException {
        byte[] rawJson = verifyAndDecode(httpRequest, transmittedBody);
        TokenEntity token = resolveAndVerifySignature(httpRequest, transmittedBody);
        UsageSubscriptionSnapshotRequest request = objectMapper.readValue(rawJson, UsageSubscriptionSnapshotRequest.class);
        validateSubscription(request);
        usageService.ingestSubscription(token.getTokenKey(), request);
        return ResponseEntity.ok(Map.of("ok", true));
    }

    @GetMapping("/summary")
    public ResponseEntity<UsageSummary> summary(
        HttpServletRequest httpRequest,
        @RequestParam(required = false) String token_key,
        @RequestParam(required = false) String from_day,
        @RequestParam(required = false) String to_day,
        @RequestParam(required = false) String agent,
        @RequestParam(defaultValue = "20") int limit
    ) {
        TokenEntity token = authHelper.resolveToken(httpRequest);
        String tokenKey = token_key == null || token_key.isBlank() ? token.getTokenKey() : token_key;
        return ResponseEntity.ok(usageService.summary(tokenKey, from_day, to_day, agent, limit));
    }

    private byte[] verifyAndDecode(HttpServletRequest request, byte[] transmittedBody) throws IOException {
        if (transmittedBody.length > props.getMaxRequestBodyBytes()) {
            throw new ResponseStatusException(HttpStatus.PAYLOAD_TOO_LARGE, "request body exceeds maximum allowed size");
        }
        validateBodyDigest(request, transmittedBody);
        validateBodyTimestamp(request);

        String encoding = request.getHeader("Content-Encoding");
        if (encoding == null || encoding.isBlank() || "identity".equalsIgnoreCase(encoding.trim())) {
            return transmittedBody;
        }
        if (!"gzip".equalsIgnoreCase(encoding.trim())) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "unsupported Content-Encoding");
        }
        try (GZIPInputStream input = new GZIPInputStream(new ByteArrayInputStream(transmittedBody))) {
            return input.readAllBytes();
        }
    }

    private TokenEntity resolveAndVerifySignature(HttpServletRequest request, byte[] transmittedBody) {
        TokenEntity token = authHelper.resolveToken(request);
        authHelper.validateRequestSignature(request, token, transmittedBody);
        return token;
    }

    private void validateBodyDigest(HttpServletRequest request, byte[] transmittedBody) {
        String header = request.getHeader("X-AiTrack-Body-Sha256");
        if (header == null || header.isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "missing X-AiTrack-Body-Sha256");
        }
        String expected = signatureService.sha256Hex(transmittedBody);
        if (!constantTimeEquals(expected, header.trim())) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "invalid X-AiTrack-Body-Sha256");
        }
    }

    private void validateBodyTimestamp(HttpServletRequest request) {
        String header = request.getHeader("X-AiTrack-Body-Timestamp");
        if (header == null || header.isBlank()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "missing X-AiTrack-Body-Timestamp");
        }
        Instant timestamp;
        try {
            timestamp = Instant.parse(header.trim());
        } catch (Exception e) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "invalid X-AiTrack-Body-Timestamp");
        }
        if (Duration.between(timestamp, Instant.now()).abs().compareTo(Duration.ofMinutes(5)) > 0) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "X-AiTrack-Body-Timestamp out of window");
        }
    }

    private static void validateRollup(UsageRollupRequest request) {
        if (request.getItems() == null || request.getItems().isEmpty()) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "items array is required and must not be empty");
        }
        if (request.getItems().size() > 500) {
            throw new ResponseStatusException(HttpStatus.PAYLOAD_TOO_LARGE, "items array exceeds maximum batch size of 500");
        }
        for (UsageRollupItem item : request.getItems()) {
            if (isBlank(item.getDeviceId()) || isBlank(item.getDay()) || isBlank(item.getAgent()) || isBlank(item.getModel())) {
                throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "device_id, day, agent, and model are required");
            }
        }
    }

    private static void validateSubscription(UsageSubscriptionSnapshotRequest request) {
        if (isBlank(request.getDeviceId()) || isBlank(request.getAgent()) || isBlank(request.getReaderStatus()) || isBlank(request.getSnapshottedAt())) {
            throw new ResponseStatusException(HttpStatus.BAD_REQUEST, "device_id, agent, reader_status, and snapshotted_at are required");
        }
    }

    private static boolean isBlank(String raw) {
        return raw == null || raw.isBlank();
    }

    private static boolean constantTimeEquals(String a, String b) {
        if (a == null || b == null) return false;
        return java.security.MessageDigest.isEqual(
            a.getBytes(java.nio.charset.StandardCharsets.UTF_8),
            b.getBytes(java.nio.charset.StandardCharsets.UTF_8)
        );
    }
}
