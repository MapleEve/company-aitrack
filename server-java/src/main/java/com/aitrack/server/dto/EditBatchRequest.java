package com.aitrack.server.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import jakarta.validation.Valid;
import jakarta.validation.constraints.NotBlank;
import jakarta.validation.constraints.NotEmpty;
import lombok.Data;

import java.util.List;

@Data
public class EditBatchRequest {
    @NotBlank @JsonProperty("device_id") private String deviceId;
    @JsonProperty("client_version") private String clientVersion;
    @Valid @NotEmpty private List<EditDto> edits;
}
