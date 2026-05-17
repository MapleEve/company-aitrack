use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct RepoInfo {
    pub repo_url: String,
    pub branch: String,
    pub current_sha: String,
}

fn run_git(args: &[&str], cwd: &Path) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn infer_repo_info(cwd: &Path) -> RepoInfo {
    if run_git(&["rev-parse", "--git-dir"], cwd).is_none() {
        return RepoInfo::default();
    }

    let repo_url = run_git(&["remote", "get-url", "origin"], cwd).unwrap_or_default();
    let branch = run_git(&["branch", "--show-current"], cwd).unwrap_or_default();
    let current_sha = run_git(&["rev-parse", "HEAD"], cwd).unwrap_or_default();

    RepoInfo {
        repo_url,
        branch,
        current_sha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn infer_repo_info_non_git_dir_returns_default() {
        let dir = TempDir::new().unwrap();
        let info = infer_repo_info(dir.path());
        assert!(info.repo_url.is_empty(), "non-git dir: repo_url should be empty");
        assert!(info.branch.is_empty(), "non-git dir: branch should be empty");
        assert!(info.current_sha.is_empty(), "non-git dir: sha should be empty");
    }

    #[test]
    fn repo_info_default_all_empty() {
        let info = RepoInfo::default();
        assert!(info.repo_url.is_empty());
        assert!(info.branch.is_empty());
        assert!(info.current_sha.is_empty());
    }

    #[test]
    fn run_git_invalid_command_returns_none() {
        let dir = TempDir::new().unwrap();
        let result = run_git(&["not-a-real-subcommand-xyz"], dir.path());
        assert!(result.is_none());
    }

    #[test]
    fn infer_repo_info_in_actual_git_repo() {
        let dir = TempDir::new().unwrap();
        let init = std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output();
        if init.map(|o| o.status.success()).unwrap_or(false) {
            let _ = std::process::Command::new("git")
                .args(["remote", "add", "origin", "git@github.com:test/repo.git"])
                .current_dir(dir.path()).output();
            let _ = std::process::Command::new("git")
                .args(["config", "user.email", "t@t.com"]).current_dir(dir.path()).output();
            let _ = std::process::Command::new("git")
                .args(["config", "user.name", "T"]).current_dir(dir.path()).output();
            std::fs::write(dir.path().join("f"), "x").unwrap();
            let _ = std::process::Command::new("git")
                .args(["add", "."]).current_dir(dir.path()).output();
            let committed = std::process::Command::new("git")
                .args(["commit", "-m", "init"]).current_dir(dir.path()).output()
                .map(|o| o.status.success()).unwrap_or(false);
            if committed {
                let info = infer_repo_info(dir.path());
                assert_eq!(info.repo_url, "git@github.com:test/repo.git");
                assert_eq!(info.current_sha.len(), 40);
            }
        }
    }
}
