package com.aitrack.server.infrastructure.config;

import jakarta.annotation.PostConstruct;
import org.springframework.boot.autoconfigure.orm.jpa.EntityManagerFactoryDependsOnPostProcessor;
import org.springframework.context.annotation.Configuration;
import org.springframework.stereotype.Component;

import javax.sql.DataSource;
import java.sql.Connection;
import java.sql.DatabaseMetaData;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.sql.Statement;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TreeMap;

@Component("usageDailyRollupSchemaMigrator")
public class UsageDailyRollupSchemaMigrator {

    private static final String TABLE_NAME = "usage_daily_rollups";
    private static final String NEW_UNIQUE_NAME = "uk_usage_daily_rollups_basis";
    private static final List<String> LEGACY_UNIQUE_NAMES = List.of(
        "usage_daily_rollups_token_key_device_id_day_agent_model_account_key",
        "usage_daily_rollups_token_key_device_id_day_agent_model_acc_key",
        "uk_usage_daily_rollup"
    );
    private static final List<String> UNIQUE_COLUMNS = List.of(
        "token_key",
        "device_id",
        "day",
        "agent",
        "model",
        "account",
        "usage_basis"
    );

    private final DataSource dataSource;

    public UsageDailyRollupSchemaMigrator(DataSource dataSource) {
        this.dataSource = dataSource;
    }

    @PostConstruct
    public void migrate() throws SQLException {
        try (Connection connection = dataSource.getConnection()) {
            if (!tableExists(connection)) {
                return;
            }
            boolean postgres = isPostgres(connection);
            executeRequired(connection, addUsageBasisSql(postgres));
            executeRequired(connection, "UPDATE " + TABLE_NAME + " SET usage_basis = 'native' WHERE usage_basis IS NULL OR usage_basis = ''");
            dropLegacyUniqueConstraints(connection);
            if (!hasExpectedUnique(connection)) {
                executeRequired(connection, createSevenColumnUniqueSql(postgres));
            }
        }
    }

    static List<String> postgresMigrationSql() {
        List<String> statements = new ArrayList<>();
        statements.add(addUsageBasisSql(true));
        statements.add("UPDATE " + TABLE_NAME + " SET usage_basis = 'native' WHERE usage_basis IS NULL OR usage_basis = ''");
        for (String name : LEGACY_UNIQUE_NAMES) {
            statements.add("ALTER TABLE " + TABLE_NAME + " DROP CONSTRAINT IF EXISTS " + name);
            statements.add("DROP INDEX IF EXISTS " + name);
        }
        statements.add(createSevenColumnUniqueSql(true));
        return statements;
    }

    private void dropLegacyUniqueConstraints(Connection connection) throws SQLException {
        for (String name : LEGACY_UNIQUE_NAMES) {
            if ("uk_usage_daily_rollup".equals(name) && uniqueIndexHasColumns(connection, name, UNIQUE_COLUMNS)) {
                continue;
            }
            executeRequired(connection, "ALTER TABLE " + TABLE_NAME + " DROP CONSTRAINT IF EXISTS " + name);
            executeOptional(connection, "DROP INDEX IF EXISTS " + name);
        }
    }

    private boolean hasExpectedUnique(Connection connection) throws SQLException {
        return uniqueIndexes(connection).values().stream().anyMatch(UNIQUE_COLUMNS::equals);
    }

    private boolean uniqueIndexHasColumns(Connection connection, String indexName, List<String> columns) throws SQLException {
        List<String> actual = uniqueIndexes(connection).get(normalizeIdentifier(indexName));
        return columns.equals(actual);
    }

    private static Map<String, List<String>> uniqueIndexes(Connection connection) throws SQLException {
        Map<String, TreeMap<Short, String>> indexedColumns = new LinkedHashMap<>();
        DatabaseMetaData metaData = connection.getMetaData();
        for (String tableName : List.of(TABLE_NAME, TABLE_NAME.toUpperCase(Locale.ROOT))) {
            try (ResultSet indexes = metaData.getIndexInfo(null, null, tableName, true, false)) {
                while (indexes.next()) {
                    String indexName = indexes.getString("INDEX_NAME");
                    String columnName = indexes.getString("COLUMN_NAME");
                    if (indexName == null || columnName == null) {
                        continue;
                    }
                    indexedColumns
                        .computeIfAbsent(normalizeIdentifier(indexName), ignored -> new TreeMap<>())
                        .put(indexes.getShort("ORDINAL_POSITION"), normalizeIdentifier(columnName));
                }
            }
        }

        Map<String, List<String>> ordered = new LinkedHashMap<>();
        indexedColumns.forEach((indexName, columns) -> ordered.put(indexName, List.copyOf(columns.values())));
        return ordered;
    }

    private static boolean tableExists(Connection connection) throws SQLException {
        DatabaseMetaData metaData = connection.getMetaData();
        for (String tableName : List.of(TABLE_NAME, TABLE_NAME.toUpperCase(Locale.ROOT))) {
            try (ResultSet tables = metaData.getTables(null, null, tableName, new String[]{"TABLE"})) {
                if (tables.next()) {
                    return true;
                }
            }
        }
        return false;
    }

    private static boolean isPostgres(Connection connection) throws SQLException {
        return connection.getMetaData().getDatabaseProductName().toLowerCase(Locale.ROOT).contains("postgres");
    }

    private static String addUsageBasisSql(boolean postgres) {
        String type = postgres ? "VARCHAR(32)" : "VARCHAR(32)";
        return "ALTER TABLE " + TABLE_NAME + " ADD COLUMN IF NOT EXISTS usage_basis " + type + " NOT NULL DEFAULT 'native'";
    }

    private static String createSevenColumnUniqueSql(boolean postgres) {
        String ifNotExists = postgres ? "IF NOT EXISTS " : "IF NOT EXISTS ";
        String dayColumn = postgres ? "day" : "\"day\"";
        return "CREATE UNIQUE INDEX " + ifNotExists + NEW_UNIQUE_NAME + " ON " + TABLE_NAME
            + "(token_key, device_id, " + dayColumn + ", agent, model, account, usage_basis)";
    }

    private static void executeRequired(Connection connection, String sql) throws SQLException {
        try (Statement statement = connection.createStatement()) {
            statement.execute(sql);
        }
    }

    private static void executeOptional(Connection connection, String sql) {
        try (Statement statement = connection.createStatement()) {
            statement.execute(sql);
        } catch (SQLException ignored) {
            // Optional legacy cleanup must not block databases that never had that name or dialect form.
        }
    }

    private static String normalizeIdentifier(String value) {
        return value.replace("\"", "").toLowerCase(Locale.ROOT);
    }
}

@Configuration(proxyBeanMethods = false)
class UsageDailyRollupSchemaMigrationOrder extends EntityManagerFactoryDependsOnPostProcessor {
    UsageDailyRollupSchemaMigrationOrder() {
        super("usageDailyRollupSchemaMigrator");
    }
}
