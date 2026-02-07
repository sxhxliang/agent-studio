use std::error::Error;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use git_worktree_manager::{
    CreateOptions, MergeOptions, MergeOutcome, WorktreeBranch, WorktreeManager,
};
use git2::{Repository, Signature, build::CheckoutBuilder};

fn main() -> Result<(), Box<dyn Error>> {
    let base = std::env::temp_dir();
    let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let repo_path = base.join(format!("worktree-demo-{}", suffix));
    std::fs::create_dir_all(&repo_path)?;

    let repo = Repository::init(&repo_path)?;
    commit_file(&repo, "README.md", "init\n")?;
    let main_branch = current_branch(&repo);

    let manager = WorktreeManager::new(&repo_path);
    let worktree_path = repo_path.join("agent-a");

    let worktree = manager.create(
        &worktree_path,
        WorktreeBranch::New {
            name: "agent-a".to_string(),
            start_point: None,
        },
        CreateOptions::default(),
    )?;
    println!("created worktree: {}", worktree.path.display());

    manager.switch(&worktree_path, "agent-a")?;

    println!("worktrees:");
    for entry in manager.list()? {
        let branch = entry.branch.as_deref().unwrap_or("detached");
        println!("  {} {}", entry.path.display(), branch);
    }

    commit_file(&repo, "conflict.txt", "main change\n")?;

    let agent_repo = Repository::open(&worktree_path)?;
    commit_file(&agent_repo, "conflict.txt", "agent change\n")?;

    repo.set_head(&format!("refs/heads/{}", main_branch))?;
    repo.checkout_head(Some(CheckoutBuilder::new().force()))?;

    match manager.merge(&repo_path, &main_branch, "agent-a", MergeOptions::default())? {
        MergeOutcome::Merged => println!("merge completed"),
        MergeOutcome::Conflicts(conflicts) => {
            println!("merge conflicts:");
            for conflict in conflicts {
                println!("- {}", conflict.path.display());
                println!("{}", conflict.content);
            }
        }
    }

    manager.delete(&worktree_path, true)?;
    std::fs::remove_dir_all(&repo_path)?;

    Ok(())
}

fn commit_file(repo: &Repository, path: &str, content: &str) -> Result<(), Box<dyn Error>> {
    let workdir = repo.workdir().ok_or("missing workdir")?;
    let file_path = workdir.join(path);
    std::fs::write(&file_path, content)?;

    let mut index = repo.index()?;
    index.add_path(Path::new(path))?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let sig = Signature::now("example", "example@local")?;
    let parent = repo
        .head()
        .ok()
        .and_then(|head| head.target())
        .and_then(|oid| repo.find_commit(oid).ok());
    let parents: Vec<&git2::Commit<'_>> = parent.as_ref().map(|p| vec![p]).unwrap_or_default();

    repo.commit(Some("HEAD"), &sig, &sig, "commit", &tree, &parents)?;
    Ok(())
}

fn current_branch(repo: &Repository) -> String {
    repo.head()
        .ok()
        .and_then(|head| head.shorthand().map(|name| name.to_string()))
        .unwrap_or_else(|| "main".to_string())
}
