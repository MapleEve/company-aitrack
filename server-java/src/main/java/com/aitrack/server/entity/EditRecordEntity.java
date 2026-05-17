package com.aitrack.server.entity;

import jakarta.persistence.*;
import lombok.Data;
import lombok.NoArgsConstructor;

import java.time.Instant;

@Entity
@Table(name = "edit_records", indexes = {
    @Index(name = "idx_edit_records_token_key", columnList = "token_key"),
    @Index(name = "idx_edit_records_repo_url", columnList = "repo_url"),
    @Index(name = "idx_edit_records_device_id", columnList = "device_id"),
    @Index(name = "idx_edit_records_received_at", columnList = "received_at")
})
@Data
@NoArgsConstructor
public class EditRecordEntity {

    @Id
    @GeneratedValue(strategy = GenerationType.IDENTITY)
    private Long id;

    @Column(name = "token_key", nullable = false, length = 16)
    private String tokenKey;

    @Column(name = "device_id", nullable = false, length = 64)
    private String deviceId;

    @Column(length = 255)
    private String hostname;

    @Column(nullable = false, length = 64)
    private String tool;

    @Column(name = "tool_version", length = 64)
    private String toolVersion;

    @Column(nullable = false, length = 64)
    private String provider;

    @Column(length = 128)
    private String model;

    @Column(name = "session_id", nullable = false, length = 128)
    private String sessionId;

    @Column(name = "repo_url", nullable = false, length = 512)
    private String repoUrl;

    @Column(nullable = false, length = 128)
    private String branch;

    @Column(name = "current_sha", nullable = false, length = 128)
    private String currentSha;

    @Column(name = "file_path", nullable = false, length = 1024)
    private String filePath;

    @Column(name = "added_lines", nullable = false)
    private long addedLines;

    @Column(name = "removed_lines", nullable = false)
    private long removedLines;

    @Column(name = "diff_hunk", columnDefinition = "TEXT")
    private String diffHunk;

    @Column(columnDefinition = "TEXT")
    private String metadata;

    @Column(nullable = false, length = 64)
    private String timestamp;

    @Column(name = "record_sig", nullable = false, length = 64)
    private String recordSig;

    @Enumerated(EnumType.STRING)
    @Column(nullable = false, length = 16)
    private RecordStatus status;

    // Comma-separated flag reasons, e.g. "diff_inconsistent,repo_unknown"
    @Column(name = "flags", length = 512)
    private String flags;

    @Column(name = "received_at", nullable = false)
    private Instant receivedAt = Instant.now();

    public enum RecordStatus {
        ACCEPTED, FLAGGED, REJECTED
    }
}
