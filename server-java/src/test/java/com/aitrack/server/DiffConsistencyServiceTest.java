package com.aitrack.server;

import com.aitrack.server.service.DiffConsistencyService;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

class DiffConsistencyServiceTest {

    private DiffConsistencyService service;

    @BeforeEach
    void setUp() {
        service = new DiffConsistencyService();
    }

    @Test
    void parseDiffHunk_basicCounts() {
        String diff = "@@ -1,3 +1,4 @@\n-line1\n-line2\n+newline1\n+newline2\n+newline3\n context\n";
        DiffConsistencyService.DiffCounts counts = service.parseDiffHunk(diff);
        assertThat(counts.added()).isEqualTo(3);
        assertThat(counts.removed()).isEqualTo(2);
    }

    @Test
    void parseDiffHunk_skipsFileHeaders() {
        String diff = "--- a/file.rs\n+++ b/file.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        DiffConsistencyService.DiffCounts counts = service.parseDiffHunk(diff);
        assertThat(counts.added()).isEqualTo(1);
        assertThat(counts.removed()).isEqualTo(1);
    }

    @Test
    void parseDiffHunk_emptyDiff() {
        DiffConsistencyService.DiffCounts counts = service.parseDiffHunk("@@ -1,0 +1,0 @@\n");
        assertThat(counts.added()).isEqualTo(0);
        assertThat(counts.removed()).isEqualTo(0);
    }

    @Test
    void isConsistent_exactMatch() {
        String diff = "@@ -1,2 +1,3 @@\n-a\n-b\n+x\n+y\n+z\n";
        assertThat(service.isConsistent(diff, 3, 2)).isTrue();
    }

    @Test
    void isConsistent_withinAllowedDelta() {
        String diff = "@@ -1,2 +1,3 @@\n-a\n-b\n+x\n+y\n+z\n";
        // claimed: 3+1=4 added, actual 3 — delta=1, allowed
        assertThat(service.isConsistent(diff, 4, 2)).isTrue();
    }

    @Test
    void isConsistent_exceedsDelta() {
        String diff = "@@ -1,2 +1,3 @@\n-a\n-b\n+x\n+y\n+z\n";
        // claimed: 10 added, actual 3 — delta=7, not allowed
        assertThat(service.isConsistent(diff, 10, 2)).isFalse();
    }

    @Test
    void isConsistent_nullDiffSkipsCheck() {
        // null diff_hunk → no check, always consistent
        assertThat(service.isConsistent(null, 999, 999)).isTrue();
    }

    @Test
    void isConsistent_blankDiffSkipsCheck() {
        assertThat(service.isConsistent("   ", 999, 999)).isTrue();
    }

    @Test
    void parseDiffHunk_fullFileHunk() {
        // Full-file hunk: @@ -1,N +1,M @@ with all old lines prefixed '-' and new prefixed '+'
        String diff = "@@ -1,2 +1,3 @@\n-old1\n-old2\n+new1\n+new2\n+new3\n";
        DiffConsistencyService.DiffCounts counts = service.parseDiffHunk(diff);
        assertThat(counts.added()).isEqualTo(3);
        assertThat(counts.removed()).isEqualTo(2);
    }

    /**
     * Real-data regression: a removed line whose content is "---" (markdown/rst horizontal
     * rule) appears in the diff as "----" (the '-' prefix followed by the literal text "---").
     * The old code matched startsWith("---") and skipped it, under-counting removed lines and
     * triggering a false diff_inconsistent flag.  With the space-guarded fix the line is
     * correctly counted as removed.
     */
    @Test
    void parseDiffHunk_removedLineContentIsDashes_notSkippedAsFileHeader() {
        // The deleted line is "---" (e.g. an RST/Markdown horizontal rule).
        // In unified-diff format it becomes "----" (prefix '-' + content "---").
        // It must be counted as a removed line, not skipped as a file header.
        String diff = "--- a/docs/README.rst\n+++ b/docs/README.rst\n"
                + "@@ -5,4 +5,3 @@\n"
                + " normal context line\n"
                + "----\n"          // deleted line whose content is "---"
                + " another context\n"
                + "+replacement line\n";
        DiffConsistencyService.DiffCounts counts = service.parseDiffHunk(diff);
        assertThat(counts.removed()).isEqualTo(1);
        assertThat(counts.added()).isEqualTo(1);
    }

    @Test
    void parseDiffHunk_removedLineContentIsFourDashes_counted() {
        // Content "----" (four dashes) in the diff becomes "-----"; also must not be skipped.
        String diff = "@@ -1,2 +1,1 @@\n"
                + "-----\n"         // deleted line whose content is "----"
                + " kept line\n";
        DiffConsistencyService.DiffCounts counts = service.parseDiffHunk(diff);
        assertThat(counts.removed()).isEqualTo(1);
        assertThat(counts.added()).isEqualTo(0);
    }
}
