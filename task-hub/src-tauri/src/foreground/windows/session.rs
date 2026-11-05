use super::input;

#[derive(Clone, Copy)]
pub(super) enum TextEntry {
    Plain,
    // Use explicit line breaks in a rich-text editor instead of paragraphs.
    Message,
}
use crate::{
    foreground::{recipes::App, Controller, Operation, Recipe},
    model::Result,
};
use std::{
    collections::HashSet,
    ffi::c_void,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use uiautomation::{
    patterns::{UITextPattern, UIValuePattern},
    types::{ControlType, Handle, Point, TreeScope},
    UIAutomation, UIElement,
};
use winreg::{
    enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY},
    RegKey,
};

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
    fn QueryFullProcessImageNameW(
        process: *mut c_void,
        flags: u32,
        path: *mut u16,
        length: *mut u32,
    ) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}
fn image_path(pid: u32) -> Option<String> {
    let process = unsafe { OpenProcess(0x1000, 0, pid) };
    if process.is_null() {
        return None;
    }
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) };
    unsafe {
        CloseHandle(process);
    }
    (ok != 0).then(|| String::from_utf16_lossy(&buffer[..length as usize]).to_lowercase())
}
fn app_path(name: &str) -> Option<PathBuf> {
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            if let Ok(key) = RegKey::predef(hive).open_subkey_with_flags(
                format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{name}"),
                KEY_READ | view,
            ) {
                if let Ok(value) = key.get_value::<String, _>("") {
                    let path = PathBuf::from(value.trim_matches('"'));
                    if path.is_file() {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}
fn desktop_windows(automation: &UIAutomation) -> Result<Vec<UIElement>> {
    let desktop = automation
        .get_root_element()
        .map_err(|e| format!("Could not access the Windows desktop: {e}"))?;
    let condition = automation
        .create_true_condition()
        .map_err(|e| format!("Could not prepare application window discovery: {e}"))?;
    // UIMatcher counts the desktop itself as depth 1. Search its children
    // explicitly so discovery reaches top-level windows without walking into apps.
    desktop
        .find_all(TreeScope::Children, &condition)
        .map_err(|e| format!("Could not enumerate desktop application windows: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_bar_labels_support_browser_and_search_engine_variants() {
        for label in [
            "Address and search bar",
            "Search or enter address",
            "Search with Google or enter address",
            "Search with DuckDuckGo or enter address",
            "Search with Bing or enter address",
        ] {
            assert!(is_address_bar_name(label), "{label}");
        }
        for label in [
            "Search in mail",
            "Search",
            "Message Body",
            "Search with Google",
        ] {
            assert!(!is_address_bar_name(label), "{label}");
        }
    }

    #[test]
    fn mailbox_validation_preserves_exact_https_provider_hosts() {
        assert!(validate_mailbox_url("mail.google.com/mail/u/0/#inbox", "Gmail").is_ok());
        assert!(
            validate_mailbox_url("https://outlook.live.com/mail/0/", "Outlook personal").is_ok()
        );
        assert!(validate_mailbox_url("https://outlook.office.com/mail/", "Outlook work").is_ok());
        for raw in [
            "http://mail.google.com/mail/",
            "https://mail.google.com.example.org/",
            "https://example.org/mail.google.com",
            "https://mail.google.com@example.org/",
            "https://someone@mail.google.com/",
        ] {
            assert!(validate_mailbox_url(raw, "Gmail").is_err(), "{raw}");
        }
        assert!(validate_mailbox_url("https://mail.google.com/", "Outlook personal").is_err());
        assert!(validate_mailbox_url("https://outlook.office.com/", "Unknown").is_err());
    }

    #[test]
    #[ignore = "requires an interactive desktop and TASKHUB_TEST_APP_EXE naming an open registered app"]
    fn desktop_child_enumeration_reaches_known_app() {
        let name = std::env::var("TASKHUB_TEST_APP_EXE")
            .expect("Set TASKHUB_TEST_APP_EXE to an open registered app, e.g. WINWORD.EXE");
        let expected = app_path(&name)
            .expect("The test app must have a registered installation path")
            .to_string_lossy()
            .to_lowercase();
        let automation = UIAutomation::new().expect("Windows UI Automation must be available");
        let windows = desktop_windows(&automation).expect("Desktop enumeration must succeed");
        assert!(
            windows.iter().any(|window| {
                window.get_process_id().ok().and_then(image_path).as_deref()
                    == Some(expected.as_str())
            }),
            "The open test app was missing from {} desktop child elements",
            windows.len()
        );
    }
}

#[derive(Clone)]
pub(super) struct RecipientInput {
    pub editor: UIElement,
    pub focus_owner: UIElement,
}

pub struct Session<'a> {
    pub automation: UIAutomation,
    pub root: UIElement,
    pub hwnd: isize,
    pub controller: &'a Controller,
    executable: String,
    started: Instant,
}
fn matches_name(name: &str, names: &[String]) -> bool {
    let name = name.to_lowercase();
    names.is_empty()
        || names.iter().any(|n| {
            name == *n || name.starts_with(&format!("{n} ")) || name.starts_with(&format!("{n},"))
        })
}
fn is_address_bar_name(name: &str) -> bool {
    let name = name.to_lowercase();
    [
        "address and search bar",
        "search or enter web address",
        "search or enter address",
        "search or type web address",
    ]
    .contains(&name.as_str())
        || (name.starts_with("search with ") && name.ends_with(" or enter address"))
}
fn in_browser_chrome(
    automation: &UIAutomation,
    root: &UIElement,
    element: &UIElement,
) -> uiautomation::Result<bool> {
    let walker = automation.get_control_view_walker()?;
    let mut current = element.clone();
    for _ in 0..32 {
        if automation.compare_elements(&current, root)? {
            return Ok(true);
        }
        if current.get_control_type()? == ControlType::Document {
            return Ok(false);
        }
        current = walker.get_parent(&current)?;
    }
    Ok(false)
}
fn validate_mailbox_url(raw: &str, provider: &str) -> Result<()> {
    let raw = raw.trim();
    let qualified = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("https://{raw}")
    };
    let url = url::Url::parse(&qualified).map_err(|_| "Invalid mailbox URL")?;
    let expected = match provider {
        "Gmail" => "mail.google.com",
        "Outlook personal" => "outlook.live.com",
        "Outlook work" => "outlook.office.com",
        _ => return Err("Unsupported email provider".into()),
    };
    if url.scheme() != "https"
        || url.host_str() != Some(expected)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(
            "The browser is not on the expected mailbox. Sign in manually before rerunning.".into(),
        );
    }
    Ok(())
}
impl<'a> Session<'a> {
    pub fn launch(controller: &'a Controller, recipe: &Recipe) -> Result<Self> {
        controller.check()?;
        let names: &[&str] = match recipe.app {
            App::Browser => match recipe.browser.as_deref() {
                Some("Mozilla Firefox") => &["firefox.exe"],
                Some("Google Chrome") => &["chrome.exe"],
                _ => &["msedge.exe"],
            },
            App::Word => &["WINWORD.EXE"],
            App::Excel => &["EXCEL.EXE"],
            App::PowerPoint => &["POWERPNT.EXE"],
        };
        let path = names.iter().find_map(|name| app_path(name)).ok_or_else(|| format!("Install {} and open it once to finish setup. Its registered application path was not found.", names.join(" or ")))?;
        let executable = path.to_string_lossy().to_lowercase();
        let automation = UIAutomation::new().map_err(|e| e.to_string())?;
        let list = || -> Result<Vec<UIElement>> {
            Ok(desktop_windows(&automation)?
                .into_iter()
                .filter(|e| {
                    e.get_process_id().ok().and_then(image_path).as_deref() == Some(&executable)
                })
                .collect())
        };
        let before: HashSet<isize> = list()?
            .iter()
            .filter_map(|e| e.get_native_window_handle().ok().map(Into::into))
            .collect();
        controller
            .log("Opening a new visible window. Existing documents and drafts are not reused.");
        controller.log(&format!(
            "Window discovery is watching {} ({} existing windows excluded).",
            names[0],
            before.len()
        ));
        let mut command = Command::new(&path);
        match &recipe.operation {
            Operation::Email { draft, .. } => {
                let url = match draft.provider.as_str() {
                    "Gmail" => "https://mail.google.com/mail/u/0/",
                    "Outlook personal" => "https://outlook.live.com/mail/0/",
                    _ => "https://outlook.office.com/mail/",
                };
                if recipe.browser.as_deref() == Some("Mozilla Firefox") {
                    command.args(["-new-window", url]);
                } else {
                    command.args(["--new-window", "--force-renderer-accessibility", url]);
                }
            }
            Operation::Word { .. } => {
                command.arg("/w");
            }
            Operation::Excel { .. } => {
                command.arg("/x");
            }
            Operation::PowerPoint { .. } => {
                command.arg("/n");
            }
        }
        command
            .spawn()
            .map_err(|e| format!("Could not open installed application: {e}"))?;
        let started = Instant::now();
        let root = loop {
            controller.check()?;
            if input::interrupted() {
                return Err("Stopped with Escape".into());
            }
            let windows = list()?;
            let matched_count = windows.len();
            let mut candidates: Vec<_> = windows
                .into_iter()
                .filter(|e| {
                    e.get_native_window_handle()
                        .ok()
                        .map(|h| !before.contains(&Into::<isize>::into(h)))
                        .unwrap_or(false)
                        && e.is_offscreen().ok() == Some(false)
                })
                .collect();
            if candidates.len() == 1 {
                break candidates.remove(0);
            }
            if started.elapsed() > Duration::from_secs(40) {
                return Err(format!(
                    "Could not identify exactly one new {} window within 40 seconds: found {} matching desktop windows, {} existing windows, and {} new visible candidates. Close startup dialogs, finish application setup, and run both apps without administrator elevation before retrying.",
                    names[0], matched_count, before.len(), candidates.len()
                ));
            }
            thread::sleep(Duration::from_millis(300));
        };
        let hwnd = root
            .get_native_window_handle()
            .map_err(|e| e.to_string())?
            .into();
        let session = Self {
            automation,
            root,
            hwnd,
            controller,
            executable,
            started: Instant::now(),
        };
        session.activate()?;
        session.wait(Duration::from_secs(2))?;
        Ok(session)
    }
    pub fn activate(&self) -> Result<()> {
        self.controller.check()?;
        unsafe {
            input::ShowWindow(self.hwnd as *mut c_void, 9);
            input::SetForegroundWindow(self.hwnd as *mut c_void);
        }
        thread::sleep(Duration::from_millis(300));
        self.guard()
    }
    pub fn guard(&self) -> Result<()> {
        self.controller.check()?;
        if input::interrupted() {
            return Err("Automation stopped with Escape. Partial work remains open.".into());
        }
        if self.started.elapsed() > Duration::from_secs(600) {
            return Err("Foreground run exceeded its ten-minute limit".into());
        }
        if unsafe { input::GetForegroundWindow() } as isize != self.hwnd {
            return Err("Focus moved away from the target window. Automation stopped before typing or clicking again.".into());
        }
        if self
            .root
            .get_process_id()
            .ok()
            .and_then(image_path)
            .as_deref()
            != Some(&self.executable)
        {
            return Err("The target application changed".into());
        }
        Ok(())
    }
    pub fn wait(&self, duration: Duration) -> Result<()> {
        let start = Instant::now();
        while start.elapsed() < duration {
            self.guard()?;
            thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }
    pub fn find_in(
        &self,
        root: &UIElement,
        names: &[&str],
        kinds: &[ControlType],
    ) -> Result<UIElement> {
        self.find_in_filtered(root, names, kinds, false)
    }
    fn find_in_filtered(
        &self,
        root: &UIElement,
        names: &[&str],
        kinds: &[ControlType],
        address_bar: bool,
    ) -> Result<UIElement> {
        self.guard()?;
        if names.is_empty() && kinds.is_empty() {
            return Err("A control lookup needs a name or a control type".into());
        }
        let description = if address_bar {
            "browser address bar (Edit or ComboBox)".to_string()
        } else {
            format!(
                "{} (types: {kinds:?})",
                if names.is_empty() {
                    "unique control".to_string()
                } else {
                    names.join(" / ")
                }
            )
        };
        let names: Vec<_> = names.iter().map(|s| s.to_lowercase()).collect();
        let exact_names = names.clone();
        let kinds = kinds.to_vec();
        let automation = self.automation.clone();
        let search_root = root.clone();
        let mut matches = self
            .automation
            .create_matcher()
            .from_ref(root)
            .depth(24)
            .timeout(0)
            .filter_fn(Box::new(move |element: &UIElement| {
                // A stale or unsupported property on an unrelated control must not
                // abort the whole application search. Unknown target properties fail closed.
                let inspect = || -> uiautomation::Result<bool> {
                    let name = element.get_name()?;
                    let name_matches = if address_bar {
                        is_address_bar_name(&name)
                    } else {
                        matches_name(&name, &names)
                    };
                    Ok(
                        (kinds.is_empty() || kinds.contains(&element.get_control_type()?))
                            && name_matches
                            && !element.is_offscreen()?
                            && element.is_enabled()?
                            && !element.is_password()?
                            && (!address_bar
                                || in_browser_chrome(&automation, &search_root, element)?),
                    )
                };
                Ok(inspect().unwrap_or(false))
            }))
            .find_all()
            .map_err(|e| format!("Could not locate visible {description}: {e}"))?;
        // Some browser providers expose a ComboBox and its nested Edit with the
        // same label. Use the Edit when available; multiple Edit matches still fail.
        if address_bar
            && matches
                .iter()
                .any(|e| e.get_control_type().ok() == Some(ControlType::Edit))
        {
            matches.retain(|e| e.get_control_type().ok() == Some(ControlType::Edit));
        }
        if matches.iter().any(|e| {
            e.get_name()
                .ok()
                .is_some_and(|name| exact_names.contains(&name.to_lowercase()))
        }) {
            matches.retain(|e| {
                e.get_name()
                    .ok()
                    .is_some_and(|name| exact_names.contains(&name.to_lowercase()))
            });
        }
        if matches.len() != 1 {
            return Err(format!(
                "Expected one visible {description}; found {}. Stop rather than guess a click.",
                matches.len()
            ));
        }
        Ok(matches.remove(0))
    }
    pub fn find(&self, names: &[&str], kinds: &[ControlType]) -> Result<UIElement> {
        self.find_in(&self.root, names, kinds)
    }
    pub fn wait_find(
        &self,
        names: &[&str],
        kinds: &[ControlType],
        seconds: u64,
    ) -> Result<UIElement> {
        let start = Instant::now();
        loop {
            self.guard()?;
            let last_error = match self.find(names, kinds) {
                Ok(element) => return Ok(element),
                Err(error) => error,
            };
            if start.elapsed() > Duration::from_secs(seconds) {
                return Err(format!("Control lookup timed out after {seconds}s: {last_error}. Finish application setup/sign-in manually if a dialog is blocking the workspace."));
            }
            thread::sleep(Duration::from_millis(300));
        }
    }
    pub fn wait_observe_text(&self, labels: &[&str], seconds: u64) -> Result<()> {
        let start = Instant::now();
        loop {
            self.guard()?;
            let labels: Vec<_> = labels.iter().map(|s| s.to_lowercase()).collect();
            let found = self
                .automation
                .create_matcher()
                .from_ref(&self.root)
                .depth(24)
                .timeout(0)
                .filter_fn(Box::new(move |element: &UIElement| {
                    let name = element.get_name()?.to_lowercase();
                    Ok(!element.is_offscreen()?
                        && labels.iter().any(|label| {
                            name == *label
                                || name.starts_with(&format!("{label}."))
                                || name.starts_with(&format!("{label} "))
                        }))
                }))
                .find_first()
                .is_ok();
            if found {
                return Ok(());
            }
            if start.elapsed() > Duration::from_secs(seconds) {
                return Err("Expected confirmation was not observed".into());
            }
            thread::sleep(Duration::from_millis(200));
        }
    }
    pub fn click(&self, element: &UIElement) -> Result<()> {
        self.click_control(element, false, false)
    }
    pub fn context_click(&self, element: &UIElement) -> Result<()> {
        self.click_control(element, false, true)
    }
    fn click_control(&self, element: &UIElement, text_start: bool, right: bool) -> Result<()> {
        self.guard()?;
        let rect = element
            .get_bounding_rectangle()
            .map_err(|e| e.to_string())?;
        if rect.get_width() < 2 || rect.get_height() < 2 {
            return Err("Target control has no clickable area".into());
        }
        let caret_point = if text_start {
            let viewport = self
                .root
                .get_bounding_rectangle()
                .map_err(|e| e.to_string())?;
            let left = rect.get_left().max(viewport.get_left());
            let top = rect.get_top().max(viewport.get_top());
            let right = rect.get_right().min(viewport.get_right());
            let bottom = rect.get_bottom().min(viewport.get_bottom());
            if right - left < 2 || bottom - top < 2 {
                return Err("Recipient editor has no visible clickable area".into());
            }
            Some((left + 6.min((right - left) / 2), top + (bottom - top) / 2))
        } else {
            None
        };
        let (x, y) = if let Some(point) = caret_point {
            point
        } else if let Ok(Some(point)) = element.get_clickable_point() {
            (point.get_x(), point.get_y())
        } else {
            // A Word page can extend below the viewport. Its full bounding-box
            // midpoint is not necessarily visible; intersect with the target window.
            let viewport = self
                .root
                .get_bounding_rectangle()
                .map_err(|e| e.to_string())?;
            let left = rect.get_left().max(viewport.get_left());
            let top = rect.get_top().max(viewport.get_top());
            let right = rect.get_right().min(viewport.get_right());
            let bottom = rect.get_bottom().min(viewport.get_bottom());
            if right - left < 2 || bottom - top < 2 {
                return Err("Target control has no visible clickable area".into());
            }
            (left + (right - left) / 2, top + (bottom - top) / 2)
        };
        let mut hit = self
            .automation
            .element_from_point(Point::new(x, y))
            .map_err(|e| e.to_string())?;
        let expected_id = element.get_runtime_id().map_err(|e| e.to_string())?;
        let walker = self
            .automation
            .get_control_view_walker()
            .map_err(|e| e.to_string())?;
        let mut verified = false;
        for _ in 0..10 {
            if hit.get_runtime_id().ok().as_ref() == Some(&expected_id) {
                verified = true;
                break;
            }
            match walker.get_parent(&hit) {
                Ok(parent) => hit = parent,
                Err(_) => break,
            }
        }
        if !verified {
            return Err(
                "The observed control is covered or changed. Automation stopped before clicking."
                    .into(),
            );
        }
        if right {
            input::context_click(self.hwnd, x, y)?;
        } else {
            input::click(self.hwnd, x, y)?;
        }
        thread::sleep(Duration::from_millis(120));
        Ok(())
    }
    pub fn press(&self, key: u16) -> Result<()> {
        self.guard()?;
        if input::modifiers_down() {
            return Err("Release modifier keys before automation runs".into());
        }
        input::press(key)?;
        self.wait(Duration::from_millis(100))
    }
    pub fn chord(&self, modifier: u16, key: u16) -> Result<()> {
        self.guard()?;
        if input::modifiers_down() {
            return Err("Release modifier keys before automation runs".into());
        }
        input::chord(modifier, key)?;
        self.wait(Duration::from_millis(150))
    }
    pub fn type_literal(&self, text: &str, focus: Option<&UIElement>) -> Result<()> {
        self.type_text(text, focus, TextEntry::Plain)
    }
    fn type_text(&self, text: &str, focus: Option<&UIElement>, entry: TextEntry) -> Result<()> {
        let anchor = input::cursor()?;
        for ch in text.chars().filter(|c| *c != '\r') {
            self.guard()?;
            let point = input::cursor()?;
            if (anchor.x - point.x).abs() > 3 || (anchor.y - point.y).abs() > 3 {
                return Err(
                    "The pointer moved during typing. Automation stopped to protect your work."
                        .into(),
                );
            }
            if input::modifiers_down() {
                return Err("A modifier key was pressed; automation stopped".into());
            }
            if let Some(expected) = focus {
                self.verify_input_focus(expected)?;
            }
            match ch {
                '\n' => match entry {
                    TextEntry::Plain => input::press(0x0D)?,
                    TextEntry::Message => input::chord(0x10, 0x0D)?,
                },
                '\t' => input::unicode(' ')?,
                _ => input::unicode(ch)?,
            }
            thread::sleep(Duration::from_millis(4));
        }
        Ok(())
    }
    pub fn fill(&self, element: &UIElement, value: &str) -> Result<()> {
        self.click(element)?;
        self.chord(0x11, 0x41)?;
        self.type_literal(value, Some(element))
    }
    fn element_is_within(&self, element: &UIElement, ancestor: &UIElement) -> Result<bool> {
        let walker = self
            .automation
            .get_raw_view_walker()
            .map_err(|e| e.to_string())?;
        let mut current = element.clone();
        for _ in 0..24 {
            if self
                .automation
                .compare_elements(&current, ancestor)
                .map_err(|e| e.to_string())?
            {
                return Ok(true);
            }
            if self
                .automation
                .compare_elements(&current, &self.root)
                .map_err(|e| e.to_string())?
            {
                return Ok(false);
            }
            match walker.get_parent(&current) {
                Ok(parent) => current = parent,
                Err(_) => return Ok(false),
            }
        }
        Ok(false)
    }
    fn verify_input_focus(&self, expected: &UIElement) -> Result<()> {
        self.guard()?;
        let focused = self
            .automation
            .get_focused_element()
            .map_err(|e| e.to_string())?;
        if expected.is_password().ok() != Some(false) || focused.is_password().ok() != Some(false) {
            return Err("Typing into unreadable or password controls is not permitted".into());
        }
        if self.element_is_within(&focused, expected)? {
            return Ok(());
        }
        let description = |element: &UIElement| {
            format!(
                "{:?} '{}' (class '{}')",
                element.get_control_type().ok(),
                element
                    .get_name()
                    .unwrap_or_default()
                    .chars()
                    .take(80)
                    .collect::<String>(),
                element.get_classname().unwrap_or_default()
            )
        };
        let actual = description(&focused);
        // ARIA-controlled lists may be outside the combo's ancestry. Accept
        // only provider-declared ownership, never a matching name/class alone.
        let mut related = Vec::new();
        if expected.get_control_type().ok() == Some(ControlType::ComboBox) {
            if let Ok(controlled) = expected.get_controller_for() {
                for popup in controlled.into_iter().take(8) {
                    related.push(description(&popup));
                    let live_list = popup.get_control_type().ok() == Some(ControlType::List)
                        && popup.is_offscreen().ok() == Some(false)
                        && popup.is_enabled().ok() == Some(true)
                        && popup.is_password().ok() == Some(false);
                    if live_list
                        && super::focus::permits_controlled_list(
                            true,
                            self.element_is_within(&focused, &popup)?,
                            live_list,
                            self.element_is_within(&focused, &self.root)?,
                        )
                    {
                        return Ok(());
                    }
                }
            }
        }
        Err(format!("Input focus does not belong to the intended field; expected {}, focused {actual}. Provider-declared controlled elements: [{}]. Typing stopped.", description(expected), related.join("; ")))
    }
    fn verify_recipient_focus(&self, target: &RecipientInput) -> Result<()> {
        super::focus::verify_recipient_binding(
            &target.editor,
            &target.focus_owner,
            |control| self.verify_input_focus(control),
            |editor| {
                self.guard()?;
                let focused = self
                    .automation
                    .get_focused_element()
                    .map_err(|e| e.to_string())?;
                // ARIA active-descendant focus can name a suggestion while the
                // text caret stays in the To editor. Require provider evidence
                // of that active caret, not the suggestion's label or CSS class.
                if focused.get_control_type().ok() != Some(ControlType::ListItem)
                    || focused.is_offscreen().ok() != Some(false)
                    || focused.is_enabled().ok() != Some(true)
                    || focused.is_password().ok() != Some(false)
                    || editor.is_offscreen().ok() != Some(false)
                    || editor.is_enabled().ok() != Some(true)
                    || editor.is_password().ok() != Some(false)
                    || editor
                        .get_pattern::<UIValuePattern>()
                        .and_then(|p| p.is_readonly())
                        .ok()
                        != Some(false)
                    || !self.element_is_within(&focused, &self.root)?
                {
                    return Err(
                        "The focused control is not a verifiable recipient suggestion".into(),
                    );
                }
                let (active, caret) = editor
                    .get_pattern::<UITextPattern>()
                    .and_then(|pattern| pattern.get_caret_range())
                    .map_err(|_| "The To editor does not expose an active text caret")?;
                let enclosing = caret.get_enclosing_element().map_err(|e| e.to_string())?;
                if !active || !self.element_is_within(&enclosing, editor)? {
                    return Err("The active caret is not inside the intended To editor".into());
                }
                Ok(())
            },
        )
    }
    pub fn fill_reacquired(
        &self,
        element: &UIElement,
        value: &str,
        label: &str,
        entry: TextEntry,
        resolve: impl Fn() -> Result<UIElement>,
    ) -> Result<()> {
        self.click(element)?;
        let focused = || {
            super::focus::wait_for_control(
                label,
                Duration::from_secs(2),
                &resolve,
                |control| self.verify_input_focus(control),
                || self.guard(),
            )
        };
        // Activation can replace the Gmail Recipient node with a new To input.
        let target = focused()?;
        self.verify_input_focus(&target)?;
        self.chord(0x11, 0x41)?;
        // Selection may trigger another render. Rebind before the first character.
        let target = focused()?;
        self.type_text(value, Some(&target), entry)
    }
    pub fn fill_recipient_reacquired(
        &self,
        element: &UIElement,
        value: &str,
        resolve: impl Fn() -> Result<RecipientInput>,
    ) -> Result<()> {
        self.controller
            .log("Placing the caret at the start of the visible recipient editor.");
        self.click_control(element, true, false)?;
        let anchor = input::cursor()?;
        super::focus::type_recipient(
            value,
            Duration::from_secs(2),
            &resolve,
            |target, prefix| {
                self.verify_recipient_focus(target)?;
                super::focus::verify_recipient_prefix(&Self::read(&target.editor)?, prefix)
            },
            |ch| {
                input::unicode(ch)?;
                thread::sleep(Duration::from_millis(4));
                Ok(())
            },
            || {
                self.guard()?;
                let point = input::cursor()?;
                if (anchor.x - point.x).abs() > 3 || (anchor.y - point.y).abs() > 3 {
                    return Err("The pointer moved during recipient entry; typing stopped".into());
                }
                if input::modifiers_down() {
                    return Err("A modifier key was pressed; recipient entry stopped".into());
                }
                Ok(())
            },
        )
    }
    pub fn read(element: &UIElement) -> Result<String> {
        if let Ok(pattern) = element.get_pattern::<UIValuePattern>() {
            return pattern.get_value().map_err(|e| e.to_string());
        }
        element
            .get_pattern::<UITextPattern>()
            .and_then(|p| p.get_document_range())
            .and_then(|r| r.get_text(20000))
            .map_err(|_| "The target does not expose readable text; verification stopped".into())
    }
    pub fn read_multiline(element: &UIElement) -> Result<String> {
        // Chromium's ValuePattern can flatten a contenteditable's line breaks.
        // TextPattern exposes the rendered document's text stream instead.
        match element.get_pattern::<UITextPattern>() {
            Ok(pattern) => pattern
                .get_document_range()
                .and_then(|range| range.get_text(20000))
                .map_err(|e| format!("Could not read multiline message text: {e}")),
            Err(_) => Self::read(element),
        }
    }
    pub fn bind_save_dialog(&mut self) -> Result<isize> {
        let original = self.hwnd;
        let start = Instant::now();
        loop {
            self.controller.check()?;
            let hwnd = unsafe { input::GetForegroundWindow() } as isize;
            if hwnd != original
                && unsafe { input::GetAncestor(hwnd as *mut c_void, 3) } as isize == original
            {
                let root = self
                    .automation
                    .element_from_handle(Handle::from(hwnd))
                    .map_err(|e| e.to_string())?;
                if root
                    .get_name()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains("save as")
                    && root.get_process_id().ok().and_then(image_path).as_deref()
                        == Some(&self.executable)
                {
                    self.root = root;
                    self.hwnd = hwnd;
                    return Ok(original);
                }
            }
            if input::interrupted() || start.elapsed() > Duration::from_secs(15) {
                return Err(
                    "Expected Save As dialog did not appear. No guessed input was sent.".into(),
                );
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
    pub fn restore_root(&mut self, hwnd: isize) -> Result<()> {
        self.hwnd = hwnd;
        self.root = self
            .automation
            .element_from_handle(Handle::from(hwnd))
            .map_err(|e| e.to_string())?;
        self.guard()
    }
    pub fn verify_url(&self, provider: &str) -> Result<()> {
        self.controller
            .log("Verifying the mailbox URL using the browser address bar.");
        let bar = self.find_in_filtered(
            &self.root,
            &[
                "Address and search bar",
                "Search or enter web address",
                "Search with Google or enter address",
                "Search or enter address",
            ],
            &[ControlType::Edit, ControlType::ComboBox],
            true,
        )?;
        let raw =
            Self::read(&bar).map_err(|e| format!("Could not read the browser address bar: {e}"))?;
        // Chromium can visually elide the HTTPS scheme; mailbox hosts below use HSTS.
        validate_mailbox_url(&raw, provider)
    }
    pub fn save_as(&mut self, path: &Path) -> Result<()> {
        if path.exists() {
            return Err("Output path already exists; refusing to overwrite".into());
        }
        self.controller
            .log("Saving the document through the application's Save As dialog.");
        // F12 opens Save As in the supported desktop Office apps.
        self.guard()?;
        input::press(0x7B)?;
        let original = self.bind_save_dialog()?;
        let name = self.find(&["File name:", "File name"], &[ControlType::Edit])?;
        self.fill(&name, &path.to_string_lossy())?;
        let save = self.find(&["Save"], &[ControlType::Button])?;
        self.click(&save)?;
        let start = Instant::now();
        while unsafe { input::GetForegroundWindow() } as isize != original {
            self.controller.check()?;
            if input::interrupted() || start.elapsed() > Duration::from_secs(15) {
                return Err("Save was not confirmed. Check any Office prompt manually.".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        self.restore_root(original)?;
        let start = Instant::now();
        while !path.is_file() || path.metadata().map(|m| m.len() == 0).unwrap_or(true) {
            self.wait(Duration::from_millis(200))?;
            if start.elapsed() > Duration::from_secs(15) {
                return Err("Office did not create the expected output file".into());
            }
        }
        self.controller
            .log("Verified the saved output file. The application is left open.");
        Ok(())
    }
}
