package com.aitrack.server.adapter.db;

import com.aitrack.server.domain.model.EditRecordEntity;
import com.aitrack.server.domain.model.PageResult;
import com.aitrack.server.domain.port.EditRecordPort;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Pageable;
import org.springframework.data.domain.Sort;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Modifying;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;
import org.springframework.transaction.annotation.Transactional;

import java.time.Instant;
import java.util.List;
import java.util.Optional;

/**
 * Spring Data JPA persistence adapter implementing {@link EditRecordPort}.
 *
 * <p>Spring Data's {@link Page} and {@link Pageable} are confined to this adapter.
 * The port-facing method {@link #findByFilters} converts the internal {@code Page}
 * into the framework-agnostic {@link PageResult} before returning.
 */
@Repository
public interface EditRecordRepository extends JpaRepository<EditRecordEntity, Long>, EditRecordPort {

    String RAW_FIELD_STRIPPED_MARKER = "[stripped by edit retention]";
    int DEFAULT_RAW_RETENTION_DAYS = 30;
    int DEFAULT_MAX_RAW_ROWS = 10_000;

    // Most-specific re-declaration resolves the overload clash between
    // JpaRepository's generic save(S) and the EditRecordPort save method.
    @Override
    EditRecordEntity save(EditRecordEntity record);

    @Override
    boolean existsByRecordSig(String recordSig);

    Optional<EditRecordEntity> findByRecordSig(String recordSig);

    // Rate limit query by diffHunkHash stored separately — simpler approach: count by tokenKey+filePath+receivedAt
    @Query("SELECT COUNT(e) FROM EditRecordEntity e WHERE e.tokenKey = :tokenKey AND e.filePath = :filePath AND e.receivedAt >= :since")
    long countByTokenKeyAndFilePathSince(
        @Param("tokenKey") String tokenKey,
        @Param("filePath") String filePath,
        @Param("since") Instant since
    );

    @Override
    @Transactional
    default int applyRawRetention(Instant now) {
        return applyRawRetentionWithPolicy(now, DEFAULT_RAW_RETENTION_DAYS, DEFAULT_MAX_RAW_ROWS);
    }

    @Transactional
    default int applyRawRetentionWithPolicy(Instant now, int rawRetentionDays, int maxRawRows) {
        int normalizedDays = rawRetentionDays > 0 ? rawRetentionDays : DEFAULT_RAW_RETENTION_DAYS;
        int normalizedRows = maxRawRows > 0 ? maxRawRows : DEFAULT_MAX_RAW_ROWS;
        Instant cutoff = now.minusSeconds(normalizedDays * 24L * 60L * 60L);
        return stripRawFieldsOlderThan(cutoff, RAW_FIELD_STRIPPED_MARKER)
            + stripRawFieldsBeyondNewestRows(normalizedRows, RAW_FIELD_STRIPPED_MARKER);
    }

    @Modifying(clearAutomatically = true, flushAutomatically = true)
    @Query("""
        UPDATE EditRecordEntity e
           SET e.diffHunk = CASE WHEN e.diffHunk IS NULL OR e.diffHunk = '' THEN e.diffHunk ELSE :marker END,
               e.metadata = CASE WHEN e.metadata IS NULL OR e.metadata = '' THEN e.metadata ELSE :marker END,
               e.promptSummary = CASE WHEN e.promptSummary IS NULL OR e.promptSummary = '' THEN e.promptSummary ELSE :marker END
         WHERE e.receivedAt < :cutoff
           AND (
               (e.diffHunk IS NOT NULL AND e.diffHunk <> '' AND e.diffHunk <> :marker)
               OR (e.metadata IS NOT NULL AND e.metadata <> '' AND e.metadata <> :marker)
               OR (e.promptSummary IS NOT NULL AND e.promptSummary <> '' AND e.promptSummary <> :marker)
           )
        """)
    int stripRawFieldsOlderThan(
        @Param("cutoff") Instant cutoff,
        @Param("marker") String marker
    );

    @Modifying(clearAutomatically = true, flushAutomatically = true)
    @Query(value = """
        UPDATE edit_records
           SET diff_hunk = CASE WHEN diff_hunk IS NULL OR diff_hunk = '' THEN diff_hunk ELSE :marker END,
               metadata = CASE WHEN metadata IS NULL OR metadata = '' THEN metadata ELSE :marker END,
               prompt_summary = CASE WHEN prompt_summary IS NULL OR prompt_summary = '' THEN prompt_summary ELSE :marker END
         WHERE id NOT IN (
               SELECT id FROM (
                   SELECT id FROM edit_records ORDER BY received_at DESC, id DESC LIMIT :maxRows
               ) retained
           )
           AND (
               (diff_hunk IS NOT NULL AND diff_hunk <> '' AND diff_hunk <> :marker)
               OR (metadata IS NOT NULL AND metadata <> '' AND metadata <> :marker)
               OR (prompt_summary IS NOT NULL AND prompt_summary <> '' AND prompt_summary <> :marker)
           )
        """, nativeQuery = true)
    int stripRawFieldsBeyondNewestRows(
        @Param("maxRows") int maxRows,
        @Param("marker") String marker
    );

