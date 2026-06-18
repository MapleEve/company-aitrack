package com.aitrack.server.domain.service;

import com.aitrack.server.domain.model.ProfileDto;
import com.aitrack.server.domain.model.ProfileDto.DayCount;
import com.aitrack.server.domain.model.ProfileDto.DepthStats;
import com.aitrack.server.domain.model.ProfileDto.FrequencyStats;
import com.aitrack.server.domain.model.EditRecordEntity;
import com.aitrack.server.domain.model.EditRecordEntity.RecordStatus;
import com.aitrack.server.domain.model.TokenEntity;
import com.aitrack.server.domain.port.EditRecordPort;
import com.aitrack.server.domain.port.TokenPort;
import lombok.RequiredArgsConstructor;
import lombok.extern.slf4j.Slf4j;
import org.springframework.stereotype.Service;

import java.time.Instant;
import java.time.LocalDate;
import java.time.ZoneOffset;
import java.util.*;
import java.util.stream.Collectors;

@Slf4j
@Service
@RequiredArgsConstructor
public class ProfileService {

    private final EditRecordPort editRecordRepo;
    private final TokenPort tokenRepo;

    /**
     * Compute a developer AI usage profile for the given tokenKey.
     *
     * @param tokenKey the 16-character token key (aitrack format)
     * @return Optional.empty() when neither records nor an active token exist; otherwise Optional.of(profile)
     */
    public Optional<ProfileDto> computeProfile(String tokenKey) {
        List<EditRecordEntity> records =
                editRecordRepo.findByTokenKeyAndStatusNot(tokenKey, RecordStatus.REJECTED);
        Optional<TokenEntity> tokenOpt = tokenRepo.findByTokenKeyAndActiveTrue(tokenKey);

        // 404: no records AND no active token
        if (records.isEmpty() && tokenOpt.isEmpty()) {
            return Optional.empty();
        }

        ProfileDto profile = new ProfileDto();
        profile.setTokenKey(tokenKey);
        profile.setGeneratedAt(Instant.now().toString());

        // Owner from token entity
        tokenOpt.ifPresent(t -> profile.setOwner(t.getOwner()));

        if (records.isEmpty()) {
            // Token exists but no records — return zero-value profile
            profile.setTotalEdits(0);
            profile.setTotalAddedLines(0);
            profile.setTotalRemovedLines(0);
            return Optional.of(profile);
        }

        // ------------------------------------------------------------------
        // Basic aggregates
        // ------------------------------------------------------------------
        profile.setTotalEdits(records.size());
        profile.setTotalAddedLines(records.stream().mapToLong(EditRecordEntity::getAddedLines).sum());
        profile.setTotalRemovedLines(records.stream().mapToLong(EditRecordEntity::getRemovedLines).sum());

        // ------------------------------------------------------------------
        // Time bounds (first_seen / last_seen)
        // ------------------------------------------------------------------
        List<Instant> instants = new ArrayList<>();
        for (EditRecordEntity e : records) {
            instants.add(parseTimestamp(e));
        }
        instants.sort(Comparator.naturalOrder());
        if (!instants.isEmpty()) {
            profile.setFirstSeen(instants.get(0).toString());
            profile.setLastSeen(instants.get(instants.size() - 1).toString());
        }

        // ------------------------------------------------------------------
        // Frequency stats
        // ------------------------------------------------------------------
        profile.setFrequency(computeFrequency(records));

        // ------------------------------------------------------------------
        // Depth stats
        // ------------------------------------------------------------------
        profile.setDepth(computeDepth(records));

        // ------------------------------------------------------------------
        // Language breakdown
        // ------------------------------------------------------------------
        profile.setLanguages(computeLanguages(records));

        // ------------------------------------------------------------------
        // Comment density (set into DepthStats after depth is computed)
        // ------------------------------------------------------------------
        ProfileDto.DepthStats depthStats = profile.getDepth();
        if (depthStats != null) {
            depthStats.setCommentDensity(computeCommentDensity(records));
        }

        // ------------------------------------------------------------------
        // Tools breakdown (skip null tools)
        // ------------------------------------------------------------------
        Map<String, Long> tools = records.stream()
                .filter(e -> e.getTool() != null)
                .collect(Collectors.groupingBy(
                        EditRecordEntity::getTool,
                        Collectors.counting()
                ));
        profile.setTools(tools.isEmpty() ? null : tools);

        // ------------------------------------------------------------------
        // Prompt patterns
        // ------------------------------------------------------------------
        profile.setPromptPatterns(computePromptPatterns(records));

        return Optional.of(profile);
    }

    // -------------------------------------------------------------------------
    // Frequency computation
    // -------------------------------------------------------------------------

