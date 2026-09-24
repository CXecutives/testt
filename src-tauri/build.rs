fn main() {
    // Without these lines Cargo would not notice a new icon.ico or interface: the build
    // script would not run again and the finished exe would keep the old icon.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=capabilities");
    println!("cargo:rerun-if-changed=../ui");

    // With an AppManifest the app's own commands are under the ACL too: only windows whose
    // capability names `allow-<command>` may call them. The names agree with
    // `src/commands/mod.rs` and `capabilities/main.json` (`core/tests/contract.rs`).
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_state",
            "start_run",
            "cancel_run",
            "list_jobs",
            "job_detail",
            "mark_read",
            "set_pinned",
            "pick_profile",
            "remove_profile",
            "save_profile_template",
            "save_mailbox",
            "remove_mailbox",
            "portal_login",
            "portal_logout",
            "pick_workspace",
            "rewrite_txt",
            "clear_txt",
            "open_target",
            "save_settings",
            "reset_all",
            "report_ui_error",
        ]),
    ))
    .expect("tauri-build failed");
}
