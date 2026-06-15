package com.aitrack.server.adapter.db;

import com.aitrack.server.domain.model.EditRecordEntity;
import com.aitrack.server.domain.model.PageResult;
import com.aitrack.server.domain.port.EditRecordPort;
import org.springframework.data.domain.Page;
import org.springframework.data.domain.PageRequest;
import org.springframework.data.domain.Pageable;
import org.springframework.data.domain.Sort;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.data.jpa.repository.Query;
import org.springframework.data.repository.query.Param;
import org.springframework.stereotype.Repository;

import java.time.Instant;
import java.util.List;

/**
 * Spring Data JPA persistence adapter implementing {@link EditRecordPort}.
 *
 * <p>Spring Data's {@link Page} and {@link Pageable} are confined to this adapter.
 * The port-facing method {@link #findByFilters} converts the internal {@code Page}
 * into the framework-agnostic {@link PageResult} before returning.
 */
@Repository
public interface EditRecordRepository extends JpaRepository<EditRecordEntity, Long>, EditRecordPort {

    // Most-specific re-declaration resolves the overload clash between
    // JpaRepository's generic save(S) and the EditRecordPort save method.
    @Override
    EditRecordEntity save(EditRecordEntity record);

    // Rate limit query by diffHunkHash stored separately — simpler approach: count by tokenKey+filePath+receivedAt
    @Query("SELECT COUNT(e) FROM EditRecordEntity e WHERE e.tokenKey = :tokenKey AND e.filePath = :filePath AND e.receivedAt >= :since")
    long countByTokenKeyAndFilePathSince(
        @Param("tokenKey") String tokenKey,
        @Param("filePath") String filePath,
        @Param("since") Instant since
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
