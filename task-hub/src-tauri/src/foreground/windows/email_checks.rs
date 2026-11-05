//! Pure comparisons for observations of the visible email composer.
use crate::model::Result;
use std::collections::BTreeSet;

#[derive(Debug, PartialEq)]
pub(super) enum RecipientState {
    Pending,
    Committed,
}

// A contact chip's visible name may be just "Daniel". Its accessible value
// or description can carry the address; never infer identity from that name.
pub(super) fn chip_address(metadata: &[String]) -> Result<String> {
    let addresses: BTreeSet<String> = metadata
        .iter()
        .flat_map(|value| value.split(|c: char| c.is_whitespace() || "<>,;()\"".contains(c)))
        .filter(|token| crate::validation::valid_email(token))
        .map(str::to_lowercase)
        .collect();
    if addresses.len() != 1 {
        return Err("A recipient chip is present, but its exact email address could not be verified from its accessibility information. A contact name alone is not sufficient. Nothing was sent.".into());
    }
    Ok(addresses.into_iter().next().unwrap())
}

// Gmail's chip menu exposes the complete address even for name-only chips.
// Inspect the label only; never invoke Copy or read/write the clipboard.
pub(super) fn chip_menu_address(labels: &[String]) -> Result<String> {
    let addresses: Vec<&str> = labels
        .iter()
        .filter_map(|label| {
            let address = label.strip_prefix("Copy \"")?.split_once('"')?.0;
            crate::validation::valid_email(address).then_some(address)
        })
        .collect();
    if addresses.len() != 1 {
        return Err("Gmail's recipient menu did not expose exactly one complete email address. Nothing was sent.".into());
    }
    Ok(addresses[0].to_lowercase())
}

pub(super) fn recipient_state(
    expected: &str,
    editor: &str,
    chips: &BTreeSet<String>,
) -> Result<RecipientState> {
    let expected = expected.to_lowercase();
    let editor = editor.trim();
    if chips.len() == 1 && chips.contains(&expected) && editor.is_empty() {
        return Ok(RecipientState::Committed);
    }
    if chips.is_empty() && editor.to_lowercase() == expected {
        return Ok(RecipientState::Pending);
    }
    if chips.is_empty() && editor.is_empty() {
        return Err("Recipient is empty: Gmail did not retain the typed address. Task Hub stopped rather than continue.".into());
    }
    Err("Recipient has not resolved to exactly the requested address. Inspect the To field; nothing was sent.".into())
}

// Shared by physical entry and verification; never collapse blank lines.
pub(super) fn message_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', " ")
}

pub(super) fn verify_body(actual: &str, expected: &str) -> Result<()> {
    verify_field("Message body", actual, &message_text(expected))
}

pub(super) fn canonical_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        // HTML contenteditable uses NBSP to represent ordinary visible spaces.
        .replace('\u{a0}', " ")
        .trim_end_matches('\n')
        .to_string()
}

