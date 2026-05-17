package com.aitrack.server.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Data;

import java.time.Instant;

@Data
@AllArgsConstructor
public class StatsRow {
    private String group;
    private long edits;
    @JsonProperty("added_lines") private long addedLines;
    @JsonProperty("removed_lines") private long removedLines;
    private long accepted;
    private long flagged;
    private long rejected;
    @JsonProperty("last_active") private Instant lastActive;
}
