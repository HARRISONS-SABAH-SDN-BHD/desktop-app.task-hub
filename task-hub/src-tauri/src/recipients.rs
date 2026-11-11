//! Recipient-file metadata is independent of outgoing email attachments.
use crate::{model::Result, validation};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipientFile {
    pub name: String,
    pub recipients: Vec<String>,
}
pub fn validate(file: &Option<RecipientFile>, selected: &str) -> Result<()> {
    let addresses = validation::to_addresses(selected)?;
    if let Some(file) = file {
        if file.name.is_empty() || file.name.len() > 255 || file.name.chars().any(char::is_control)
        {
            return Err("Invalid recipient filename".into());
        }
        validation::to_addresses(&file.recipients.join(", "))?;
        if addresses
            .iter()
            .any(|v| !file.recipients.iter().any(|item| item == v))
        {
            return Err("Select recipients from the imported file".into());
        }
    }
    Ok(())
}
pub fn import(name: String, data: String) -> Result<Vec<String>> {
    if name.len() > 255 || data.len() > 2_800_000 {
        return Err("Recipient files must be 2 MB or smaller".into());
    }
    let result = crate::runtime::request(
        serde_json::json!({"operation":"read_recipients","name":name,"data":data}),
    )?;
    let addresses: Vec<String> =
        serde_json::from_value(result).map_err(|_| "Could not read recipient list")?;
    validation::to_addresses(&addresses.join(", "))?;
    Ok(addresses)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selections_must_belong_to_the_saved_file_and_cannot_be_empty() {
        let file = Some(RecipientFile {
            name: "team.csv".into(),
            recipients: vec!["a@example.com".into(), "b@example.com".into()],
        });
        assert!(validate(&file, "b@example.com").is_ok());
        assert!(validate(&file, "c@example.com").is_err());
        assert!(validate(&file, "").is_err());
        assert!(validate(&file, "a@example.com, A@example.com").is_err());
    }
}
