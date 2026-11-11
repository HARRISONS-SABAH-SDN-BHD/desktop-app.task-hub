mod email;
mod email_checks;
mod focus;
mod input;
mod session;
use super::{Controller, Operation, Recipe};
use crate::model::Result;
pub(super) fn escape_pressed() -> bool {
    input::interrupted()
}
pub(super) fn desktop_ready() -> bool {
    input::desktop_ready()
}
pub fn execute(controller: &Controller, recipe: &Recipe) -> Result<Vec<String>> {
    let mut session = session::Session::launch(controller, recipe)?;
    match &recipe.operation {
        Operation::Email {
            draft,
            send,
            direct,
        } => email::compose(&mut session, draft, *send, *direct)?,
    }
    Ok(controller.snapshot().map(|p| p.logs).unwrap_or_default())
}
