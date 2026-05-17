package com.aitrack.server.service;

import com.aitrack.server.dto.EditDto;
import org.springframework.stereotype.Component;

/**
 * Explicit field validation for EditDto after manual JSON deserialization.
 * Bean Validation (@NotNull/@NotBlank) is bypassed because controllers receive
 * raw byte[] bodies for HMAC verification; this class fills that gap.
 *
 * Any edit that fails here is added to the rejected list with reason "malformed"
 * before computeRecordSig is called, preventing NPE from Long unboxing.
 */
@Component
public class EditValidator {

    /**
     * Returns null if the edit is valid, or a non-null reason string if it is malformed.
     * Checked fields mirror the @NotBlank/@NotNull annotations on EditDto.
     */
    public String validate(EditDto edit) {
        if (edit == null) return "malformed";
        if (isBlank(edit.getTool()))       return "malformed";
        if (isBlank(edit.getProvider()))   return "malformed";
        if (isBlank(edit.getSessionId()))  return "malformed";
        if (isBlank(edit.getFilePath()))   return "malformed";
        if (isBlank(edit.getTimestamp()))  return "malformed";
        if (isBlank(edit.getDeviceId()))   return "malformed";
        if (isBlank(edit.getHostname()))   return "malformed";
        if (isBlank(edit.getRepoUrl()))    return "malformed";
        if (isBlank(edit.getBranch()))     return "malformed";
        if (isBlank(edit.getCurrentSha())) return "malformed";
        if (isBlank(edit.getRecordSig()))  return "malformed";
        if (edit.getAddedLines() == null)  return "malformed";
        if (edit.getRemovedLines() == null) return "malformed";
        return null;
    }

    private static boolean isBlank(String s) {
        return s == null || s.isBlank();
    }
}
