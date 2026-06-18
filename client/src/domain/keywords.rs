// domain/keywords.rs — hardcoded keyword categories, tamper-evident
// Keywords are compiled into the binary and cannot be changed by users.

use sha2::{Digest, Sha256};

/// Prompt pattern categories for classifying prompt text.
#[derive(Debug, Clone, PartialEq)]
pub enum PromptCategory {
    Generate,
    FixDebug,
    Refactor,
    Explain,
    Test,
    Other,
}

impl PromptCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            PromptCategory::Generate => "generate",
            PromptCategory::FixDebug => "fix_debug",
            PromptCategory::Refactor => "refactor",
            PromptCategory::Explain => "explain",
            PromptCategory::Test => "test",
            PromptCategory::Other => "other",
        }
    }
}

// Keywords are hardcoded — cannot be modified by users at runtime.
// SHA256 fingerprint is computed at build time for tamper detection.

const KEYWORDS_GENERATE: &[&str] = &[
    "generate",
    "create",
    "write",
    "implement",
    "add",
    "build",
    "make",
    "生成",
    "创建",
    "新增",
    "写",
    "实现",
    "构建",
];

const KEYWORDS_FIX_DEBUG: &[&str] = &[
    "fix", "bug", "error", "debug", "issue", "wrong", "broken", "fail", "修复", "错误", "调试",
    "修", "问题", "故障", "报错",
];

const KEYWORDS_REFACTOR: &[&str] = &[
    "refactor",
    "clean",
    "improve",
    "optimize",
    "restructure",
    "simplify",
    "重构",
    "优化",
    "改进",
    "简化",
    "整理",
    "重写",
];

const KEYWORDS_EXPLAIN: &[&str] = &[
    "explain",
    "what",
    "how",
    "why",
    "describe",
    "understand",
    "clarify",
    "解释",
    "什么",
    "怎么",
    "为什么",
    "说明",
    "理解",
];

const KEYWORDS_TEST: &[&str] = &[
    "test",
    "spec",
    "mock",
    "coverage",
    "unit",
    "integration",
    "assert",
    "测试",
    "单元测试",
    "集成测试",
    "断言",
    "覆盖",
];

/// Classify a prompt into an intent category.
/// Keywords are checked in priority order: FixDebug > Test > Refactor > Explain > Generate.
/// Returns Other if no keywords match.
pub fn classify_prompt(prompt: &str) -> PromptCategory {
    let lower = prompt.to_lowercase();
    if KEYWORDS_FIX_DEBUG.iter().any(|k| lower.contains(k)) {
        return PromptCategory::FixDebug;
    }
    if KEYWORDS_TEST.iter().any(|k| lower.contains(k)) {
        return PromptCategory::Test;
    }
    if KEYWORDS_REFACTOR.iter().any(|k| lower.contains(k)) {
        return PromptCategory::Refactor;
    }
    if KEYWORDS_EXPLAIN.iter().any(|k| lower.contains(k)) {
        return PromptCategory::Explain;
    }
    if KEYWORDS_GENERATE.iter().any(|k| lower.contains(k)) {
        return PromptCategory::Generate;
    }
    PromptCategory::Other
}

/// Compute the SHA256 fingerprint of all hardcoded keywords.
/// Used to detect tampering with the keyword store.
pub fn keyword_fingerprint() -> String {
    let mut hasher = Sha256::new();
    for kw in KEYWORDS_GENERATE
        .iter()
        .chain(KEYWORDS_FIX_DEBUG.iter())
        .chain(KEYWORDS_REFACTOR.iter())
        .chain(KEYWORDS_EXPLAIN.iter())
        .chain(KEYWORDS_TEST.iter())
    {
        hasher.update(kw.as_bytes());
        hasher.update(b"|");
    }
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_generate() {
        assert_eq!(
            classify_prompt("帮我生成一个 REST API"),
            PromptCategory::Generate
        );
        assert_eq!(
            classify_prompt("create a new function"),
            PromptCategory::Generate
        );
    }

    #[test]
    fn test_classify_fix_debug() {
        assert_eq!(classify_prompt("修复这个错误"), PromptCategory::FixDebug);
        assert_eq!(
            classify_prompt("fix the bug in auth"),
            PromptCategory::FixDebug
        );
    }

    #[test]
    fn test_classify_test() {
        assert_eq!(classify_prompt("写单元测试"), PromptCategory::Test);
        assert_eq!(
            classify_prompt("add unit test for this"),
            PromptCategory::Test
        );
    }

    #[test]
    fn test_classify_other() {
        assert_eq!(classify_prompt(""), PromptCategory::Other);
    }

    #[test]
    fn test_fingerprint_stable() {
        let fp1 = keyword_fingerprint();
        let fp2 = keyword_fingerprint();
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.len(), 64); // SHA256 hex
    }

    #[test]
    fn test_classify_refactor() {
        assert_eq!(
            classify_prompt("refactor this module"),
            PromptCategory::Refactor
        );
        assert_eq!(classify_prompt("重构这段代码"), PromptCategory::Refactor);
    }

    #[test]
    fn test_classify_explain() {
        assert_eq!(
            classify_prompt("explain how this works"),
            PromptCategory::Explain
        );
        assert_eq!(classify_prompt("解释一下这个函数"), PromptCategory::Explain);
    }

    #[test]
    fn test_category_as_str() {
        assert_eq!(PromptCategory::Generate.as_str(), "generate");
        assert_eq!(PromptCategory::FixDebug.as_str(), "fix_debug");
        assert_eq!(PromptCategory::Refactor.as_str(), "refactor");
        assert_eq!(PromptCategory::Explain.as_str(), "explain");
        assert_eq!(PromptCategory::Test.as_str(), "test");
        assert_eq!(PromptCategory::Other.as_str(), "other");
    }

    #[test]
    fn test_fix_debug_priority_over_generate() {
        // "fix" wins over "create" — FixDebug has higher priority
        assert_eq!(
            classify_prompt("fix the create function"),
            PromptCategory::FixDebug
        );
    }
}
