package com.aitrack.server.controller;

import com.aitrack.server.config.AiTrackProperties;
import com.aitrack.server.dto.CreateTokenRequest;
import com.aitrack.server.dto.CreateTokenResponse;
import com.aitrack.server.service.TokenService;
import jakarta.servlet.http.HttpServletRequest;
import jakarta.validation.Valid;
import lombok.RequiredArgsConstructor;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;
import org.springframework.web.server.ResponseStatusException;

@RestController
@RequestMapping("/admin")
@RequiredArgsConstructor
public class AdminTokenController {

    private final TokenService tokenService;
    private final AiTrackProperties props;

    /**
     * Issues a new token. Requires X-Admin-Key header matching aitrack.admin-key.
     * Set aitrack.admin-key (or AITRACK_ADMIN_KEY env var) before deployment.
     */
    @PostMapping("/tokens")
    public ResponseEntity<CreateTokenResponse> createToken(
        HttpServletRequest httpRequest,
        @Valid @RequestBody CreateTokenRequest request
    ) {
        verifyAdminKey(httpRequest);
        CreateTokenResponse response = tokenService.createToken(request);
        return ResponseEntity.ok(response);
    }

    private void verifyAdminKey(HttpServletRequest request) {
        String configuredKey = props.getAdminKey();
        if (configuredKey == null || configuredKey.isBlank()) {
            // Admin key not configured — refuse all access until it is set.
            throw new ResponseStatusException(HttpStatus.SERVICE_UNAVAILABLE,
                "admin-key is not configured; set aitrack.admin-key before using this endpoint");
        }
        String provided = request.getHeader("X-Admin-Key");
        if (provided == null || !constantTimeEquals(configuredKey, provided.trim())) {
            throw new ResponseStatusException(HttpStatus.FORBIDDEN, "invalid X-Admin-Key");
        }
    }

    private static boolean constantTimeEquals(String a, String b) {
        if (a == null || b == null) return false;
        if (a.length() != b.length()) return false;
        int diff = 0;
        for (int i = 0; i < a.length(); i++) {
            diff |= a.charAt(i) ^ b.charAt(i);
        }
        return diff == 0;
    }
}
