use super::validate_web_provider_base_url_with_allowlist;

#[test]
fn reserved_provider_endpoints_are_refused() {
    for endpoint in [
        "https://user:password@api.openai.com/v1",
        "http://127.0.0.1:8080/v1",
        "http://10.0.0.7/v1",
        "http://169.254.169.254/latest/meta-data",
        "http://168.63.129.16/metadata/instance",
        "http://[::1]:8080/v1",
        "http://[fd00:ec2::254]/latest/meta-data",
        "http://metadata.google.internal/computeMetadata/v1",
        "https://provider.example.test/v1",
    ] {
        assert!(
            validate_web_provider_base_url_with_allowlist(endpoint, None).is_err(),
            "reserved endpoint was accepted: {endpoint}"
        );
    }
}

#[test]
fn a_public_https_origin_passes_without_allowlist() {
    assert!(validate_web_provider_base_url_with_allowlist(
        "https://custom-gateway.example/v1",
        None
    )
    .is_ok());
}

#[test]
fn explicit_endpoint_allowlist_can_opt_into_an_internal_origin() {
    assert!(validate_web_provider_base_url_with_allowlist(
        "http://127.0.0.1:11434/v1",
        Some("https://inference.example.com,http://127.0.0.1:11434"),
    )
    .is_ok());
    assert!(validate_web_provider_base_url_with_allowlist(
        "http://127.0.0.1:11435/v1",
        Some("http://127.0.0.1:11434"),
    )
    .is_err());
}
