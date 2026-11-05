//! Provider labels are intentionally isolated: UI changes should update this module.
use super::{
    email_checks::{self, RecipientState},
    session::{RecipientInput, Session, TextEntry},
};
use crate::{foreground::Draft, model::Result};
use std::{
    thread,
    time::{Duration, Instant},
};
use uiautomation::{
    patterns::{UILegacyIAccessiblePattern, UIValuePattern},
    types::ControlType,
    UIElement,
};
const SUBJECT: &[&str] = &["Subject", "Add a subject"];
const BODY: &[&str] = &["Message Body", "Message body", "Message content"];
// Include Gmail's Recipient label and its plural spelling.
// Keep one alias list for initial lookup, filling, and recipient verification.
const TO: &[&str] = &[
    "To recipients",
    "To",
    "To address",
    "Recipient",
    "Recipients",
];
const ACTIVE_TO: &[&str] = &["To recipients", "To", "To address"];
const COLLAPSED_TO: &[&str] = &["Recipient", "Recipients"];
fn field(session: &Session<'_>, names: &[&str]) -> Result<UIElement> {
    session
        .find(names, &[ControlType::Edit])
        .or_else(|_| session.find(names, &[ControlType::Document]))
        .or_else(|_| session.find(names, &[ControlType::ComboBox]))
}
fn field_in(session: &Session<'_>, root: &UIElement, names: &[&str]) -> Result<UIElement> {
    session
        .find_in(root, names, &[ControlType::Edit])
        .or_else(|_| session.find_in(root, names, &[ControlType::Document]))
        .or_else(|_| session.find_in(root, names, &[ControlType::ComboBox]))
}
fn compose_scope(session: &Session<'_>) -> Result<UIElement> {
    // Anchor generic "Recipient" labels to the composer containing the actual
    // subject and message fields, rather than searching inbox text for a click.
    let subject = field(session, SUBJECT)?;
    let body = field(session, BODY)?;
    let walker = session
        .automation
        .get_control_view_walker()
        .map_err(|e| e.to_string())?;
    let mut subject_ancestors = Vec::new();
    let mut current = subject;
    for _ in 0..20 {
        if session
            .automation
            .compare_elements(&current, &session.root)
            .map_err(|e| e.to_string())?
            || current.get_control_type().map_err(|e| e.to_string())? == ControlType::Document
        {
            break;
        }
        subject_ancestors.push(current.clone());
        match walker.get_parent(&current) {
            Ok(parent) => current = parent,
            Err(_) => break,
        }
    }
    let mut current = body;
    for _ in 0..20 {
        for ancestor in &subject_ancestors {
            if session
                .automation
                .compare_elements(&current, ancestor)
                .map_err(|e| e.to_string())?
            {
                return Ok(ancestor.clone());
            }
        }
        match walker.get_parent(&current) {
            Ok(parent) => current = parent,
            Err(_) => break,
        }
    }
    Err("Could not identify the compose container; refusing to click a Recipient label elsewhere in the mailbox".into())
}
fn recipient_field(session: &Session<'_>, provider: &str) -> Result<UIElement> {
    if provider != "Gmail" {
        return field(session, TO);
    }
    let scope = compose_scope(session)?;
    if let Ok(input) = field_in(session, &scope, ACTIVE_TO) {
        return Ok(input);
    }
    let collapsed = session
        .find_in(&scope, COLLAPSED_TO, &[])
        .or_else(|_| collapsed_recipient_row(session, &scope))?;
    session
        .controller
        .log("Opening Gmail's recipient summary to inspect the To field.");
    session.click(&collapsed)?;
    // Clicking the row changes the accessible name and may replace the node.
    // Re-query after activation; never type through the old collapsed control.
    let started = Instant::now();
    loop {
        session.guard()?;
        let result = compose_scope(session).and_then(|scope| field_in(session, &scope, TO));
        let error = match result {
            Ok(input) => return Ok(input),
            Err(error) => error,
        };
        if started.elapsed() >= Duration::from_secs(5) {
            return Err(format!(
                "Recipient row was clicked, but its editable To field did not appear: {error}"
            ));
        }
        thread::sleep(Duration::from_millis(200));
    }
}
fn collapsed_recipient_row(session: &Session<'_>, scope: &UIElement) -> Result<UIElement> {
    // After editing the body Gmail renders only a contact name/email above
    // Subject, with no Recipient label or visible To input. Find that row by
    // its relationship to Subject, not by matching user-provided message text.
    let subject = field_in(session, scope, SUBJECT)?;
    let subject_rect = subject
        .get_bounding_rectangle()
        .map_err(|e| e.to_string())?;
    let walker = session
        .automation
        .get_control_view_walker()
        .map_err(|e| e.to_string())?;
    let mut branch = subject;
    for _ in 0..20 {
        let parent = walker.get_parent(&branch).map_err(|e| e.to_string())?;
        if session
            .automation
            .compare_elements(&parent, scope)
            .map_err(|e| e.to_string())?
        {
            let row = walker
                .get_previous_sibling(&branch)
                .map_err(|_| "No recipient summary precedes Subject")?;
            let rect = row.get_bounding_rectangle().map_err(|e| e.to_string())?;
            if row.get_control_type().ok() != Some(ControlType::Group)
                || row.is_offscreen().ok() != Some(false)
                || row.is_enabled().ok() != Some(true)
                || row.is_password().ok() != Some(false)
                || rect.get_width() < 2
                || rect.get_height() < 2
                || rect.get_bottom() > subject_rect.get_top()
            {
                return Err(
                    "The collapsed Gmail recipient row could not be identified safely".into(),
                );
            }
            let children = session
                .automation
                .create_matcher()
                .from_ref(&row)
                .depth(4)
                .timeout(0)
                .find_all()
                .map_err(|e| e.to_string())?;
            if !children.iter().any(|element| {
                element.get_control_type().ok() == Some(ControlType::Text)
                    && element.is_offscreen().ok() == Some(false)
                    && element
                        .get_name()
                        .ok()
                        .is_some_and(|name| !name.trim().is_empty())
            }) {
                return Err("The collapsed Gmail recipient row contains no visible summary".into());
            }
            return Ok(row);
        }
        branch = parent;
    }
    Err("Could not locate the recipient summary within this composer".into())
}
fn recipient_typing_surface(session: &Session<'_>, provider: &str) -> Result<RecipientInput> {
    let scope = compose_scope(session)?;
    let input = field_in(session, &scope, TO)?;
    if provider != "Gmail" || input.get_control_type().ok() != Some(ControlType::ComboBox) {
        return Ok(RecipientInput {
            editor: input.clone(),
            focus_owner: input,
        });
    }
    let walker = session
        .automation
        .get_control_view_walker()
        .map_err(|e| e.to_string())?;
    let parent = walker.get_parent(&input).map_err(|e| e.to_string())?;
    // Observed Chrome/Gmail tree: List "Search Field" (afx) directly owns
    // ComboBox "To recipients" (agP aFx). The LIST owns keyboard focus.
    // This is an editing wrapper, not an ARIA-controlled suggestion popup.
    // Resolve it only through the To field inside this composer; never search
    // the window for a generic Search Field or allow arbitrary ancestors.
    if parent.get_control_type().ok() != Some(ControlType::List)
        || parent.get_name().ok().as_deref() != Some("Search Field")
        || parent.get_classname().ok().as_deref() != Some("afx")
    {
        return Ok(RecipientInput {
            editor: input.clone(),
            focus_owner: input,
        });
    }
    if parent.is_offscreen().ok() != Some(false)
        || parent.is_enabled().ok() != Some(true)
        || parent.is_password().ok() != Some(false)
        || parent
            .get_pattern::<UIValuePattern>()
            .and_then(|pattern| pattern.is_readonly())
            .ok()
            != Some(false)
    {
        return Err("Gmail recipient wrapper is not a visible, writable control".into());
    }
    let children = walker.get_children(&parent).unwrap_or_default();
    if children.len() != 1
        || !session
            .automation
            .compare_elements(&children[0], &input)
            .map_err(|e| e.to_string())?
    {
        return Err(
            "Gmail recipient wrapper does not exclusively own the intended To input".into(),
        );
    }
    Ok(RecipientInput {
        editor: input,
        focus_owner: parent,
    })
}
fn observe_until<T>(session: &Session<'_>, check: impl Fn() -> Result<T>) -> Result<T> {
    let started = Instant::now();
    loop {
        session.guard()?;
        let error = match check() {
            Ok(value) => return Ok(value),
            Err(error) => error,
        };
        if started.elapsed() >= Duration::from_secs(3) {
            return Err(error);
        }
        // Passive observation only; no retyping, clicking, or Send retries.
        thread::sleep(Duration::from_millis(100));
    }
}
fn chip_address_from_menu(
    session: &Session<'_>,
    chip: &UIElement,
    recipient: &UIElement,
) -> Result<String> {
    let walker = session
        .automation
        .get_control_view_walker()
        .map_err(|e| e.to_string())?;
    let owner = walker.get_parent(chip).map_err(|e| e.to_string())?;
    // Observed Gmail structure: a Group owns one chip and its context Menu.
    // Never read an arbitrary menu elsewhere in the browser or another chip.
    if owner.get_control_type().ok() != Some(ControlType::Group) {
        return Err("Could not identify this recipient chip's menu container".into());
    }
    let siblings = walker
        .get_children(&owner)
        .ok_or("Recipient chip container has no readable children")?;
    let chips: Vec<_> = siblings
        .iter()
        .filter(|e| e.get_control_type().ok() == Some(ControlType::ListItem))
        .collect();
    if chips.len() != 1
        || !session
            .automation
            .compare_elements(chips[0], chip)
            .map_err(|e| e.to_string())?
    {
        return Err("Recipient menu container does not uniquely own the intended chip".into());
    }
    session
        .controller
        .log("Inspecting the contact-name recipient's email address in its Gmail menu.");
    session.context_click(chip)?;
    let result = observe_until(session, || {
        let elements = session
            .automation
            .create_matcher()
            .from_ref(&owner)
            .depth(3)
            .timeout(0)
            .find_all()
            .map_err(|e| e.to_string())?;
        let labels = elements
            .iter()
            .filter(|e| {
                e.get_control_type().ok() == Some(ControlType::MenuItem)
                    && e.is_offscreen().ok() == Some(false)
            })
            .filter_map(|e| e.get_name().ok())
            .collect::<Vec<_>>();
        email_checks::chip_menu_address(&labels)
    });
    // Dismiss by returning to the To editor. Do not activate any menu command
    // or use Escape (which is also the automation's emergency stop).
    session.click(recipient)?;
    result
}
fn observed_recipient(session: &Session<'_>, draft: &Draft) -> Result<RecipientState> {
    let recipient = recipient_field(session, &draft.provider)?;
    let mut editor = Session::read(&recipient)?;
    // Live Chrome inspection shows committed chips as Option children in the
    // same List that directly owns the To ComboBox. Never scan the inbox.
    let walker = session
        .automation
        .get_control_view_walker()
        .map_err(|e| e.to_string())?;
    let parent = walker.get_parent(&recipient).map_err(|e| e.to_string())?;
    let values = session
        .automation
        .create_matcher()
        .from_ref(&parent)
        .depth(4)
        .timeout(0)
        .find_all()
        .map_err(|e| e.to_string())?;
    let mut addresses = std::collections::BTreeSet::new();
    for element in values {
        if element.is_offscreen().ok() != Some(false) {
            continue;
        }
        // Pending editor text is not a committed chip. Autocomplete suggestions
        // live outside this To input's owning List and are not searched here.
        if session
            .automation
            .compare_elements(&element, &recipient)
            .map_err(|e| e.to_string())?
            || session
                .automation
                .compare_elements(&element, &parent)
                .map_err(|e| e.to_string())?
        {
            continue;
        }
        let name = element.get_name().unwrap_or_default();
        if element.get_control_type().ok() == Some(ControlType::ListItem) {
            let mut metadata = vec![name, element.get_help_text().unwrap_or_default()];
            if let Ok(value) = element
                .get_pattern::<UIValuePattern>()
                .and_then(|p| p.get_value())
            {
                metadata.push(value);
            }
            if let Ok(legacy) = element.get_pattern::<UILegacyIAccessiblePattern>() {
                metadata.push(legacy.get_value().unwrap_or_default());
                metadata.push(legacy.get_description().unwrap_or_default());
            }
            let address = email_checks::chip_address(&metadata)
                .or_else(|_| chip_address_from_menu(session, &element, &recipient))?;
            addresses.insert(address);
            continue;
        }
        for token in name.split(|c: char| c.is_whitespace() || "<>,;()\"".contains(c)) {
            if crate::validation::valid_email(token) {
                addresses.insert(token.to_lowercase());
            }
        }
    }
    if editor.is_empty()
        && addresses.is_empty()
        && parent.get_control_type().ok() == Some(ControlType::List)
    {
        // Some provider versions expose pending text on the editing wrapper.
        editor = Session::read(&parent)?;
    }
    email_checks::recipient_state(&draft.recipient, &editor, &addresses)
}
fn verify_recipient(session: &Session<'_>, draft: &Draft) -> Result<()> {
    match observed_recipient(session, draft)? {
        RecipientState::Committed => Ok(()),
        RecipientState::Pending => Err("Recipient address is still uncommitted; no matching address chip was observed. Nothing was sent.".into()),
    }
}
fn verify_text(session: &Session<'_>, draft: &Draft) -> Result<()> {
    session.verify_url(&draft.provider)?;
    observe_until(session, || verify_recipient(session, draft))?;
    observe_until(session, || {
        email_checks::verify_field(
            "Subject",
            &Session::read(&field(session, SUBJECT)?)?,
            &draft.subject,
        )
    })?;
    observe_until(session, || {
        email_checks::verify_body(
            &Session::read_multiline(&field(session, BODY)?)?,
            &draft.body,
        )
    })?;
    let walker = session
        .automation
        .get_control_view_walker()
        .map_err(|e| e.to_string())?;
    // Fail closed if additional Cc/Bcc fields contain text.
    for names in [["Cc", "Cc recipients"], ["Bcc", "Bcc recipients"]] {
        if let Ok(element) = field(session, &names) {
            let parent = walker.get_parent(&element).map_err(|e| e.to_string())?;
            let chips = session
                .automation
                .create_matcher()
                .from_ref(&parent)
                .depth(4)
                .timeout(0)
                .find_all()
                .unwrap_or_default();
            let contains_address = chips.iter().any(|chip| {
                chip.get_name()
                    .unwrap_or_default()
                    .split(|c: char| c.is_whitespace() || "<>,;()\"".contains(c))
                    .any(crate::validation::valid_email)
            });
            if !Session::read(&element)?.trim().is_empty() || contains_address {
                return Err("Unexpected Cc/Bcc recipients. Nothing was sent by Task Hub.".into());
            }
        }
    }
    Ok(())
}
pub fn compose(session: &mut Session<'_>, draft: &Draft, send: bool, direct: bool) -> Result<()> {
    let mut draft = draft.clone();
    session
        .controller
        .log("Waiting for your signed-in mailbox. Task Hub never fills authentication forms.");
    let compose = session.wait_find(
        if draft.provider == "Gmail" {
            &["Compose"]
        } else {
            &["New mail", "New message"]
        },
        &[ControlType::Button],
        60,
    )?;
    session.verify_url(&draft.provider)?;
    session.click(&compose)?;
    session.wait_find(SUBJECT, &[ControlType::Edit], 15)?;
    let subject = field(session, SUBJECT)?;
    let body = field(session, BODY)?;
    let to = recipient_field(session, &draft.provider)?;
    for element in [&subject, &to] {
        if !Session::read(element)?.trim().is_empty() {
            return Err(
                "Compose fields were not empty. Existing drafts are not overwritten.".into(),
            );
        }
    }
    let signature = Session::read_multiline(&body)?;
    if !signature.trim().is_empty() {
        if direct {
            return Err("Direct send requires an empty message body so the authorized content stays exact. Use Send after review to retain the provider's default signature.".into());
        }
        if signature.chars().count() > 4000 {
            return Err(
                "Unexpected compose content; refusing to replace a potentially existing draft"
                    .into(),
            );
        }
        draft.body = format!("{}\n\n{}", draft.body, signature);
        session
            .controller
            .log("Retaining the compose window's default signature for review.");
    }
    session
        .controller
        .log("Entering the recipient before the subject and body.");
    let to = recipient_field(session, &draft.provider)?;
    session.fill_recipient_reacquired(&to, &draft.recipient, || {
        recipient_typing_surface(session, &draft.provider)
    })?;
    if observe_until(session, || observed_recipient(session, &draft))? == RecipientState::Pending {
        session
            .controller
            .log("Committing the verified recipient address in Gmail.");
        // Gmail can report focus on an autocomplete suggestion outside the
        // input's subtree. Tab accepts the pending address and leaves the field
        // without activating a suggestion with Enter. The exact input value was
        // verified above; the resulting chip must be verified below.
        session.press(0x09)?;
    }
    observe_until(session, || verify_recipient(session, &draft))?;
    session
        .controller
        .log("Verified exactly one intended recipient address chip.");
    session
        .controller
        .log("Typing the subject and message into the visible compose window.");
    let subject = field(session, SUBJECT)?;
    session.fill_reacquired(
        &subject,
        &draft.subject,
        "email subject",
        TextEntry::Plain,
        || field(session, SUBJECT),
    )?;
    let body = field(session, BODY)?;
    let body_text = email_checks::message_text(&draft.body);
    session.fill_reacquired(
        &body,
        &body_text,
        "email message body",
        TextEntry::Message,
        || field(session, BODY),
    )?;
    verify_text(session, &draft)?;
    if !send {
        session.wait(Duration::from_secs(2))?;
        session.controller.log("Draft prepared and left visible in the mailbox. Provider auto-save and delivery have not been assumed.");
        return Ok(());
    }
    if !direct {
        session
            .controller
            .log("Draft verified. Awaiting your explicit approval before sending.");
        session
            .controller
            .await_send(&draft, Duration::from_secs(180))?;
        session.activate()?;
    } else {
        session
            .controller
            .log("Direct sending was explicitly authorized for this exact run.");
    }
    verify_text(session, &draft)?;
    let send_button = session.find(&["Send"], &[ControlType::Button])?;
    session
        .controller
        .log("Approval received. Clicking Send once; this action is never retried automatically.");
    session.click(&send_button)?;
    session.wait_observe_text(&["Message sent", "Your message has been sent", "Sent successfully"], 15)
        .map_err(|_| "Send was clicked, but the provider's success message could not be verified. Outcome is uncertain: inspect Sent mail before retrying.".to_string())?;
    session.controller.log("Observed the provider's sent confirmation. Final delivery depends on the recipient's provider.");
    Ok(())
}
