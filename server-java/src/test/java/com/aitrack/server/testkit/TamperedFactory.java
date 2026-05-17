package com.aitrack.server.testkit;

import com.aitrack.server.dto.EditBatchRequest;
import com.aitrack.server.dto.EditDto;
import com.aitrack.server.service.SignatureService;

import java.util.List;

/**
 * Negative-example factory: produces malformed, tampered, or boundary-violating
 * objects used to verify that validation and rejection paths fire correctly.
 */
public final class TamperedFactory {

    private static final SignatureService SIG = new SignatureService();

    private TamperedFactory() {}

    /** EditDto with wrong (all-zero) record_sig — should be rejected with sig_mismatch. */
    public static EditDto badRecordSig() {
        EditDto dto = EditDtoFactory.build();
        dto.setRecordSig("0".repeat(64));
        return dto;
    }

    /** EditDto with a timestamp far in the past (simulates replay attack at the record level). */
    public static EditDto expiredTimestamp() {
        EditDto dto = EditDtoFactory.build();
        dto.setTimestamp("2020-01-01T00:00:00Z");
        // Recompute sig to keep record_sig valid, but the timestamp itself is stale
        String sig = SIG.computeRecordSig(
                EditDtoFactory.DEFAULT_HMAC_SECRET,
                EditDtoFactory.DEFAULT_TOKEN_KEY,
                EditDtoFactory.DEFAULT_DEVICE_ID,
                EditDtoFactory.DEFAULT_HOSTNAME,
                "2020-01-01T00:00:00Z",
                EditDtoFactory.DEFAULT_TOOL,
                EditDtoFactory.DEFAULT_FILE_PATH,
                EditDtoFactory.DEFAULT_REPO_URL,
                EditDtoFactory.DEFAULT_SHA,
                EditDtoFactory.DEFAULT_ADDED,
                EditDtoFactory.DEFAULT_REMOVED,
                EditDtoFactory.DEFAULT_DIFF_HUNK);
        dto.setRecordSig(sig);
        return dto;
    }

    /** EditDto with added_lines > 5000 — should be flagged as oversized. */
    public static EditDto oversizedAddedLines() {
        return EditDtoFactory.withAddedLines(9999L);
    }

    /** EditDto with tool == null — should be rejected as malformed. */
    public static EditDto nullTool() {
        EditDto dto = EditDtoFactory.build();
        dto.setTool(null);
        return dto;
    }

    /** EditDto with blank provider — should be rejected as malformed. */
    public static EditDto blankProvider() {
        EditDto dto = EditDtoFactory.build();
        dto.setProvider("   ");
        return dto;
    }

    /** EditDto with null addedLines — should be rejected as malformed. */
    public static EditDto nullAddedLines() {
        EditDto dto = EditDtoFactory.build();
        dto.setAddedLines(null);
        return dto;
    }

    /** EditDto with null removedLines — should be rejected as malformed. */
    public static EditDto nullRemovedLines() {
        EditDto dto = EditDtoFactory.build();
        dto.setRemovedLines(null);
        return dto;
    }

    /** EditDto with blank recordSig — should be rejected as malformed by EditValidator. */
    public static EditDto blankRecordSig() {
        EditDto dto = EditDtoFactory.build();
        dto.setRecordSig("");
        return dto;
    }

    /** Malformed JSON byte array (not valid JSON). */
    public static byte[] malformedJson() {
        return "{not valid json!".getBytes(java.nio.charset.StandardCharsets.UTF_8);
    }

    /** EditBatchRequest with empty edits list. */
    public static EditBatchRequest emptyEdits() {
        return EditBatchRequestFactory.withEdits(List.of());
    }

    /** EditDto with an absolute file path that will trigger path_mismatch. */
    public static EditDto absoluteFilePath() {
        String filePath = "/Users/attacker/injected/evil.rs";
        String sig = SIG.computeRecordSig(
                EditDtoFactory.DEFAULT_HMAC_SECRET,
                EditDtoFactory.DEFAULT_TOKEN_KEY,
                EditDtoFactory.DEFAULT_DEVICE_ID,
                EditDtoFactory.DEFAULT_HOSTNAME,
                EditDtoFactory.DEFAULT_TIMESTAMP,
                EditDtoFactory.DEFAULT_TOOL,
                filePath,
                EditDtoFactory.DEFAULT_REPO_URL,
                EditDtoFactory.DEFAULT_SHA,
                EditDtoFactory.DEFAULT_ADDED,
                EditDtoFactory.DEFAULT_REMOVED,
                EditDtoFactory.DEFAULT_DIFF_HUNK);
        EditDto dto = EditDtoFactory.build();
        dto.setFilePath(filePath);
        dto.setRecordSig(sig);
        return dto;
    }

    /** EditDto with a repo_url that is not in the whitelist. */
    public static EditDto unknownRepo() {
        String repoUrl = "git@github.com:unknown-org/unknown-repo.git";
        String sig = SIG.computeRecordSig(
                EditDtoFactory.DEFAULT_HMAC_SECRET,
                EditDtoFactory.DEFAULT_TOKEN_KEY,
                EditDtoFactory.DEFAULT_DEVICE_ID,
                EditDtoFactory.DEFAULT_HOSTNAME,
                EditDtoFactory.DEFAULT_TIMESTAMP,
                EditDtoFactory.DEFAULT_TOOL,
                EditDtoFactory.DEFAULT_FILE_PATH,
                repoUrl,
                EditDtoFactory.DEFAULT_SHA,
                EditDtoFactory.DEFAULT_ADDED,
                EditDtoFactory.DEFAULT_REMOVED,
                EditDtoFactory.DEFAULT_DIFF_HUNK);
        EditDto dto = EditDtoFactory.build();
        dto.setRepoUrl(repoUrl);
        dto.setRecordSig(sig);
        return dto;
    }
}
