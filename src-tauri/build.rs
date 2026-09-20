fn main() {
    // Ohne diese Zeilen merkt Cargo eine neue icon.ico oder Oberfläche nicht: Das
    // Bauskript liefe nicht erneut und die fertige Exe trüge weiter das alte Symbol.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=capabilities");
    println!("cargo:rerun-if-changed=../ui");

    // Mit AppManifest stehen auch die eigenen Commands unter der ACL: Nur Fenster,
    // deren Capability `allow-<command>` nennt, dürfen sie aufrufen.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_state",
            "save_settings",
            "pick_workspace",
            "save_gmail_credentials",
            "delete_gmail_credentials",
            "start_run",
            "cancel_run",
            "list_jobs",
            "job_detail",
            "pick_profile",
            "remove_profile",
            "rewrite_txt",
            "clear_txt_files",
            "open_target",
            "reset_all",
            "report_ui_error",
            "portal_login",
            "portal_logout",
        ]),
    ))
    .expect("tauri-build fehlgeschlagen");
}
