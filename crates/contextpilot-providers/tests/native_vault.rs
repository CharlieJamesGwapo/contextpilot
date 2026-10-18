//! Explicit native integration check; never uses a real API key or provider entry.
#[test]
#[ignore = "Explicit opt-in: creates and deletes a unique non-secret native-vault probe"]
fn isolated_native_vault_roundtrip() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let service = format!(
        "ContextPilot-offline-verification-{}-{nonce}",
        std::process::id()
    );
    let entry =
        keyring::Entry::new(&service, "non-secret-probe").expect("native vault entry unavailable");
    assert!(
        matches!(entry.get_password(), Err(keyring::Error::NoEntry)),
        "unique probe already exists or native vault unavailable"
    );
    entry
        .set_password("non-secret-offline-verification")
        .expect("native vault write unavailable");
    struct Cleanup(keyring::Entry);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = self.0.delete_credential();
        }
    }
    let cleanup = Cleanup(entry);
    let value = cleanup
        .0
        .get_password()
        .expect("native vault read unavailable");
    assert_eq!(value, "non-secret-offline-verification");
    cleanup
        .0
        .delete_credential()
        .expect("native vault cleanup unavailable");
    assert!(matches!(
        cleanup.0.get_password(),
        Err(keyring::Error::NoEntry)
    ));
}
