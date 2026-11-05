use super::session::Session;
use crate::{
    foreground::{Operation, Recipe},
    model::Result,
};
use std::time::Duration;
use uiautomation::types::ControlType;

fn blank(session: &Session<'_>, names: &[&str]) -> Result<()> {
    // Some Office installations open their start screen, others already expose a blank document.
    if let Ok(button) = session.find(names, &[]) {
        session.click(&button)?;
        session.wait(Duration::from_secs(1))?;
    }
    Ok(())
}
pub fn create(session: &mut Session<'_>, recipe: &Recipe) -> Result<()> {
    match &recipe.operation {
        Operation::Word { title, paragraphs } => {
            blank(session, &["Blank document"])?;
            session
                .controller
                .log("Locating Word's document and Page 1 content editing surface.");
            // Word names the document after the file (e.g. Document1), while
            // "Page 1 content" is an Edit child, not a Document control.
            let document = session.wait_find(&[], &[ControlType::Document], 20)?;
            let page = session.find_in(
                &document,
                &["Page 1 content", "Page 1"],
                &[ControlType::Edit],
            )?;
            if !Session::read(&document)?.trim().is_empty() {
                return Err("Word document was not empty; existing content is not replaced".into());
            }
            session.click(&page)?;
            session
                .controller
                .log("Typing the report into the Word editing surface.");
            session.type_literal(
                &format!("{title}\n\n{}", paragraphs.join("\n")),
                Some(&document),
            )?;
            // The text must be observable before we save.
            let written = Session::read(&document)?;
            if !written.contains(title)
                || paragraphs
                    .iter()
                    .any(|p| !p.is_empty() && !written.contains(p))
            {
                return Err(
                    "Could not verify all report text in Word; inspect the open document".into(),
                );
            }
        }
        Operation::Excel { rows } => {
            blank(session, &["Blank workbook"])?;
            let name_box = session
                .wait_find(&["Name Box"], &[ControlType::ComboBox], 20)
                .or_else(|_| session.find(&["Name Box"], &[ControlType::Edit]))?;
            session.click(&name_box)?;
            session.chord(0x11, 0x41)?;
            session.type_literal("A1", None)?;
            session.press(0x0D)?;
            for (index, row) in rows.iter().enumerate() {
                session.controller.log(&format!(
                    "Typing spreadsheet row {} of {}.",
                    index + 1,
                    rows.len()
                ));
                for (column, value) in row.iter().enumerate() {
                    session.type_literal(value, None)?;
                    session.press(if column + 1 < row.len() { 0x09 } else { 0x0D })?;
                }
            }
        }
        Operation::PowerPoint { slides } => {
            blank(session, &["Blank Presentation", "Blank presentation"])?;
            for (index, (title, content)) in slides.iter().enumerate() {
                if index > 0 {
                    session.chord(0x11, 0x4D)?;
                }
                session
                    .controller
                    .log(&format!("Typing slide {} of {}.", index + 1, slides.len()));
                let heading =
                    session.wait_find(&["Title 1", "Title", "Click to add title"], &[], 15)?;
                session.click(&heading)?;
                session.press(0x71)?; // F2 enters the observed shape's text editor.
                session.type_literal(title, None)?;
                session.press(0x1B)?;
                if !content.is_empty() {
                    let text = session.find(
                        &[
                            "Content Placeholder 2",
                            "Content 2",
                            "Subtitle 2",
                            "Text Placeholder 2",
                            "Click to add text",
                            "Click to add subtitle",
                        ],
                        &[],
                    )?;
                    session.click(&text)?;
                    session.press(0x71)?;
                    session.type_literal(&content.join("\n"), None)?;
                    session.press(0x1B)?;
                }
            }
        }
        Operation::Email { .. } => {
            return Err("Email recipe cannot run through the Office adapter".into())
        }
    }
    session.save_as(
        recipe
            .artifact
            .as_ref()
            .ok_or("Missing Office output path")?,
    )?;
    Ok(())
}
