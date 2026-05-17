use jwt_exchange::services::token_service;

// ── filter_groups ───────────────────────────────────────────

#[test]
fn filter_groups_all_match() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = ["admins", "analysts", "developers"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let groups = vec!["admins".to_string(), "analysts".to_string()];
    let result = token_service::filter_groups(Some(&groups), &whitelist, 50, 256);
    assert_eq!(
        result.unwrap(),
        Some(vec!["admins".to_string(), "analysts".to_string()])
    );
}

#[test]
fn filter_groups_some_filtered() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = ["admins", "analysts"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let groups = vec![
        "admins".to_string(),
        "unknown-group".to_string(),
        "analysts".to_string(),
    ];
    let result = token_service::filter_groups(Some(&groups), &whitelist, 50, 256);
    assert_eq!(
        result.unwrap(),
        Some(vec!["admins".to_string(), "analysts".to_string()])
    );
}

#[test]
fn filter_groups_none_match_returns_none() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
    let groups = vec!["random".to_string(), "other".to_string()];
    let result = token_service::filter_groups(Some(&groups), &whitelist, 50, 256);
    assert!(result.unwrap().is_none());
}

#[test]
fn filter_groups_none_input_returns_none() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
    let result = token_service::filter_groups(None, &whitelist, 50, 256);
    assert!(result.unwrap().is_none());
}

#[test]
fn filter_groups_empty_whitelist_returns_none() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = HashSet::new();
    let groups = vec!["admins".to_string()];
    let result = token_service::filter_groups(Some(&groups), &whitelist, 50, 256);
    assert!(result.unwrap().is_none());
}

#[test]
fn filter_groups_empty_input_returns_none() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
    let groups: Vec<String> = vec![];
    let result = token_service::filter_groups(Some(&groups), &whitelist, 50, 256);
    assert!(result.unwrap().is_none());
}

#[test]
fn filter_groups_preserves_order() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = ["z-group", "a-group", "m-group"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let groups = vec![
        "m-group".to_string(),
        "z-group".to_string(),
        "x-group".to_string(),
        "a-group".to_string(),
    ];
    let result = token_service::filter_groups(Some(&groups), &whitelist, 50, 256);
    assert_eq!(
        result.unwrap(),
        Some(vec![
            "m-group".to_string(),
            "z-group".to_string(),
            "a-group".to_string()
        ])
    );
}

#[test]
fn filter_groups_preserves_duplicates() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
    let groups = vec!["admins".to_string(), "admins".to_string()];
    let result = token_service::filter_groups(Some(&groups), &whitelist, 50, 256);
    assert_eq!(
        result.unwrap(),
        Some(vec!["admins".to_string(), "admins".to_string()])
    );
}

#[test]
fn filter_groups_exceeds_max_count_returns_error() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
    let groups: Vec<String> = (0..51).map(|i| format!("group-{i}")).collect();
    let result = token_service::filter_groups(Some(&groups), &whitelist, 50, 256);
    assert!(result.is_err());
}

#[test]
fn filter_groups_exceeds_max_name_length_returns_error() {
    use std::collections::HashSet;

    let whitelist: HashSet<String> = ["admins"].iter().map(|s| s.to_string()).collect();
    let groups = vec!["a".repeat(257)];
    let result = token_service::filter_groups(Some(&groups), &whitelist, 50, 256);
    assert!(result.is_err());
}

// ── base64url_decode ────────────────────────────────────────

#[test]
fn decode_jwt_header() {
    // {"alg":"RS256","typ":"JWT"} encoded
    let encoded = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9";
    let decoded = token_service::base64url_decode(encoded).unwrap();
    let s = String::from_utf8(decoded).unwrap();
    assert!(s.contains("RS256"));
    assert!(s.contains("JWT"));
}

#[test]
fn decode_empty_string() {
    let result = token_service::base64url_decode("");
    assert_eq!(result, Some(vec![]));
}

#[test]
fn decode_simple() {
    // "hello" -> "aGVsbG8"
    let result = token_service::base64url_decode("aGVsbG8").unwrap();
    assert_eq!(result, b"hello");
}

// ── hash_token ──────────────────────────────────────────────

#[test]
fn hash_token_produces_consistent_sha256() {
    let token = "header.payload.signature";
    let hash1 = token_service::hash_token(token);
    let hash2 = token_service::hash_token(token);
    assert_eq!(hash1, hash2);
    assert_eq!(hash1.len(), 64); // SHA-256 hex = 64 chars
}

#[test]
fn hash_token_differs_for_different_inputs() {
    let hash1 = token_service::hash_token("token-a");
    let hash2 = token_service::hash_token("token-b");
    assert_ne!(hash1, hash2);
}
