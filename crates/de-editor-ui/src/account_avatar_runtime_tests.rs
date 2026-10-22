use super::*;

const URL: &str = "https://cdn.example/account.png";

fn png_header(width: u32, height: u32, payload_bytes: usize) -> Vec<u8> {
    let mut bytes = vec![0; payload_bytes.max(24)];
    bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    bytes[8..12].copy_from_slice(&13_u32.to_be_bytes());
    bytes[12..16].copy_from_slice(b"IHDR");
    bytes[16..20].copy_from_slice(&width.to_be_bytes());
    bytes[20..24].copy_from_slice(&height.to_be_bytes());
    bytes
}

fn fetch(registry: &mut AvatarRegistry, url: &str) -> AccountAvatarFetchRequest {
    assert!(registry.lookup(url).is_none());
    registry.take_requests(1).pop().expect("request queued")
}

#[test]
fn success_is_cached_and_a_failure_keeps_the_fallback() {
    let mut registry = AvatarRegistry::new();
    let ok = fetch(&mut registry, URL);
    assert!(registry.complete(&ok, Some(png_header(16, 16, 48))));
    let image = registry.lookup(URL).expect("valid response is ready");
    assert_eq!(image.image_id, ok.image_id);
    assert!(registry.bytes_for(ok.image_id).is_some());

    let failed = fetch(&mut registry, "https://cdn.example/other.png");
    assert!(registry.complete(&failed, None));
    assert!(registry.ready().is_none());
    assert!(registry.take_requests(1).is_empty());
}

#[test]
fn encoded_bytes_and_dimensions_are_bounded() {
    let mut registry = AvatarRegistry::new();
    let large_body = fetch(&mut registry, "https://cdn.example/large.png");
    assert!(registry.complete(&large_body, Some(vec![0; MAX_AVATAR_ENCODED_BYTES + 1])));
    assert!(registry.lookup("https://cdn.example/large.png").is_none());

    let large_raster = fetch(&mut registry, "https://cdn.example/wide.png");
    assert!(registry.complete(
        &large_raster,
        Some(png_header(MAX_AVATAR_SOURCE_EDGE_PX + 1, 1, 24))
    ));
    assert!(registry.lookup("https://cdn.example/wide.png").is_none());
}

#[test]
fn a_profile_url_change_drops_the_stale_result() {
    let mut registry = AvatarRegistry::new();
    let old = fetch(&mut registry, "https://cdn.example/old.png");
    assert!(registry.lookup("https://cdn.example/new.png").is_none());
    let new = registry.take_requests(1).pop().expect("new profile queued");
    assert!(!registry.complete(&old, Some(png_header(2, 2, 32))));
    assert!(registry.lookup("https://cdn.example/new.png").is_none());
    assert!(registry.complete(&new, Some(png_header(2, 2, 32))));
    assert_eq!(
        registry
            .lookup("https://cdn.example/new.png")
            .expect("new profile wins")
            .image_id,
        new.image_id
    );
}

#[test]
fn failed_account_fetch_retries_with_backoff_and_keeps_the_fallback() {
    let mut registry = AvatarRegistry::new();
    let failed = fetch(&mut registry, URL);
    assert!(registry.complete(&failed, None));
    assert!(registry.ready().is_none());
    assert!(registry.has_background_work());
    assert!(registry.take_requests(1).is_empty());

    registry.slot.as_mut().expect("account slot").retry_at = Some(Instant::now());
    assert!(registry.ready().is_none());
    let retried = registry
        .take_requests(1)
        .pop()
        .expect("due account retry queued");
    assert_eq!(retried.image_id, failed.image_id);
    assert!(registry.complete(&retried, Some(png_header(16, 16, 32))));
    assert!(registry.ready().is_some());
    assert!(!registry.has_background_work());
}

#[test]
fn proxy_install_replaces_only_on_a_valid_new_revision() {
    let _guard = lock_account_avatar_registry_for_tests();
    assert!(install_account_avatar_bytes(
        "revision-one",
        png_header(16, 16, 32)
    ));
    let first = account_avatar_image().expect("first proxy image");
    assert!(!install_account_avatar_bytes(
        "../unsafe",
        png_header(16, 16, 32)
    ));
    assert_eq!(
        account_avatar_image()
            .expect("invalid revision keeps current")
            .image_id,
        first.image_id
    );
    assert!(install_account_avatar_bytes(
        "revision-two",
        png_header(16, 16, 32)
    ));
    assert_ne!(
        account_avatar_image().expect("new proxy image").image_id,
        first.image_id
    );
}

#[test]
fn request_debug_redacts_the_url() {
    let mut registry = AvatarRegistry::new();
    let request = fetch(&mut registry, "https://cdn.example/secret.png");
    let debug = format!("{request:?}");
    assert!(!debug.contains("cdn.example"));
    assert!(debug.contains("[REDACTED]"));
}