pub(super) fn verify_field(label: &str, actual: &str, expected: &str) -> Result<()> {
    let actual = canonical_text(actual);
    let expected = canonical_text(expected);
    if actual == expected {
        return Ok(());
    }
    let offset = actual
        .chars()
        .zip(expected.chars())
        .position(|(actual, expected)| actual != expected)
        .unwrap_or_else(|| actual.chars().count().min(expected.chars().count()));
    let describe = |value: &str| match value.chars().nth(offset) {
        Some('\n') => "line break",
        Some(' ') => "space",
        Some(_) => "text character",
        None => "end of text",
    };
    let detail = format!("First difference at character {}: expected {}, observed {}. Blank lines must match; message text is omitted from this log.", offset + 1, describe(&expected), describe(&actual));
    Err(format!(
        "{label} verification failed (expected {} characters / {} lines, observed {} characters / {} lines). {detail} Nothing was sent.",
        expected.chars().count(), expected.lines().count(),
        actual.chars().count(), actual.lines().count()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn name_chip_menu_reveals_exact_address_without_using_clipboard() {
        let labels = vec![
            "Copy Ctrl+C".into(),
            "Copy \"dan@example.com\" Alt+C".into(),
            "Change email address Shift+F".into(),
        ];
        let address = chip_menu_address(&labels).unwrap();
        assert_eq!(address, "dan@example.com");
        assert!(recipient_state("wrong@example.com", "", &chips(&[&address])).is_err());
    }
    #[test]
    fn chip_menu_rejects_names_unrelated_text_and_multiple_addresses() {
        for labels in [
            vec!["Copy \"Daniel\"".into()],
            vec!["Email dan@example.com".into()],
            vec![
                "Copy \"a@example.com\"".into(),
                "Copy \"b@example.com\"".into(),
            ],
        ] {
            assert!(chip_menu_address(&labels).is_err());
        }
    }
    #[test]
    fn contact_name_chip_uses_its_underlying_address() {
        let address = chip_address(&[
            "Daniel".into(),
            "Daniel <dan@example.com>".into(),
            "dan@example.com".into(),
        ])
        .unwrap();
        assert_eq!(
            recipient_state("dan@example.com", "", &chips(&[&address])).unwrap(),
            RecipientState::Committed
        );
        assert!(recipient_state("other@example.com", "", &chips(&[&address])).is_err());
    }
    #[test]
    fn name_only_or_ambiguous_contact_chip_is_not_an_empty_recipient_or_verified_address() {
        assert!(chip_address(&["Daniel".into()])
            .unwrap_err()
            .contains("chip is present"));
        assert!(chip_address(&["Daniel <a@example.com> <b@example.com>".into()]).is_err());
    }
    #[test]
    fn entry_and_verification_share_message_normalization() {
        let expected = "Hello\r\n\r\nRegards,\t\rTest";
        assert_eq!(message_text(expected), "Hello\n\nRegards, \nTest");
        assert!(verify_body("Hello\r\n\r\nRegards,\u{a0}\r\nTest\r\n", expected).is_ok());
    }
    #[test]
    fn missing_internal_blank_line_is_not_skipped() {
        let error = verify_body("Hello\nRegards\nTest", "Hello\n\nRegards\nTest").unwrap_err();
        assert!(error.contains("expected line break, observed text character"));
        assert!(!error.contains("Hello"));
    }
    #[test]
    fn added_blank_lines_or_changed_spaces_are_not_skipped() {
        assert!(verify_body("Hello\n\nTest", "Hello\nTest").is_err());
        assert!(verify_body("Hello Test", "Hello  Test").is_err());
        assert!(verify_body("Hello\n", "Hello\nTest").is_err());
    }
    fn chips(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn typing_is_not_equivalent_to_a_committed_recipient() {
        assert_eq!(
            recipient_state("a@example.com", "a@example.com", &chips(&[])).unwrap(),
            RecipientState::Pending
        );
        assert_eq!(
            recipient_state("a@example.com", "", &chips(&["a@example.com"])).unwrap(),
            RecipientState::Committed
        );
    }
    #[test]
    fn missing_recipient_blocks_progress_before_other_fields() {
        assert!(recipient_state("a@example.com", "", &chips(&[]))
            .unwrap_err()
            .contains("Recipient is empty"));
    }
    #[test]
    fn incorrect_extra_or_uncommitted_addresses_are_rejected() {
        for (editor, observed) in [
            ("wrong@example.com", chips(&[])),
            ("", chips(&["wrong@example.com"])),
            ("", chips(&["a@example.com", "other@example.com"])),
            ("other@example.com", chips(&["a@example.com"])),
        ] {
            assert!(recipient_state("a@example.com", editor, &observed).is_err());
        }
    }
    #[test]
    fn html_spaces_and_windows_line_endings_compare_as_plain_text() {
        assert!(verify_field(
            "Message body",
            "Hello\r\n\r\nRegards,\u{a0}\r\nTest\r\n",
            "Hello\n\nRegards, \nTest"
        )
        .is_ok());
    }
    #[test]
    fn flattened_or_missing_body_text_is_not_accepted() {
        assert!(
            verify_field("Message body", "Hello Regards Test", "Hello\nRegards\nTest").is_err()
        );
        assert!(verify_field("Message body", "", "Hello").is_err());
        assert!(verify_field("Message body", "Hello  Test", "Hello Test").is_err());
    }
    #[test]
    fn mismatch_diagnostic_identifies_field_without_logging_content() {
        let error = verify_field(
            "Subject",
            "Private changed subject",
            "Private expected subject",
        )
        .unwrap_err();
        assert!(error.starts_with("Subject verification failed"));
        assert!(!error.contains("Private"));
    }
}
