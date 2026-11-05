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
    Word,
    Excel,
    PowerPoint,
}
#[derive(Clone, Debug)]
pub enum Operation {
    Email {
        draft: Draft,
        send: bool,
        direct: bool,
    },
    Word {
        title: String,
        paragraphs: Vec<String>,
    },
    Excel {
        rows: Vec<Vec<String>>,
    },
    PowerPoint {
        slides: Vec<(String, Vec<String>)>,
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
        output: &Path,
        id: &str,
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
                if !["Gmail", "Outlook personal", "Outlook work"].contains(&provider) {
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
            "word_report" => Ok(Self {
                browser: None,
                label: "Write a report in Microsoft Word".into(),
                app: App::Word,
                artifact: Some(output.join(format!("{id}.docx"))),
                operation: Operation::Word {
                    title,
                    paragraphs: body
                        .replace("\r\n", "\n")
                        .split('\n')
                        .map(String::from)
                        .collect(),
                },
            }),
            "excel_report" => {
                let mut rows = crate::adapters::documents::csv_rows(body)?;
                if rows.len() > 100 || rows.iter().map(Vec::len).sum::<usize>() > 500 {
                    return Err("Foreground Excel is limited to 100 rows / 500 cells. Use file export for larger data.".into());
                }
                for row in &mut rows {
                    for value in row {
                        if value.contains(['\n', '\r', '\t']) || value.chars().count() > 1000 {
                            return Err("Foreground cells must be single-line text of at most 1,000 characters".into());
                        }
                        // Every cell is entered as literal text, including numbers and formula-looking strings.
                        value.insert(0, '\'');
                    }
                }
                Ok(Self {
                    browser: None,
                    label: "Enter data in Microsoft Excel".into(),
                    app: App::Excel,
                    artifact: Some(output.join(format!("{id}.xlsx"))),
                    operation: Operation::Excel { rows },
                })
            }
            "powerpoint_deck" => {
                let normalized = body.replace("\r\n", "\n");
                let mut slides = vec![(title, vec!["Created with Task Hub".into()])];
                for section in normalized.split("\n---\n").filter(|s| !s.trim().is_empty()) {
                    let mut lines = section.lines();
                    let heading = lines.next().unwrap_or("").to_string();
                    let content = lines.map(String::from).collect::<Vec<_>>();
                    if heading.chars().count() > 160
                        || content.len() > 10
                        || section.chars().count() > 1000
                    {
                        return Err("Keep each slide to a short heading and at most 10 content lines / 1,000 characters".into());
                    }
                    slides.push((heading, content));
                }
                if slides.len() > 21 {
                    return Err("Foreground PowerPoint is limited to 20 content slides".into());
                }
                Ok(Self {
                    browser: None,
                    label: "Build slides in Microsoft PowerPoint".into(),
                    app: App::PowerPoint,
                    artifact: Some(output.join(format!("{id}.pptx"))),
                    operation: Operation::PowerPoint { slides },
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
    fn excel_values_are_literal_and_save_paths_are_owned_by_runner() {
        let recipe =
            Recipe::build("excel_report", &fields(), Path::new("outputs"), "owned-id").unwrap();
        assert_eq!(
            recipe.artifact.unwrap(),
            Path::new("outputs").join("owned-id.xlsx")
        );
        let Operation::Excel { rows } = recipe.operation else {
            panic!()
        };
        assert_eq!(rows[1][1], "'=1+1");
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
