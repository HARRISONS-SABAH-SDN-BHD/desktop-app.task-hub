//! Wait for a freshly resolved control to own focus before issuing text input.
use crate::model::Result;
use std::{
    thread,
    time::{Duration, Instant},
};

/// Names and CSS classes do not establish popup ownership. All evidence is required.
pub(super) fn permits_controlled_list(
    owner_is_combo: bool,
    controller_relation: bool,
    visible_enabled_list: bool,
    inside_target_window: bool,
) -> bool {
    owner_is_combo && controller_relation && visible_enabled_list && inside_target_window
}

pub(super) fn wait_for_control<T>(
    label: &str,
    timeout: Duration,
    mut resolve: impl FnMut() -> Result<T>,
    verify: impl Fn(&T) -> Result<()>,
    guard: impl Fn() -> Result<()>,
) -> Result<T> {
    let started = Instant::now();
    loop {
        // Focus loss to another window, cancellation, or Escape is terminal.
        guard()?;
        let error = match resolve().and_then(|control| verify(&control).map(|_| control)) {
            Ok(control) => return Ok(control),
            Err(error) => error,
        };
        if started.elapsed() >= timeout {
            return Err(format!("Could not bind keyboard focus to {label}: {error}"));
        }
        thread::sleep(Duration::from_millis(40));
    }
}

/// Check the semantic editor before its wrapper, so ComboBox popup ownership
/// is not lost when a browser delegates ordinary focus to a parent List.
pub(super) fn verify_recipient_binding<T>(
    editor: &T,
    wrapper: &T,
    verify: impl Fn(&T) -> Result<()>,
    verify_caret: impl Fn(&T) -> Result<()>,
) -> Result<()> {
    let editor_error = match verify(editor) {
        Ok(()) => return Ok(()),
        Err(error) => error,
    };
    if verify(wrapper).is_ok() || verify_caret(editor).is_ok() {
        return Ok(());
    }
    Err(format!("{editor_error} Neither recipient ownership nor an active caret in the To editor could be confirmed."))
}

pub(super) fn verify_recipient_prefix(actual: &str, prefix: &str) -> Result<()> {
    if actual == prefix {
        Ok(())
    } else {
        Err("The To input differs from the address prefix entered by Task Hub; typing stopped without replacing it.".into())
    }
}

