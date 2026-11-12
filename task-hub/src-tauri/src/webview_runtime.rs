//! Apply renderer compatibility policy before Tauri starts any WebView threads.
fn browser_arguments(existing: &str) -> String {
    if existing
        .split_whitespace()
        .any(|arg| arg == "--disable-gpu")
    {
        return existing.into();
    }
    if existing.trim().is_empty() {
        "--disable-gpu".into()
    } else {
        format!("{existing} --disable-gpu")
    }
}
pub fn configure() {
    #[cfg(windows)]
    {
        // Process-local only: no system registry change and no user setting.
        // Called before Builder/plugins create threads or the WebView2 environment.
        let existing = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").unwrap_or_default();
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            browser_arguments(&existing),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn software_rendering_preserves_other_arguments_without_duplicates() {
        assert_eq!(browser_arguments(""), "--disable-gpu");
        assert_eq!(browser_arguments("--lang=en"), "--lang=en --disable-gpu");
        assert_eq!(
            browser_arguments("--disable-gpu --lang=en"),
            "--disable-gpu --lang=en"
        );
    }
}
