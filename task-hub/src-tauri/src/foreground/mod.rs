//! Legacy recipe validation and regression tests. Production email uses Gmail API.
mod controller;
mod recipes;
#[cfg(test)]
mod tests;
#[cfg(all(windows, test))]
mod windows;
#[cfg(test)]
use crate::model::Result;
pub use controller::{Controller, Progress};
pub use recipes::{validate_fields, Draft};
#[cfg(test)]
pub use recipes::{Operation, Recipe};
#[cfg(test)]
use std::{collections::BTreeMap, path::Path};
#[cfg(test)]
pub fn execute(
    controller: &Controller,
    id: &str,
    recipe: &str,
    fields: &BTreeMap<String, String>,
    output: &Path,
    allow_direct_send: bool,
) -> Result<(String, Vec<String>)> {
    let plan = Recipe::build(recipe, fields, output, id)?;
    if matches!(plan.operation, Operation::Email { direct: true, .. }) && !allow_direct_send {
        return Err("Direct sending requires explicit authorization for this run.".into());
    }
    controller.begin(id, &plan.label)?;
    #[cfg(windows)]
    let result = windows::execute(controller, &plan);
    #[cfg(not(windows))]
    let result: Result<Vec<String>> =
        Err("Foreground automation currently requires Windows.".into());
    controller.finish(result.is_ok());
    let logs = result?;
    // A receipt describes the visible email edits.
    let receipt = output.join(format!("{id}.foreground.txt"));
    crate::storage::atomic_write(
        &receipt,
        format!("{}\n\n{}\n", plan.label, logs.join("\n")).as_bytes(),
    )?;
    Ok((
        plan.artifact
            .unwrap_or(receipt)
            .to_string_lossy()
            .into_owned(),
        logs,
    ))
}
