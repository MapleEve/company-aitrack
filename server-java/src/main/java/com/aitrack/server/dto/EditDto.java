package com.aitrack.server.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotNull;
import lombok.Data;

@Data
public class EditDto {
    @NotBlank private String tool;
    @JsonProperty("tool_version") private String toolVersion;
    @NotBlank private String provider;
    private String model;
    @NotBlank @JsonProperty("session_id") private String sessionId;
    @NotBlank @JsonProperty("repo_url") private String repoUrl;
    @NotBlank private String branch;
    @NotBlank @JsonProperty("current_sha") private String currentSha;
    @NotBlank @JsonProperty("file_path") private String filePath;
    @NotNull @JsonProperty("added_lines") private Long addedLines;
    @NotNull @JsonProperty("removed_lines") private Long removedLines;
    @JsonProperty("diff_hunk") private String diffHunk;
    private String metadata;
    @NotBlank private String timestamp;
    @NotBlank @JsonProperty("device_id") private String deviceId;
    @NotBlank private String hostname;
    @NotBlank @JsonProperty("record_sig") private String recordSig;
}
