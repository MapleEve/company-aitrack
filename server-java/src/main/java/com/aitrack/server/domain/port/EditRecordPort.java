package com.aitrack.server.domain.port;

import com.aitrack.server.domain.model.EditRecordEntity;
import com.aitrack.server.domain.model.PageResult;

import java.time.Instant;
import java.util.List;

/**
 * Driven-side persistence port for edit records.
 *
 * <p>This is a pure domain interface (a secondary port of the hexagon).
 * The {@code adapter.db.EditRecordRepository} Spring Data interface provides the
 * concrete implementation; domain and application code depend only on this port.
 *
 * <p>Pagination is expressed with plain {@code int page} / {@code int size} primitives
 * rather than {@code org.springframework.data.domain.Pageable} to prevent Spring Data
 * infrastructure types from leaking into the domain layer.
 */
public interface EditRecordPort {

    /** Persists a single edit record; returns the saved instance. */
    EditRecordEntity save(EditRecordEntity record);

    /** Counts records for a token+file within a rolling window (rate limiting). */
    long countByTokenKeyAndFilePathSince(String tokenKey, String filePath, Instant since);

    /** Returns a page of records optionally filtered by token key and repo URL. */
    PageResult<EditRecordEntity> findByFilters(String tokenKey, String repoUrl, int page, int size);

    /** Loads all non-rejected records for a token, excluding the given status. */
    List<EditRecordEntity> findByTokenKeyAndStatusNot(String tokenKey, EditRecordEntity.RecordStatus status);

    /** Aggregates stats grouped by token key. */
    List<Object[]> aggregateByTokenKey();

    /** Aggregates stats grouped by repo URL. */
    List<Object[]> aggregateByRepo();

    /** Aggregates stats grouped by device ID. */
    List<Object[]> aggregateByDevice();

    /** Aggregates stats grouped by hostname. */
    List<Object[]> aggregateByHostname();

    /** Aggregates stats grouped by tool name. */
    List<Object[]> aggregateByTool();
}
