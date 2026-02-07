use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use git2::{
    BranchType, Index, MergeOptions as GitMergeOptions, Oid, Repository, StatusOptions,
    WorktreeAddOptions, WorktreeLockStatus, WorktreePruneOptions, build::CheckoutBuilder,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub name: Option<String>,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub is_bare: bool,
    pub is_locked: bool,
    pub lock_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateOptions {
    pub force: bool,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self { force: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorktreeBranch {
    Existing(String),
    New {
        name: String,
        start_point: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOptions {
    pub no_ff: bool,
    pub message: Option<String>,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self {
            no_ff: true,
            message: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    Merged,
    Conflicts(Vec<MergeConflict>),
}

#[derive(Debug, Clone)]
pub struct WorktreeManager {
    repo_path: PathBuf,
}

impl WorktreeManager {
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
        }
    }

    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    pub fn list(&self) -> Result<Vec<WorktreeInfo>> {
        let repo = self.open_repo(&self.repo_path)?;
        let mut worktrees = Vec::new();

        if let Some(workdir) = repo.workdir() {
            let workdir = workdir
                .canonicalize()
                .unwrap_or_else(|_| workdir.to_path_buf());
            let (head, branch) = repo_head_branch(&repo)?;
            worktrees.push(WorktreeInfo {
                path: workdir,
                name: None,
                head,
                branch,
                is_bare: repo.is_bare(),
                is_locked: false,
                lock_reason: None,
            });
        }

        let names = repo.worktrees().context("failed to list worktrees")?;
        for name in names.iter().flatten() {
            let name_str = name.to_string();
            let worktree = repo
                .find_worktree(name)
                .with_context(|| format!("failed to open worktree {}", name_str))?;
            let worktree_path = worktree
                .path()
                .canonicalize()
                .unwrap_or_else(|_| worktree.path().to_path_buf());
            let wt_repo = self.open_repo(&worktree_path)?;
            let (head, branch) = repo_head_branch(&wt_repo)?;
            let (is_locked, lock_reason) = match worktree.is_locked()? {
                WorktreeLockStatus::Unlocked => (false, None),
                WorktreeLockStatus::Locked(reason) => (true, reason),
            };
            worktrees.push(WorktreeInfo {
                path: worktree_path,
                name: Some(name_str),
                head,
                branch,
                is_bare: wt_repo.is_bare(),
                is_locked,
                lock_reason,
            });
        }

        Ok(worktrees)
    }

    pub fn create(
        &self,
        path: impl AsRef<Path>,
        branch: WorktreeBranch,
        options: CreateOptions,
    ) -> Result<WorktreeInfo> {
        let path = path.as_ref();
        let repo = self.open_repo(&self.repo_path)?;
        if path.exists() {
            if options.force {
                fs::remove_dir_all(path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            } else {
                bail!("worktree path already exists: {}", path.display());
            }
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("worktree")
            .to_string();

        let mut add_opts = WorktreeAddOptions::new();

        let branch_name = match branch {
            WorktreeBranch::Existing(name) => name,
            WorktreeBranch::New { name, start_point } => {
                let commit = resolve_commit(&repo, start_point.as_deref())?;
                repo.branch(&name, &commit, false)
                    .with_context(|| format!("failed to create branch {}", name))?;
                name
            }
        };

        let reference = repo
            .find_reference(&format!("refs/heads/{}", branch_name))
            .with_context(|| format!("failed to find branch {}", branch_name))?;
        add_opts.reference(Some(&reference));

        let worktree = repo
            .worktree(&name, path, Some(&add_opts))
            .with_context(|| format!("failed to add worktree {}", path.display()))?;

        let worktree_repo = self.open_repo(path)?;
        worktree_repo
            .set_head(&format!("refs/heads/{}", branch_name))
            .with_context(|| format!("failed to set head to {}", branch_name))?;
        worktree_repo
            .checkout_head(Some(CheckoutBuilder::new().safe()))
            .with_context(|| "failed to checkout new worktree")?;

        let worktree_repo = self.open_repo(path)?;
        let (head, branch) = repo_head_branch(&worktree_repo)?;
        let (is_locked, lock_reason) = match worktree.is_locked()? {
            WorktreeLockStatus::Unlocked => (false, None),
            WorktreeLockStatus::Locked(reason) => (true, reason),
        };
        Ok(WorktreeInfo {
            path: worktree
                .path()
                .canonicalize()
                .unwrap_or_else(|_| worktree.path().to_path_buf()),
            name: worktree.name().map(|value| value.to_string()),
            head,
            branch,
            is_bare: worktree_repo.is_bare(),
            is_locked,
            lock_reason,
        })
    }

    pub fn delete(&self, path: impl AsRef<Path>, force: bool) -> Result<()> {
        let path = path.as_ref();
        let resolved_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let repo = self.open_repo(&self.repo_path)?;
        if let Some(root) = repo.workdir() {
            let resolved_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
            if resolved_root == resolved_path {
                bail!(
                    "refusing to remove main worktree at {}",
                    resolved_path.display()
                );
            }
        }

        let worktree_name = self
            .list()?
            .into_iter()
            .find(|info| info.path == resolved_path)
            .and_then(|info| info.name)
            .with_context(|| format!("worktree not found: {}", resolved_path.display()))?;

        let worktree = repo
            .find_worktree(&worktree_name)
            .with_context(|| format!("failed to find worktree {}", worktree_name))?;

        let mut prune_opts = WorktreePruneOptions::new();
        if force {
            prune_opts.valid(true).locked(true).working_tree(true);
        }
        worktree
            .prune(Some(&mut prune_opts))
            .with_context(|| format!("failed to prune worktree {}", worktree_name))?;

        if resolved_path.exists() {
            fs::remove_dir_all(&resolved_path)
                .with_context(|| format!("failed to remove {}", resolved_path.display()))?;
        }
        Ok(())
    }

    pub fn switch(&self, worktree_path: impl AsRef<Path>, branch: &str) -> Result<()> {
        let worktree_path = worktree_path.as_ref();
        let repo = self.open_repo(worktree_path)?;
        repo.set_head(&format!("refs/heads/{}", branch))
            .with_context(|| format!("failed to set head to {}", branch))?;
        repo.checkout_head(Some(CheckoutBuilder::new().safe()))
            .with_context(|| format!("failed to checkout {}", branch))?;
        Ok(())
    }

    pub fn merge(
        &self,
        target_worktree: impl AsRef<Path>,
        target_branch: &str,
        source_branch: &str,
        options: MergeOptions,
    ) -> Result<MergeOutcome> {
        let target_worktree = target_worktree.as_ref();
        let repo = self.open_repo(target_worktree)?;
        ensure_clean_repo(&repo)?;

        let current_branch = repo_head_branch(&repo)?.1;
        if current_branch.as_deref() != Some(target_branch) {
            self.switch(target_worktree, target_branch)
                .with_context(|| format!("failed to switch to {}", target_branch))?;
        }

        let source_commit = find_branch_commit(&repo, source_branch)?;
        let source_ref = repo
            .find_reference(&format!("refs/heads/{}", source_branch))
            .with_context(|| format!("branch not found: {}", source_branch))?;
        let annotated = repo.reference_to_annotated_commit(&source_ref)?;
        let (analysis, _) = repo.merge_analysis(&[&annotated])?;
        if analysis.is_up_to_date() {
            return Ok(MergeOutcome::Merged);
        }

        if analysis.is_fast_forward() && !options.no_ff {
            fast_forward(&repo, target_branch, source_commit.id())?;
            return Ok(MergeOutcome::Merged);
        }

        let mut merge_opts = GitMergeOptions::new();
        let mut checkout = CheckoutBuilder::new();
        checkout.safe();
        repo.merge(&[&annotated], Some(&mut merge_opts), Some(&mut checkout))?;

        let mut index = repo.index()?;
        if index.has_conflicts() {
            let conflicts = collect_conflicts_from_index(&mut index, target_worktree)?;
            return Ok(MergeOutcome::Conflicts(conflicts));
        }

        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;
        let head_commit = repo.head()?.peel_to_commit()?;
        let message = options.message.unwrap_or_else(|| {
            format!("Merge branch '{}' into '{}'", source_branch, target_branch)
        });
        let signature = repo.signature()?;
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &message,
            &tree,
            &[&head_commit, &source_commit],
        )?;
        repo.checkout_head(Some(CheckoutBuilder::new().safe()))?;

        Ok(MergeOutcome::Merged)
    }

    fn find_by_path(&self, path: &Path) -> Result<WorktreeInfo> {
        let path = path
            .canonicalize()
            .with_context(|| format!("failed to resolve path {}", path.display()))?;
        let worktrees = self.list()?;
        worktrees
            .into_iter()
            .find(|info| info.path == path)
            .with_context(|| format!("worktree not found: {}", path.display()))
    }

    fn open_repo(&self, path: &Path) -> Result<Repository> {
        Repository::open(path)
            .with_context(|| format!("failed to open repository at {}", path.display()))
    }
}

fn read_conflict_content(path: &Path) -> Result<String> {
    match fs::read(path) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).to_string()),
        Err(error) => Ok(format!("<<unable to read conflict file: {}>>", error)),
    }
}

fn repo_head_branch(repo: &Repository) -> Result<(Option<String>, Option<String>)> {
    let head = repo.head().ok();
    let head_oid = head
        .as_ref()
        .and_then(|reference| reference.target())
        .map(|oid| oid.to_string());
    let branch = head
        .as_ref()
        .and_then(|reference| reference.shorthand())
        .map(|name| name.to_string())
        .filter(|name| name != "HEAD");
    Ok((head_oid, branch))
}

fn resolve_commit<'a>(repo: &'a Repository, start_point: Option<&str>) -> Result<git2::Commit<'a>> {
    if let Some(point) = start_point {
        let object = repo.revparse_single(point)?;
        object
            .peel_to_commit()
            .with_context(|| format!("invalid start point {}", point))
    } else {
        repo.head()?
            .peel_to_commit()
            .context("failed to resolve HEAD")
    }
}

