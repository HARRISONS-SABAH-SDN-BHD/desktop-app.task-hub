fn main() {
    println!("cargo:rerun-if-env-changed=TASK_HUB_GOOGLE_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=TASK_HUB_GOOGLE_CLIENT_SECRET");
    #[cfg(feature = "desktop")]
    tauri_build::build()
}
