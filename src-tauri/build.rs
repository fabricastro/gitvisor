fn main() {
    // `capabilities_path_pattern` suppresses tauri-build's own rerun-if-changed
    // for the capabilities directory, so emit it here.
    println!("cargo:rerun-if-changed=capabilities");

    // The harness capability names a permission that only exists when the
    // WebDriver plugin is compiled in. tauri-build validates every file the
    // glob matches, before the config selects any of them — so the glob itself
    // has to be the gate. One Cargo feature controls the dependency and the ACL.
    let pattern = if std::env::var_os("CARGO_FEATURE_E2E_WEBDRIVER").is_some() {
        "./capabilities/**/*"
    } else {
        "./capabilities/app/**/*"
    };

    tauri_build::try_build(tauri_build::Attributes::new().capabilities_path_pattern(pattern))
        .expect("tauri-build failed");
}