fn find_branch_commit<'a>(repo: &'a Repository, branch: &str) -> Result<git2::Commit<'a>> {
    let reference = repo
        .find_branch(branch, BranchType::Local)
        .with_context(|| format!("branch not found: {}", branch))?
        .into_reference();
    let object = reference.peel_to_commit()?;
    Ok(object)
}

fn ensure_clean_repo(repo: &Repository) -> Result<()> {
    let mut options = StatusOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo.statuses(Some(&mut options))?;
    if !statuses.is_empty() {
        bail!("worktree has uncommitted changes");
    }
    Ok(())
}

fn fast_forward(repo: &Repository, branch: &str, target: Oid) -> Result<()> {
    let mut reference = repo
        .find_reference(&format!("refs/heads/{}", branch))
        .with_context(|| format!("missing branch {}", branch))?;
    reference.set_target(target, "fast-forward")?;
    repo.set_head(&format!("refs/heads/{}", branch))?;
    repo.checkout_head(Some(CheckoutBuilder::new().safe()))?;
    Ok(())
}

fn collect_conflicts_from_index(
    index: &mut Index,
    worktree_path: &Path,
) -> Result<Vec<MergeConflict>> {
    let mut conflicts = Vec::new();
    let entries = index.conflicts()?;
    for conflict in entries {
        let conflict = conflict?;
        let entry = conflict
            .our
            .as_ref()
            .or(conflict.their.as_ref())
            .or(conflict.ancestor.as_ref());
        if let Some(entry) = entry {
            let relative = PathBuf::from(String::from_utf8_lossy(entry.path.as_ref()).to_string());
            let content = read_conflict_content(&worktree_path.join(&relative))?;
            conflicts.push(MergeConflict {
                path: relative,
                content,
            });
        }
    }
    Ok(conflicts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn init_repo() -> (TempDir, Repository) {
        let temp = TempDir::new().unwrap();
        let repo = Repository::init(temp.path()).unwrap();
        commit_file(&repo, "README.md", "init\n");
        (temp, repo)
    }

    fn commit_file(repo: &Repository, path: &str, content: &str) -> Oid {
        let workdir = repo.workdir().unwrap();
        let file_path = workdir.join(path);
        fs::write(&file_path, content).unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let sig = Signature::now("tests", "tests@example.com").unwrap();
        let parent = repo
            .head()
            .ok()
            .and_then(|head| head.target())
            .and_then(|oid| repo.find_commit(oid).ok());
        let parents: Vec<&git2::Commit<'_>> = parent.as_ref().map(|p| vec![p]).unwrap_or_default();

        repo.commit(Some("HEAD"), &sig, &sig, "commit", &tree, &parents)
            .unwrap()
    }

    fn current_branch(repo: &Repository) -> String {
        repo_head_branch(repo)
            .unwrap()
            .1
            .unwrap_or_else(|| "main".to_string())
    }

    #[test]
    fn create_list_switch_worktree() {
        let (temp, _repo) = init_repo();
        let manager = WorktreeManager::new(temp.path());
        let worktree_path = temp.path().join("agent-a");

        manager
            .create(
                &worktree_path,
                WorktreeBranch::New {
                    name: "agent-a".to_string(),
                    start_point: None,
                },
                CreateOptions::default(),
            )
            .unwrap();

        let list = manager.list().unwrap();
        let expected = worktree_path.canonicalize().unwrap();
        assert!(list.iter().any(|entry| entry.path == expected));

        manager.switch(&worktree_path, "agent-a").unwrap();
        manager.delete(&worktree_path, true).unwrap();
        let list = manager.list().unwrap();
        assert!(!list.iter().any(|entry| entry.path == expected));
    }

    #[test]
    fn merge_without_conflicts() {
        let (temp, repo) = init_repo();
        let manager = WorktreeManager::new(temp.path());
        let main_branch = current_branch(&repo);

        let base_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("agent", &base_commit, false).unwrap();

        commit_file(&repo, "main.txt", "main change\n");

        repo.set_head("refs/heads/agent").unwrap();
        repo.checkout_head(Some(CheckoutBuilder::new().force()))
            .unwrap();
        commit_file(&repo, "agent.txt", "agent change\n");

        repo.set_head(&format!("refs/heads/{}", main_branch))
            .unwrap();
        repo.checkout_head(Some(CheckoutBuilder::new().force()))
            .unwrap();

        let outcome = manager
            .merge(temp.path(), &main_branch, "agent", MergeOptions::default())
            .unwrap();

        assert!(matches!(outcome, MergeOutcome::Merged));
    }

    #[test]
    fn merge_conflict_returns_content() {
        let (temp, repo) = init_repo();
        let manager = WorktreeManager::new(temp.path());
        let main_branch = current_branch(&repo);

        let base_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("agent", &base_commit, false).unwrap();

        commit_file(&repo, "conflict.txt", "main change\n");

        repo.set_head("refs/heads/agent").unwrap();
        repo.checkout_head(Some(CheckoutBuilder::new().force()))
            .unwrap();
        commit_file(&repo, "conflict.txt", "agent change\n");

        repo.set_head(&format!("refs/heads/{}", main_branch))
            .unwrap();
        repo.checkout_head(Some(CheckoutBuilder::new().force()))
            .unwrap();

        let outcome = manager
            .merge(temp.path(), &main_branch, "agent", MergeOptions::default())
            .unwrap();

        match outcome {
            MergeOutcome::Conflicts(conflicts) => {
                assert!(!conflicts.is_empty());
                assert!(conflicts[0].content.contains("<<<<<<<"));
            }
            MergeOutcome::Merged => panic!("expected merge conflicts"),
        }
    }
}
