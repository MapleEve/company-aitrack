package model

import (
	"encoding/json"
	"testing"
)

func TestUsageRollupItemDefaultsMissingBasisToNative(t *testing.T) {
	var item UsageRollupItem
	if err := json.Unmarshal([]byte(`{"device_id":"d1","day":"2026-06-18","agent":"codex","model":"gpt-5"}`), &item); err != nil {
		t.Fatalf("unmarshal usage rollup item: %v", err)
	}
	if item.UsageBasis != "native" {
		t.Fatalf("usage_basis = %q, want native", item.UsageBasis)
	}
}

func TestUsageRollupItemRejectsInvalidBasis(t *testing.T) {
	var item UsageRollupItem
	if err := json.Unmarshal([]byte(`{"device_id":"d1","day":"2026-06-18","agent":"codex","model":"gpt-5","usage_basis":"estimated"}`), &item); err == nil {
		t.Fatal("expected invalid usage_basis to fail")
	}
}
