use crate::model::Result;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize)]
pub struct Draft {
    pub provider: String,
    pub recipient: String,
    pub subject: String,
    pub body: String,
}
#[derive(Clone, Debug, PartialEq)]
pub enum App {
    Browser,
}
#[derive(Clone, Debug)]
pub enum Operation {
    Email {
        draft: Draft,
        send: bool,
        direct: bool,
    },
}
#[derive(Clone, Debug)]
pub struct Recipe {
    pub label: String,
    pub app: App,
    pub operation: Operation,
    pub artifact: Option<PathBuf>,
    pub browser: Option<String>,
}
fn field<'a>(fields: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str> {
    fields
        .get(key)
        .map(String::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("Missing foreground field: {key}"))
}
pub fn validate_fields(recipe: &str, fields: &BTreeMap<String, String>) -> Result<()> {
    Recipe::build(recipe, fields, Path::new("outputs"), "validation").map(|_| ())
}
impl Recipe {
    pub fn build(
        recipe: &str,
        fields: &BTreeMap<String, String>,
        _output: &Path,
        _id: &str,
    ) -> Result<Self> {
        let title = field(fields, "title")?.to_string();
        if title.chars().count() > 160 || title.contains(['\n', '\r', '\t']) {
            return Err("Use a single-line title of at most 160 characters".into());
        }
        let body = field(fields, "body")?;
        if body.chars().count() > 12000 {
            return Err("Foreground content is limited to 12,000 characters per run".into());
        }
        match recipe {
            "web_email" => {
                let browser = fields
                    .get("browser")
                    .map(String::as_str)
                    .unwrap_or("Microsoft Edge");
                if !["Microsoft Edge", "Google Chrome", "Mozilla Firefox"].contains(&browser) {
                    return Err("Choose a supported browser".into());
                }
                let provider = field(fields, "provider")?;
                if !["Gmail"].contains(&provider) {
                    return Err("Unsupported email provider".into());
                }
                let recipient = field(fields, "recipient")?;
                if !crate::validation::valid_email(recipient) {
                    return Err("Enter one valid recipient email address".into());
                }
                let delivery = field(fields, "delivery")?;
                if !["Draft", "Send after review", "Send directly"].contains(&delivery) {
                    return Err("Choose Draft, Send after review, or Send directly".into());
                }
                Ok(Self {
                    browser: Some(browser.into()),
                    label: format!("Compose in {provider}"),
                    app: App::Browser,
                    artifact: None,
                    operation: Operation::Email {
                        draft: Draft {
                            provider: provider.into(),
                            recipient: recipient.into(),
                            subject: title,
                            body: body.into(),
                        },
                        send: delivery != "Draft",
                        direct: delivery == "Send directly",
                    },
                })
            }
            _ => Err("Foreground recipe is not registered".into()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fields() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("title".into(), "Report".into()),
            ("body".into(), "Name,Value\nTask,=1+1".into()),
        ])
    }
    #[test]
    fn unknown_recipes_and_ambiguous_delivery_are_rejected() {
        assert!(Recipe::build("powershell", &fields(), Path::new("outputs"), "id").is_err());
        let mut values = fields();
        values.extend([
            ("provider".into(), "Gmail".into()),
            ("recipient".into(), "a@example.com".into()),
            ("delivery".into(), "send maybe".into()),
        ]);
        assert!(validate_fields("web_email", &values).is_err());
        values.insert("delivery".into(), "Draft".into());
        assert!(validate_fields("web_email", &values).is_ok());
    }
}
