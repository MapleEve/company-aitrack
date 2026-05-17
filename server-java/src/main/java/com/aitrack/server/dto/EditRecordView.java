package com.aitrack.server.dto;

import com.aitrack.server.entity.EditRecordEntity;
import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

/**
 * Read-side view of an EditRecord with explicit snake_case JSON keys.
 * Mirrors the Go model.EditRecord json tags exactly.
 */
@Data
@NoArgsConstructor
public class EditRecordView {

    private long id;

    @JsonProperty("token_key")
    private String tokenKey;

    @JsonProperty("device_id")
    private String deviceId;

    private String hostname;

    private String tool;

    @JsonProperty("tool_version")
    private String toolVersion;

    private String provider;

    private String model;

    @JsonProperty("session_id")
    private String sessionId;

    @JsonProperty("repo_url")
    private String repoUrl;

    private String branch;

    @JsonProperty("current_sha")
    private String currentSha;

    @JsonProperty("file_path")
    private String filePath;

    @JsonProperty("added_lines")
    private long addedLines;

    @JsonProperty("removed_lines")
    private long removedLines;

    @JsonProperty("diff_hunk")
    private String diffHunk;

    private String metadata;

    private String timestamp;

    @JsonProperty("record_sig")
    private String recordSig;

    private String status;

    private String flags;

    @JsonProperty("received_at")
    private Instant receivedAt;

    public static EditRecordView from(EditRecordEntity e) {
        EditRecordView v = new EditRecordView();
        v.id = e.getId();
        v.tokenKey = e.getTokenKey();
        v.deviceId = e.getDeviceId();
        v.hostname = e.getHostname();
        v.tool = e.getTool();
        v.toolVersion = e.getToolVersion();
        v.provider = e.getProvider();
        v.model = e.getModel();
        v.sessionId = e.getSessionId();
        v.repoUrl = e.getRepoUrl();
        v.branch = e.getBranch();
        v.currentSha = e.getCurrentSha();
        v.filePath = e.getFilePath();
        v.addedLines = e.getAddedLines();
        v.removedLines = e.getRemovedLines();
        v.diffHunk = e.getDiffHunk();
        v.metadata = e.getMetadata();
        v.timestamp = e.getTimestamp();
        v.recordSig = e.getRecordSig();
        v.status = e.getStatus() != null ? e.getStatus().name() : null;
        v.flags = e.getFlags();
        v.receivedAt = e.getReceivedAt();
        return v;
    }
}
