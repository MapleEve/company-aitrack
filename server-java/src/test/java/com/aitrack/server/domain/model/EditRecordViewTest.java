package com.aitrack.server.domain.model;

import org.junit.jupiter.api.Test;

import java.time.Instant;

import static org.assertj.core.api.Assertions.assertThat;

class EditRecordViewTest {

    @Test
    void fromEntityIncludesPromptSummary() {
        EditRecordEntity entity = new EditRecordEntity();
        entity.setId(7L);
        entity.setTokenKey("token");
        entity.setDeviceId("device");
        entity.setHostname("host");
        entity.setTool("codex");
        entity.setProvider("openai");
        entity.setSessionId("session");
        entity.setRepoUrl("local");
        entity.setBranch("main");
        entity.setCurrentSha("abc");
        entity.setFilePath("src/main.rs");
        entity.setTimestamp("2026-06-16T10:00:00Z");
        entity.setRecordSig("sig");
        entity.setStatus(EditRecordEntity.RecordStatus.ACCEPTED);
        entity.setReceivedAt(Instant.parse("2026-06-16T10:00:00Z"));
        entity.setPromptSummary("prompt text");

        EditRecordView view = EditRecordView.from(entity);

        assertThat(view.getPromptSummary()).isEqualTo("prompt text");
    }
}
