package com.aitrack.server.testkit;

import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import com.fasterxml.jackson.databind.node.ObjectNode;

/**
 * Factory for raw hook payload JSON bytes (simulating PostToolUse events from
 * claude/codex/cursor), used in integration-level tests that need the raw wire format.
 */
public final class HookPayloadFactory {

    private static final JsonNodeFactory NF = JsonNodeFactory.instance;

    private HookPayloadFactory() {}

    /** Claude PostToolUse payload (apply_patch/Edit/Write). */
    public static ObjectNode claude(String filePath, String diffHunk) {
        ObjectNode node = NF.objectNode();
        node.put("tool_name", "Edit");
        ObjectNode input = NF.objectNode();
        input.put("file_path", filePath);
        input.put("old_string", "old content");
        input.put("new_string", "new content");
        node.set("tool_input", input);
        ObjectNode output = NF.objectNode();
        output.put("diff", diffHunk);
        node.set("tool_response", output);
        node.put("session_id", "claude-sess-001");
        return node;
    }

    /** Codex PostToolUse payload. */
    public static ObjectNode codex(String filePath) {
        ObjectNode node = NF.objectNode();
        node.put("type", "apply_patch");
        ObjectNode patch = NF.objectNode();
        patch.put("path", filePath);
        patch.put("content", "updated content");
        node.set("patch", patch);
        return node;
    }

    /** Cursor afterFileEdit payload. */
    public static ObjectNode cursor(String filePath) {
        ObjectNode node = NF.objectNode();
        node.put("file", filePath);
        node.put("workspace", "/Users/user/workspace");
        return node;
    }
}
