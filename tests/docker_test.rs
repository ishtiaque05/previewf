use previewf::docker::{parse_container_list, validate_container_name};

#[test]
fn test_parse_container_list_single() {
    let output = "a1b2c3d4e5f6\tmy-app\tnode:20\tUp 2 hours\n";
    let containers = parse_container_list(output);
    assert_eq!(containers.len(), 1);
    assert_eq!(containers[0].name, "my-app");
    assert_eq!(containers[0].image, "node:20");
}

#[test]
fn test_parse_container_list_multiple() {
    let output = "a1b2c3\tapp\tnode:20\tUp 2h\nf6e5d4\tdb\tpostgres:16\tUp 1h\n";
    let containers = parse_container_list(output);
    assert_eq!(containers.len(), 2);
    assert_eq!(containers[0].name, "app");
    assert_eq!(containers[1].name, "db");
}

#[test]
fn test_parse_container_list_empty() {
    let containers = parse_container_list("");
    assert!(containers.is_empty());
}

#[test]
fn test_validate_container_name_valid() {
    assert!(validate_container_name("my-app").is_ok());
    assert!(validate_container_name("app_v2.1").is_ok());
    assert!(validate_container_name("a1b2c3d4").is_ok());
}

#[test]
fn test_validate_container_name_invalid() {
    assert!(validate_container_name("").is_err());
    assert!(validate_container_name("my;app").is_err());
    assert!(validate_container_name("$(whoami)").is_err());
    assert!(validate_container_name("app name").is_err());
}
