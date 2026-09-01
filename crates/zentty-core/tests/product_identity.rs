use zentty_core::{APPLICATION_ID, COMPACT_PRODUCT_NAME, FORK_ATTRIBUTION, PRODUCT_NAME};

#[test]
fn downstream_public_identity_is_tornadotty_and_attributes_upstream() {
    assert_eq!(PRODUCT_NAME, "Tornado TTY");
    assert_eq!(COMPACT_PRODUCT_NAME, "TornadoTTY");
    assert_eq!(APPLICATION_ID, "com.tamedtornado.tornadotty");
    assert!(FORK_ATTRIBUTION.contains("unofficial fork of Zentty"));
    assert!(FORK_ATTRIBUTION.contains("not affiliated with or endorsed by Zenjoy BV"));
}
