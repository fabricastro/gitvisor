#[cfg(all(feature = "e2e-webdriver", not(debug_assertions)))]
compile_error!(
    "e2e-webdriver embeds a WebDriver control server and must never be built in release"
);

mod commands;
mod state;

use state::RepoRegistry;

pub fn run() {
    let builder = tauri::Builder::default();

    // Embedded WebDriver server used by the end-to-end harness. Debug builds
    // only: this is a remote-control surface and must never ship in a release.
    #[cfg(all(feature = "e2e-webdriver", debug_assertions))]
    let builder = builder.plugin(tauri_plugin_wdio_webdriver::init());

    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(RepoRegistry::default())
        .invoke_handler(tauri::generate_handler![
            commands::startup_path,
            commands::open_repository,
            commands::close_repository,
            commands::list_refs,
            commands::commit_graph,
            commands::commit_detail,
            commands::working_status,
            commands::stage_paths,
            commands::unstage_paths,
            commands::git_probe,
            commands::create_commit,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the application");
}
