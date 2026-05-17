package com.aitrack.server;

import com.aitrack.server.dto.EditDto;
import com.aitrack.server.service.EditValidator;
import com.aitrack.server.testkit.EditDtoFactory;
import com.aitrack.server.testkit.TamperedFactory;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

class EditValidatorTest {

    private EditValidator validator;

    @BeforeEach
    void setUp() {
        validator = new EditValidator();
    }

    @Test
    void validEdit_returnsNull() {
        assertThat(validator.validate(EditDtoFactory.build())).isNull();
    }

    @Test
    void nullEdit_malformed() {
        assertThat(validator.validate(null)).isEqualTo("malformed");
    }

    @Test
    void nullTool_malformed() {
        EditDto edit = TamperedFactory.nullTool();
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void blankTool_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setTool("   ");
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void blankProvider_malformed() {
        EditDto edit = TamperedFactory.blankProvider();
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullProvider_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setProvider(null);
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullSessionId_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setSessionId(null);
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void blankSessionId_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setSessionId("");
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullFilePath_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setFilePath(null);
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void blankFilePath_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setFilePath("  ");
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullTimestamp_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setTimestamp(null);
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullDeviceId_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setDeviceId(null);
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullHostname_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setHostname(null);
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void blankHostname_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setHostname("   ");
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullRepoUrl_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setRepoUrl(null);
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullBranch_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setBranch(null);
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullCurrentSha_malformed() {
        EditDto edit = EditDtoFactory.build();
        edit.setCurrentSha(null);
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void blankRecordSig_malformed() {
        EditDto edit = TamperedFactory.blankRecordSig();
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullAddedLines_malformed() {
        EditDto edit = TamperedFactory.nullAddedLines();
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullRemovedLines_malformed() {
        EditDto edit = TamperedFactory.nullRemovedLines();
        assertThat(validator.validate(edit)).isEqualTo("malformed");
    }

    @Test
    void nullModel_isAllowed() {
        // model is optional per CONTRACT.md
        EditDto edit = EditDtoFactory.build();
        edit.setModel(null);
        assertThat(validator.validate(edit)).isNull();
    }

    @Test
    void nullDiffHunk_isAllowed() {
        // diff_hunk is optional
        EditDto edit = EditDtoFactory.build();
        edit.setDiffHunk(null);
        assertThat(validator.validate(edit)).isNull();
    }
}
