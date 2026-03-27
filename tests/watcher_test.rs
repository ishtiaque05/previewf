use previewf::watcher::FileWatcher;
use std::fs;
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_watcher_detects_file_change() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.md");
    fs::write(&file_path, "# Hello").unwrap();

    let (mut watcher, mut rx) = FileWatcher::new(dir.path().to_path_buf()).unwrap();
    watcher.watch().unwrap();

    // Modify the file
    tokio::time::sleep(Duration::from_millis(100)).await;
    fs::write(&file_path, "# Hello Updated").unwrap();

    // Should receive a notification within 2 seconds
    let result = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
    assert!(result.is_ok(), "Should receive file change notification");
}
