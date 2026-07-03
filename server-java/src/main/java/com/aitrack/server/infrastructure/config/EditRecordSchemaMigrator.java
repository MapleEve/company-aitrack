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
import java.util.List;
import java.util.Locale;

@Component("editRecordSchemaMigrator")
public class EditRecordSchemaMigrator {

    private static final String TABLE_NAME = "edit_records";
    private final DataSource dataSource;

    public EditRecordSchemaMigrator(DataSource dataSource) {
        this.dataSource = dataSource;
    }

    @PostConstruct
    public void migrate() throws SQLException {
        try (Connection connection = dataSource.getConnection()) {
            if (!tableExists(connection)) {
                return;
            }
            executeRequired(connection, addPromptSummarySql());
        }
    }

    static List<String> postgresMigrationSql() {
        return List.of(addPromptSummarySql());
    }

    private static String addPromptSummarySql() {
        return "ALTER TABLE " + TABLE_NAME + " ADD COLUMN IF NOT EXISTS prompt_summary TEXT";
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

    private static void executeRequired(Connection connection, String sql) throws SQLException {
        try (Statement statement = connection.createStatement()) {
            statement.execute(sql);
        }
    }
}

@Configuration(proxyBeanMethods = false)
class EditRecordSchemaMigrationOrder extends EntityManagerFactoryDependsOnPostProcessor {
    EditRecordSchemaMigrationOrder() {
        super("editRecordSchemaMigrator");
    }
}
