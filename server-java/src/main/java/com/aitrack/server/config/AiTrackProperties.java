package com.aitrack.server.config;

import lombok.Data;
import org.springframework.boot.context.properties.ConfigurationProperties;
import org.springframework.stereotype.Component;

import java.util.List;

@Component
@ConfigurationProperties(prefix = "aitrack")
@Data
public class AiTrackProperties {
    private long timestampWindowSeconds = 300;
    private long rateLimitPerHour = 30;
    private long maxAddedLines = 5000;
    private RepoWhitelist repoWhitelist = new RepoWhitelist();

    /**
     * 32-byte Base64 key used to AES-256-GCM encrypt hmac_secret at rest.
     * REQUIRED in production. Generate with:
     *   openssl rand -base64 32
     * Set via environment variable AITRACK_SECRET_KEY or application.yml.
     * WARNING: rotating this key requires re-encrypting all rows in the tokens table.
     */
    private String secretKey;

    /**
     * Admin secret for POST /admin/tokens.
     * Set via environment variable AITRACK_ADMIN_KEY or application.yml.
     * Must be set before any deployment — server refuses to start if blank
     * and startupAdminKeyCheck() is called.
     */
    private String adminKey;

    @Data
    public static class RepoWhitelist {
        private boolean enforce = false;
        private List<String> urls = List.of();
    }
}