    Page<EditRecordEntity> findByTokenKey(String tokenKey, Pageable pageable);
    Page<EditRecordEntity> findByRepoUrl(String repoUrl, Pageable pageable);

    @Query("SELECT e FROM EditRecordEntity e WHERE (:tokenKey IS NULL OR e.tokenKey = :tokenKey) AND (:repoUrl IS NULL OR e.repoUrl = :repoUrl)")
    Page<EditRecordEntity> findByFiltersInternal(
        @Param("tokenKey") String tokenKey,
        @Param("repoUrl") String repoUrl,
        Pageable pageable
    );

    /** Implements the port method; converts Spring {@link Page} to framework-agnostic {@link PageResult}. */
    @Override
    default PageResult<EditRecordEntity> findByFilters(String tokenKey, String repoUrl, int page, int size) {
        Pageable pageable = PageRequest.of(
            Math.max(0, page),
            Math.min(100, Math.max(1, size)),
            Sort.by("receivedAt").descending()
        );
        Page<EditRecordEntity> springPage = findByFiltersInternal(tokenKey, repoUrl, pageable);
        return new PageResult<>(springPage.getContent(), springPage.getTotalElements());
    }

    // Stats aggregation queries
    @Query("SELECT e.tokenKey, COUNT(e), SUM(e.addedLines), SUM(e.removedLines), " +
           "SUM(CASE WHEN e.status = 'ACCEPTED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'FLAGGED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'REJECTED' THEN 1 ELSE 0 END), " +
           "MAX(e.receivedAt) FROM EditRecordEntity e GROUP BY e.tokenKey")
    java.util.List<Object[]> aggregateByTokenKey();

    @Query("SELECT e.repoUrl, COUNT(e), SUM(e.addedLines), SUM(e.removedLines), " +
           "SUM(CASE WHEN e.status = 'ACCEPTED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'FLAGGED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'REJECTED' THEN 1 ELSE 0 END), " +
           "MAX(e.receivedAt) FROM EditRecordEntity e GROUP BY e.repoUrl")
    java.util.List<Object[]> aggregateByRepo();

    @Query("SELECT e.deviceId, COUNT(e), SUM(e.addedLines), SUM(e.removedLines), " +
           "SUM(CASE WHEN e.status = 'ACCEPTED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'FLAGGED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'REJECTED' THEN 1 ELSE 0 END), " +
           "MAX(e.receivedAt) FROM EditRecordEntity e GROUP BY e.deviceId")
    java.util.List<Object[]> aggregateByDevice();

    @Query("SELECT e.hostname, COUNT(e), SUM(e.addedLines), SUM(e.removedLines), " +
           "SUM(CASE WHEN e.status = 'ACCEPTED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'FLAGGED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'REJECTED' THEN 1 ELSE 0 END), " +
           "MAX(e.receivedAt) FROM EditRecordEntity e GROUP BY e.hostname")
    java.util.List<Object[]> aggregateByHostname();

    @Query("SELECT e.tool, COUNT(e), SUM(e.addedLines), SUM(e.removedLines), " +
           "SUM(CASE WHEN e.status = 'ACCEPTED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'FLAGGED' THEN 1 ELSE 0 END), " +
           "SUM(CASE WHEN e.status = 'REJECTED' THEN 1 ELSE 0 END), " +
           "MAX(e.receivedAt) FROM EditRecordEntity e GROUP BY e.tool")
    java.util.List<Object[]> aggregateByTool();

    // BM25 full-text search via ParadeDB — only functional on the postgres profile.
    // Will fail if invoked against H2; not wired to any controller yet (Phase DB-2).
    @Query(value = "SELECT * FROM edit_records WHERE diff_hunk ||| :query ORDER BY paradedb.score(id) DESC LIMIT :limit", nativeQuery = true)
    List<EditRecordEntity> searchBm25(@Param("query") String query, @Param("limit") int limit);

    // Phase 3: used by ProfileService to load all non-rejected records for a token
    List<EditRecordEntity> findByTokenKeyAndStatusNot(String tokenKey, EditRecordEntity.RecordStatus status);
}
