use previewf::source::local::LocalSource;
use previewf::source::{EntryType, FileSource};

#[tokio::test]
async fn test_local_read_file() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let content = source.read_file("sample.md").await.unwrap();
    assert!(content.contains("Sample Document"));
}

#[tokio::test]
async fn test_local_read_file_not_found() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let result = source.read_file("nonexistent.md").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_local_list_dir_root() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let entries = source.list_dir("").await.unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"sample.md"));
    assert!(names.contains(&"sample.html"));
}

#[tokio::test]
async fn test_local_list_dir_has_correct_types() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let entries = source.list_dir("").await.unwrap();
    for entry in &entries {
        if entry.name.ends_with(".md") || entry.name.ends_with(".html") {
            assert_eq!(entry.entry_type, EntryType::File);
        }
    }
}

#[tokio::test]
async fn test_local_stat_file() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let meta = source.stat("sample.md").await.unwrap();
    assert!(!meta.is_dir);
    assert!(meta.size > 0);
}

#[tokio::test]
async fn test_local_is_file() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    assert!(source.is_file("sample.md").await);
    assert!(!source.is_file("nonexistent.md").await);
}

#[tokio::test]
async fn test_local_is_dir() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    assert!(source.is_dir("").await);
    assert!(!source.is_dir("sample.md").await);
}

#[tokio::test]
async fn test_local_write_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let source = LocalSource::new(dir.path()).unwrap();
    source.write_file("test.md", "# Hello\n").await.unwrap();
    let content = source.read_file("test.md").await.unwrap();
    assert_eq!(content, "# Hello\n");
}

#[tokio::test]
async fn test_local_path_traversal_rejected() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let result = source.read_file("../../Cargo.toml").await;
    assert!(result.is_err(), "path traversal must be rejected");
}

#[tokio::test]
async fn test_local_display_root() {
    let source = LocalSource::new("tests/fixtures").unwrap();
    let root = source.display_root();
    assert!(
        root.contains("fixtures"),
        "display root should contain the path"
    );
}
