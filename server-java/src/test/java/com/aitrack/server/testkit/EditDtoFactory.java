package com.aitrack.server.testkit;

import com.aitrack.server.dto.EditDto;
import com.aitrack.server.service.SignatureService;

import java.util.function.Consumer;

/**
 * Deterministic factory for EditDto test instances.
 * Defaults represent a valid, fully-populated claude edit.
 */
public final class EditDtoFactory {

    private static final SignatureService SIG = new SignatureService();

    public static final String DEFAULT_HMAC_SECRET = "testhmac-secret-32byteslong!!!!!";
    public static final String DEFAULT_TOKEN_KEY = "abcdef…7890";
    public static final String DEFAULT_DEVICE_ID = "device-uuid-test-001";
    public static final String DEFAULT_HOSTNAME = "MacBook-Pro.local";
    public static final String DEFAULT_TIMESTAMP = "2026-05-17T10:00:00Z";
    public static final String DEFAULT_TOOL = "claude";
    public static final String DEFAULT_FILE_PATH = "src/main.rs";
    public static final String DEFAULT_REPO_URL = "git@github.com:org/repo.git";
    public static final String DEFAULT_SHA = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2";
    public static final String DEFAULT_DIFF_HUNK = "@@ -1,3 +1,4 @@\n context\n-old line\n+new line 1\n+new line 2\n";
    public static final long DEFAULT_ADDED = 2L;
    public static final long DEFAULT_REMOVED = 1L;

    private EditDtoFactory() {}

    /** Returns a fully valid EditDto with realistic defaults. */
    public static EditDto build() {
        return buildWithSig(DEFAULT_HMAC_SECRET, DEFAULT_TOKEN_KEY,
                DEFAULT_DEVICE_ID, DEFAULT_HOSTNAME, DEFAULT_TIMESTAMP, DEFAULT_TOOL,
                DEFAULT_FILE_PATH, DEFAULT_REPO_URL, DEFAULT_SHA,
                DEFAULT_ADDED, DEFAULT_REMOVED, DEFAULT_DIFF_HUNK);
    }

    /** Build with seed-based tool name override (claude/codex/cursor). */
    public static EditDto buildForTool(String tool) {
        EditDto dto = build();
        dto.setTool(tool);
        String sig = SIG.computeRecordSig(
                DEFAULT_HMAC_SECRET, DEFAULT_TOKEN_KEY, DEFAULT_DEVICE_ID,
                DEFAULT_HOSTNAME, DEFAULT_TIMESTAMP, tool, DEFAULT_FILE_PATH, DEFAULT_REPO_URL,
                DEFAULT_SHA, DEFAULT_ADDED, DEFAULT_REMOVED, DEFAULT_DIFF_HUNK);
        dto.setRecordSig(sig);
        return dto;
    }

    /**
     * Builder-style override: build a valid default then apply customisation.
     * Callers that modify sig-bound fields must recompute record_sig themselves.
     */
    public static EditDto with(Consumer<EditDto> customizer) {
        EditDto dto = build();
        customizer.accept(dto);
        return dto;
    }

    /** Build and override added_lines, recomputing record_sig. */
    public static EditDto withAddedLines(long addedLines) {
        return buildWithSig(DEFAULT_HMAC_SECRET, DEFAULT_TOKEN_KEY,
                DEFAULT_DEVICE_ID, DEFAULT_HOSTNAME, DEFAULT_TIMESTAMP, DEFAULT_TOOL,
                DEFAULT_FILE_PATH, DEFAULT_REPO_URL, DEFAULT_SHA,
                addedLines, DEFAULT_REMOVED, DEFAULT_DIFF_HUNK);
    }

    private static EditDto buildWithSig(String hmacSecret, String tokenKey,
                                        String deviceId, String hostname,
                                        String timestamp, String tool,
                                        String filePath, String repoUrl,
                                        String sha, long added, long removed,
                                        String diffHunk) {
        EditDto dto = new EditDto();
        dto.setTool(tool);
        dto.setToolVersion("claude-code");
        dto.setProvider("anthropic");
        dto.setModel(null);
        dto.setSessionId("sess-" + Long.toHexString(42L));
        dto.setRepoUrl(repoUrl);
        dto.setBranch("main");
        dto.setCurrentSha(sha);
        dto.setFilePath(filePath);
        dto.setAddedLines(added);
        dto.setRemovedLines(removed);
        dto.setDiffHunk(diffHunk);
        dto.setMetadata(null);
        dto.setTimestamp(timestamp);
        dto.setDeviceId(deviceId);
        dto.setHostname(hostname);

        String sig = SIG.computeRecordSig(hmacSecret, tokenKey, deviceId, hostname, timestamp,
                tool, filePath, repoUrl, sha, added, removed, diffHunk);
        dto.setRecordSig(sig);
        return dto;
    }
}