    private FrequencyStats computeFrequency(List<EditRecordEntity> records) {
        Instant now = Instant.now();
        Instant cutoff30d = now.minusSeconds(30L * 24 * 3600);
        Instant cutoff84d = now.minusSeconds(84L * 24 * 3600);

        long count30d = 0;
        long count84d = 0;
        Map<LocalDate, Long> dailyCounts = new TreeMap<>();

        for (EditRecordEntity e : records) {
            Instant ts = parseTimestamp(e);
            if (!ts.isBefore(cutoff30d)) {
                count30d++;
                LocalDate day = ts.atZone(ZoneOffset.UTC).toLocalDate();
                dailyCounts.merge(day, 1L, Long::sum);
            }
            if (!ts.isBefore(cutoff84d)) {
                count84d++;
            }
        }

        FrequencyStats freq = new FrequencyStats();
        freq.setDailyAvg30d(count30d / 30.0);
        freq.setWeeklyAvg12w(count84d / 12.0);

        List<DayCount> trend = dailyCounts.entrySet().stream()
                .map(entry -> new DayCount(entry.getKey().toString(), entry.getValue()))
                .collect(Collectors.toList());
        freq.setDailyTrend(trend);

        return freq;
    }

    // -------------------------------------------------------------------------
    // Depth computation
    // -------------------------------------------------------------------------

    private DepthStats computeDepth(List<EditRecordEntity> records) {
        List<Long> totals = records.stream()
                .map(e -> e.getAddedLines() + e.getRemovedLines())
                .sorted()
                .collect(Collectors.toList());

        DepthStats depth = new DepthStats();

        if (totals.isEmpty()) {
            return depth;
        }

        double avg = totals.stream().mapToLong(Long::longValue).average().orElse(0.0);
        depth.setAvgLines(avg);
        depth.setP50Lines(totals.get(totals.size() / 2));
        depth.setP90Lines(totals.get((int) (totals.size() * 0.9)));
        depth.setSmallCount(totals.stream().filter(t -> t < 10).count());
        depth.setMediumCount(totals.stream().filter(t -> t >= 10 && t <= 100).count());
        depth.setLargeCount(totals.stream().filter(t -> t > 100).count());

        return depth;
    }

    // -------------------------------------------------------------------------
    // Language detection
    // -------------------------------------------------------------------------

    /**
     * Detects the programming language from a file path based on its extension.
     *
     * @param filePath the file path to examine (may be null/blank)
     * @return language name string, e.g. "Python", "TypeScript", or "Other"
     */
    String detectLanguage(String filePath) {
        if (filePath == null || filePath.isBlank()) {
            return "Other";
        }
        String lower = filePath.toLowerCase();

        if (lower.endsWith(".py"))                                                  return "Python";
        if (lower.endsWith(".ts") || lower.endsWith(".tsx"))                       return "TypeScript";
        if (lower.endsWith(".js") || lower.endsWith(".jsx"))                       return "JavaScript";
        if (lower.endsWith(".java"))                                                return "Java";
        if (lower.endsWith(".go"))                                                  return "Go";
        if (lower.endsWith(".rs"))                                                  return "Rust";
        if (lower.endsWith(".cpp") || lower.endsWith(".cc") || lower.endsWith(".cxx") || lower.endsWith(".c"))
                                                                                    return "C/C++";
        if (lower.endsWith(".cs"))                                                  return "C#";
        if (lower.endsWith(".rb"))                                                  return "Ruby";
        if (lower.endsWith(".php"))                                                 return "PHP";
        if (lower.endsWith(".swift"))                                               return "Swift";
        if (lower.endsWith(".kt") || lower.endsWith(".kts"))                       return "Kotlin";
        if (lower.endsWith(".scala"))                                               return "Scala";
        if (lower.endsWith(".vue"))                                                 return "Vue";
        if (lower.endsWith(".html") || lower.endsWith(".htm"))                     return "HTML";
        if (lower.endsWith(".css") || lower.endsWith(".scss") || lower.endsWith(".sass") || lower.endsWith(".less"))
                                                                                    return "CSS";
        if (lower.endsWith(".sql"))                                                 return "SQL";
        if (lower.endsWith(".sh") || lower.endsWith(".bash") || lower.endsWith(".zsh"))
                                                                                    return "Shell";
        if (lower.endsWith(".yaml") || lower.endsWith(".yml"))                     return "YAML";
        if (lower.endsWith(".json"))                                                return "JSON";
        if (lower.endsWith(".xml"))                                                 return "XML";
        if (lower.endsWith(".md") || lower.endsWith(".rst") || lower.endsWith(".txt"))
                                                                                    return "Docs";

        return "Other";
    }

    /**
     * Groups records by detected language and returns a count per language.
     *
     * @param records non-null list of edit records
     * @return map from language name to occurrence count
     */
    private Map<String, Long> computeLanguages(List<EditRecordEntity> records) {
        return records.stream()
                .collect(Collectors.groupingBy(
                        e -> detectLanguage(e.getFilePath()),
                        Collectors.counting()
                ));
    }