/// A stale binding may be replaced, but characters already entered are never
/// replayed. Normal keystrokes use the live properties of the current binding.
pub(super) fn type_recipient<T>(
    text: &str,
    timeout: Duration,
    mut resolve: impl FnMut() -> Result<T>,
    verify: impl Fn(&T, &str) -> Result<()>,
    mut send: impl FnMut(char) -> Result<()>,
    guard: impl Fn() -> Result<()>,
) -> Result<()> {
    let mut prefix = String::new();
    let mut target = wait_for_control(
        "Gmail recipient input",
        timeout,
        &mut resolve,
        |target| verify(target, &prefix),
        &guard,
    )?;
    for ch in text.chars() {
        guard()?;
        if verify(&target, &prefix).is_err() {
            target = wait_for_control(
                "Gmail recipient input",
                timeout,
                &mut resolve,
                |target| verify(target, &prefix),
                &guard,
            )?;
        }
        guard()?;
        send(ch)?;
        prefix.push(ch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::cell::RefCell;

    #[test]
    fn recipient_popup_ownership_is_checked_on_combo_before_wrapper() {
        let inspected = RefCell::new(Vec::new());
        verify_recipient_binding(
            &"To ComboBox",
            &"Search Field List",
            |control| {
                inspected.borrow_mut().push(*control);
                if *control == "To ComboBox" {
                    Ok(())
                } else {
                    Err("not owned".into())
                }
            },
            |_| Err("caret unavailable".into()),
        )
        .unwrap();
        assert_eq!(*inspected.borrow(), ["To ComboBox"]);
    }

    #[test]
    fn suggestion_focus_requires_editor_caret_when_ownership_is_unavailable() {
        verify_recipient_binding(
            &"To ComboBox",
            &"Search Field List",
            |_| Err("suggestion is outside this control".into()),
            |control| {
                if *control == "To ComboBox" {
                    Ok(())
                } else {
                    Err("wrong editor".into())
                }
            },
        )
        .unwrap();
        assert!(verify_recipient_binding(
            &"To ComboBox",
            &"Search Field List",
            |_| Err("unrelated suggestion".into()),
            |_| Err("no active editor caret".into())
        )
        .is_err());
    }

    #[test]
    fn autocomplete_during_typing_does_not_replay_characters() {
        let current = Cell::new(0);
        let reads = Cell::new(0);
        let entered = RefCell::new(String::new());
        type_recipient(
            "a@example.com",
            Duration::ZERO,
            || {
                reads.set(reads.get() + 1);
                Ok(current.get())
            },
            |binding, prefix| {
                if *binding != current.get() {
                    return Err("editor replaced by autocomplete render".into());
                }
                verify_recipient_prefix(&entered.borrow(), prefix)
            },
            |ch| {
                entered.borrow_mut().push(ch);
                if entered.borrow().len() == 1 {
                    current.set(1);
                }
                Ok(())
            },
            || Ok(()),
        )
        .unwrap();
        assert_eq!(*entered.borrow(), "a@example.com");
        assert_eq!(reads.get(), 2);
    }

    #[test]
    fn edited_prefix_and_unrelated_focus_stop_before_next_character() {
        for focus_owned in [true, false] {
            let entered = RefCell::new(String::new());
            let error = type_recipient(
                "a@example.com",
                Duration::ZERO,
                || Ok(()),
                |_, prefix| {
                    if !prefix.is_empty() && !focus_owned {
                        return Err("focus moved to another field".into());
                    }
                    verify_recipient_prefix(&entered.borrow(), prefix)
                },
                |ch| {
                    entered.borrow_mut().push(ch);
                    if focus_owned {
                        entered.borrow_mut().push('x');
                    }
                    Ok(())
                },
                || Ok(()),
            )
            .unwrap_err();
            assert!(error.contains("prefix") || error.contains("another field"));
            assert_eq!(*entered.borrow(), if focus_owned { "ax" } else { "a" });
        }
    }

    #[test]
    fn recipient_cancellation_stops_before_resolving_or_typing() {
        let reads = Cell::new(0);
        let sends = Cell::new(0);
        assert!(type_recipient(
            "a@example.com",
            Duration::ZERO,
            || {
                reads.set(reads.get() + 1);
                Ok(())
            },
            |_, _| Ok(()),
            |_| {
                sends.set(sends.get() + 1);
                Ok(())
            },
            || Err("Stopped".into())
        )
        .is_err());
        assert_eq!(reads.get(), 0);
        assert_eq!(sends.get(), 0);
    }
    #[test]
    fn explicitly_owned_visible_list_can_keep_combo_focus() {
        assert!(permits_controlled_list(true, true, true, true));
    }
    #[test]
    fn a_search_field_name_alone_never_establishes_ownership() {
        assert!(!permits_controlled_list(true, false, true, true));
        assert!(!permits_controlled_list(false, true, true, true));
    }
    #[test]
    fn hidden_or_external_lists_are_rejected() {
        assert!(!permits_controlled_list(true, true, false, true));
        assert!(!permits_controlled_list(true, true, true, false));
    }
    #[test]
    fn replaced_input_is_resolved_again_before_typing() {
        let reads = Cell::new(0);
        let active = wait_for_control(
            "recipient",
            Duration::from_millis(300),
            || {
                reads.set(reads.get() + 1);
                Ok(if reads.get() == 1 {
                    "collapsed-old-node"
                } else {
                    "focused-new-input"
                })
            },
            |node| {
                if *node == "focused-new-input" {
                    Ok(())
                } else {
                    Err("old node no longer owns focus".into())
                }
            },
            || Ok(()),
        )
        .unwrap();
        assert_eq!(active, "focused-new-input");
        assert_eq!(reads.get(), 2);
    }
    #[test]
    fn other_control_focus_never_authorizes_input() {
        assert!(wait_for_control(
            "recipient",
            Duration::ZERO,
            || Ok("subject"),
            |_| Err("different field".into()),
            || Ok(())
        )
        .is_err());
    }
    #[test]
    fn cancellation_stops_before_any_control_lookup() {
        let reads = Cell::new(0);
        let error = wait_for_control(
            "recipient",
            Duration::from_secs(1),
            || {
                reads.set(reads.get() + 1);
                Ok(())
            },
            |_| Ok(()),
            || Err("Stopped".into()),
        )
        .unwrap_err();
        assert_eq!(error, "Stopped");
        assert_eq!(reads.get(), 0);
    }
}
