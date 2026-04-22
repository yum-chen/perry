use perry_container_compose::service::generate_name;

#[test]
fn test_generate_name_format() {
    let name = generate_name("web", "nginx");
    // Format: {short_hash}{random_suffix_hex}
    assert_eq!(name.len(), 8 + 8);
}

#[test]
fn test_generate_name_stable_per_image() {
    let name1 = generate_name("web", "nginx");
    let name2 = generate_name("api", "nginx");
    // short_hash is MD5(image)
    let hash1 = &name1[0..8];
    let hash2 = &name2[0..8];
    assert_eq!(hash1, hash2);
}

#[test]
fn test_generate_name_different_per_image() {
    let name1 = generate_name("web", "nginx");
    let name2 = generate_name("web", "redis");
    let hash1 = &name1[0..8];
    let hash2 = &name2[0..8];
    assert_ne!(hash1, hash2);
}
