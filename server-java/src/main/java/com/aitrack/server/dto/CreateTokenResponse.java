package com.aitrack.server.dto;

import com.fasterxml.jackson.annotation.JsonProperty;
import lombok.AllArgsConstructor;
import lombok.Data;

@Data
@AllArgsConstructor
public class CreateTokenResponse {
    private String token;
    @JsonProperty("hmac_secret") private String hmacSecret;
    @JsonProperty("token_key") private String tokenKey;
}
