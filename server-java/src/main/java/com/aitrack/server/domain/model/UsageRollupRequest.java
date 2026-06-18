package com.aitrack.server.domain.model;

import lombok.Data;

import java.util.List;

@Data
public class UsageRollupRequest {
    private List<UsageRollupItem> items;
}
