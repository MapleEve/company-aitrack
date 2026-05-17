package com.aitrack.server.service;

import org.springframework.stereotype.Service;

/**
 * Parses unified diff hunks to count added/removed lines,
 * then compares them against the claimed counts in the edit record.
 */
@Service
public class DiffConsistencyService {

    private static final int ALLOWED_DELTA = 1;

    public record DiffCounts(long added, long removed) {}

    /**
     * Returns true if the diff_hunk is consistent with the claimed added/removed counts.
     * Null or blank diff_hunk skips the check (returns true).
     */
    public boolean isConsistent(String diffHunk, long claimedAdded, long claimedRemoved) {
        if (diffHunk == null || diffHunk.isBlank()) {
            return true;
        }
        DiffCounts counts = parseDiffHunk(diffHunk);
        return Math.abs(counts.added() - claimedAdded) <= ALLOWED_DELTA
            && Math.abs(counts.removed() - claimedRemoved) <= ALLOWED_DELTA;
    }

    /**
     * Counts lines starting with '+' (added) and '-' (removed) in a unified diff,
     * skipping the file header lines and hunk headers ("@@...@@").
     *
     * <p>File header detection requires a trailing space: lines starting with
     * {@code "+++ "} or {@code "--- "} (with a space) are treated as unified-diff
     * file headers (e.g. {@code "--- a/path/to/file"}, {@code "+++ b/path/to/file"})
     * and are skipped.  A content line whose text is {@code "---"} or {@code "----"}
     * appears in the diff as {@code "----"} (the '-' prefix followed by the literal
     * content), which does NOT contain a space after the triple dash and is therefore
     * counted correctly as a removed line.</p>
     */
    public DiffCounts parseDiffHunk(String diffHunk) {
        long added = 0;
        long removed = 0;
        for (String line : diffHunk.split("\n", -1)) {
            // Skip unified-diff file headers: "--- a/..." and "+++ b/..."
            // Real headers always have a space between the marker and the path.
            // Content lines like "----" start with '-' but lack that space — do NOT skip them.
            if (line.startsWith("+++ ") || line.startsWith("--- ")) {
                continue;
            }
            if (line.startsWith("@@")) {
                continue;
            }
            if (line.startsWith("+")) {
                added++;
            } else if (line.startsWith("-")) {
                removed++;
            }
        }
        return new DiffCounts(added, removed);
    }
}