    /**
     * Computes the comment density across all records.
     * <p>
     * For each record the {@code diffHunk} field is inspected. Lines that begin
     * with {@code +} (added lines in unified-diff format) are counted as total
     * added lines; among those, lines whose content (after stripping the leading
     * {@code +}) starts with a known comment prefix are counted as comment lines.
     *
     * @param records non-null list of edit records
     * @return ratio of comment lines to total added lines, or 0.0 if no added lines
     */
    double computeCommentDensity(List<EditRecordEntity> records) {
        long totalAdded = 0;
        long commentLines = 0;

        for (EditRecordEntity record : records) {
            String hunk = record.getDiffHunk();
            if (hunk == null) {
                continue;
            }
            for (String line : hunk.split("\n")) {
                if (!line.startsWith("+")) {
                    continue;
                }
                totalAdded++;
                String content = line.substring(1).stripLeading();
                if (content.startsWith("//")
                        || content.startsWith("#")
                        || content.startsWith("/*")
                        || content.startsWith("* ")
                        || content.startsWith("*/")
                        || content.startsWith("/**")
                        || content.startsWith("\"\"\"")
                        || content.startsWith("'''")
                        || content.startsWith("--")
                        || content.startsWith("<!--")) {
                    commentLines++;
                }
            }
        }

        if (totalAdded == 0) {
            return 0.0;
        }
        return (double) commentLines / totalAdded;
    }

    // -------------------------------------------------------------------------
    // Prompt pattern classification
    // -------------------------------------------------------------------------

    private static final Map<String, List<String>> PROMPT_KEYWORDS = Map.of(
        "generate",  List.of("generate", "create", "write", "implement", "add", "build", "make",
                              "生成", "创建", "新增", "写", "实现", "构建"),
        "fix_debug", List.of("fix", "bug", "error", "debug", "issue", "wrong", "broken",
                              "修复", "错误", "调试", "修", "问题", "故障", "报错"),
        "refactor",  List.of("refactor", "clean", "improve", "optimize", "restructure",
                              "重构", "优化", "改进", "简化", "整理"),
        "explain",   List.of("explain", "what", "how", "why", "describe",
                              "解释", "什么", "怎么", "为什么", "说明"),
        "test",      List.of("test", "spec", "mock", "coverage", "unit",
                              "测试", "单元测试", "集成测试", "断言", "覆盖"),
        "other",     List.of()
    );

    /**
     * Classifies a prompt intent label or legacy summary string into an intent category.
     * Priority order: fix_debug > test > refactor > explain > generate > other
     */
    String classifyPrompt(String prompt) {
        if (prompt == null || prompt.isBlank()) return "other";
        String lower = prompt.toLowerCase();
        if (PROMPT_KEYWORDS.get("fix_debug").stream().anyMatch(lower::contains)) return "fix_debug";
        if (PROMPT_KEYWORDS.get("test").stream().anyMatch(lower::contains))      return "test";
        if (PROMPT_KEYWORDS.get("refactor").stream().anyMatch(lower::contains))  return "refactor";
        if (PROMPT_KEYWORDS.get("explain").stream().anyMatch(lower::contains))   return "explain";
        if (PROMPT_KEYWORDS.get("generate").stream().anyMatch(lower::contains))  return "generate";
        return "other";
    }

    private Map<String, Long> computePromptPatterns(List<EditRecordEntity> records) {
        Map<String, Long> patterns = new java.util.LinkedHashMap<>();
        patterns.put("generate", 0L);
        patterns.put("fix_debug", 0L);
        patterns.put("refactor", 0L);
        patterns.put("explain", 0L);
        patterns.put("test", 0L);
        patterns.put("other", 0L);

        for (EditRecordEntity r : records) {
            String ps = r.getPromptSummary();
            if (ps == null || ps.isBlank()) continue;
            patterns.merge(classifyPrompt(ps), 1L, Long::sum);
        }
        return patterns;
    }

    // -------------------------------------------------------------------------
    // Timestamp parsing
    // -------------------------------------------------------------------------

    /**
     * Parses the entity's timestamp field (expected to be epoch-seconds as a Long string).
     * Falls back to receivedAt if parsing fails.
     */
    private Instant parseTimestamp(EditRecordEntity e) {
        try {
            long epochSec = Long.parseLong(e.getTimestamp());
            return Instant.ofEpochSecond(epochSec);
        } catch (NumberFormatException | NullPointerException ex) {
            log.debug("Failed to parse timestamp '{}' for record id={}, falling back to receivedAt",
                    e.getTimestamp(), e.getId());
            return e.getReceivedAt();
        }
    }
}
