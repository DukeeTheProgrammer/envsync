use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

struct TestRepo {
    _temp: TempDir,
    main: PathBuf,
    linked: PathBuf,
}

impl TestRepo {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let main = temp.path().join("main");
        let linked_path = temp.path().join("linked");

        // Create main repo
        fs::create_dir_all(&main).unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(&main)
            .assert()
            .success();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&main)
            .assert()
            .success();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&main)
            .assert()
            .success();

        // Add .gitignore
        fs::write(main.join(".gitignore"), ".env\n.env.local\n").unwrap();
        fs::write(main.join("README.md"), "test").unwrap();

        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&main)
            .assert()
            .success();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&main)
            .assert()
            .success();

        // Create .env files
        fs::write(main.join(".env"), "PORT=3000\nDB_HOST=localhost\n").unwrap();
        fs::write(main.join(".env.local"), "SECRET=abc123\n").unwrap();

        // Create linked worktree
        Command::new("git")
            .args([
                "worktree",
                "add",
                &linked_path.to_string_lossy(),
                "-b",
                "feature",
            ])
            .current_dir(&main)
            .assert()
            .success();

        TestRepo {
            _temp: temp,
            main,
            linked: linked_path,
        }
    }
}

#[test]
fn test_status_shows_worktrees() {
    let repo = TestRepo::new();
    Command::cargo_bin("envsync")
        .unwrap()
        .args(["status"])
        .current_dir(&repo.linked)
        .assert()
        .success()
        .stdout(predicate::str::contains("Found 2 .env file(s)"));
}

#[test]
fn test_sync_copies_env_files() {
    let repo = TestRepo::new();
    assert!(!repo.linked.join(".env").exists());
    assert!(!repo.linked.join(".env.local").exists());

    Command::cargo_bin("envsync")
        .unwrap()
        .args(["sync"])
        .current_dir(&repo.linked)
        .assert()
        .success()
        .stdout(predicate::str::contains("copied"));

    assert!(repo.linked.join(".env").exists());
    assert!(repo.linked.join(".env.local").exists());

    let content = fs::read_to_string(repo.linked.join(".env")).unwrap();
    assert!(content.contains("PORT=3000"));
    assert!(content.contains("DB_HOST=localhost"));
}

#[test]
fn test_diff_shows_differences() {
    let repo = TestRepo::new();

    // Sync first
    Command::cargo_bin("envsync")
        .unwrap()
        .args(["sync"])
        .current_dir(&repo.linked)
        .assert()
        .success();

    // Modify linked .env
    fs::write(repo.linked.join(".env"), "PORT=4000\nDB_HOST=localhost\n").unwrap();

    Command::cargo_bin("envsync")
        .unwrap()
        .args(["diff"])
        .current_dir(&repo.linked)
        .assert()
        .success()
        .stdout(predicate::str::contains("difference"));
}

#[test]
fn test_sync_dry_run_does_not_write() {
    let repo = TestRepo::new();
    assert!(!repo.linked.join(".env").exists());

    Command::cargo_bin("envsync")
        .unwrap()
        .args(["sync", "--dry-run"])
        .current_dir(&repo.linked)
        .assert()
        .success()
        .stdout(predicate::str::contains("dry run"));

    // File should NOT exist after dry run
    assert!(!repo.linked.join(".env").exists());
}

#[test]
fn test_sync_use_source_resolves_conflicts() {
    let repo = TestRepo::new();

    // Sync first
    Command::cargo_bin("envsync")
        .unwrap()
        .args(["sync"])
        .current_dir(&repo.linked)
        .assert()
        .success();

    // Modify linked .env to create a conflict
    fs::write(repo.linked.join(".env"), "PORT=4000\nDB_HOST=localhost\n").unwrap();

    // Also modify main .env
    fs::write(repo.main.join(".env"), "PORT=3000\nDB_HOST=newhost\n").unwrap();

    // Resolve with --use-source
    Command::cargo_bin("envsync")
        .unwrap()
        .args(["sync", "--use-source"])
        .current_dir(&repo.linked)
        .assert()
        .success()
        .stdout(predicate::str::contains("resolved"));

    let content = fs::read_to_string(repo.linked.join(".env")).unwrap();
    assert!(content.contains("DB_HOST=newhost"));
}

#[test]
fn test_init_creates_config() {
    let repo = TestRepo::new();
    assert!(!repo.main.join(".envsync.toml").exists());

    Command::cargo_bin("envsync")
        .unwrap()
        .args(["init"])
        .current_dir(&repo.main)
        .assert()
        .success()
        .stdout(predicate::str::contains("Created .envsync.toml"));

    assert!(repo.main.join(".envsync.toml").exists());
    let content = fs::read_to_string(repo.main.join(".envsync.toml")).unwrap();
    assert!(content.contains("[envsync]"));
}
