use crate::model::*;
use chrono::DateTime;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

pub fn config(value: Value) -> Result<Config> {
    let schema: Value = serde_json::from_str(include_str!("../../config/tasks.schema.json"))
        .map_err(|e| e.to_string())?;
    let validator = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
    if let Err(error) = validator.validate(&value) {
        return Err(format!("Invalid configuration: {error}"));
    }
    let config: Config = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let mut ids = HashSet::new();
    for task in &config.tasks {
        if !ids.insert(&task.id) {
            return Err(format!("Duplicate task id: {}", task.id));
        }
        let mut step_ids = HashSet::new();
        for step in &task.steps {
            if !step_ids.insert(&step.id) {
                return Err(format!("Duplicate step id: {}", step.id));
            }
            if let Some(value) = &step.default {
                validate_step(step, value, false)?;
            }
        }
        if let Action::Export { fields, .. }
        | Action::Smtp { fields }
        | Action::Foreground { fields, .. } = &task.action
        {
            let required = match &task.action {
                Action::Smtp { .. } => vec!["title", "body", "recipient"],
                Action::Foreground { recipe, .. } if recipe == "web_email" => {
                    vec!["title", "body", "recipient", "provider", "delivery"]
                }
                Action::Export { format, .. } if format == "eml" => {
                    vec!["title", "body", "recipient"]
                }
                Action::Export { format, .. } if format == "ics" => vec!["title", "start"],
                _ => vec!["title", "body"],
            };
            for key in required {
                if !fields.contains_key(key) {
                    return Err(format!("{}: missing action field {key}", task.id));
                }
            }
            let parameters = task
                .steps
                .iter()
                .map(|s| (s.id.clone(), json!("")))
                .collect();
            for value in fields.values() {
                template(value, &parameters)?;
            }
        }
    }
    Ok(config)
}
pub fn validate_task(task: &Task) -> Result<()> {
    config(json!({ "schema_version": 1, "tasks": [task] })).map(|_| ())
}
pub fn parameters(task: &Task, parameters: &Parameters) -> Result<()> {
    if parameters
        .keys()
        .any(|key| !task.steps.iter().any(|step| step.id == *key))
    {
        return Err("Unknown parameter".into());
    }
    for step in &task.steps {
        let empty = if step.kind == "multi_select" {
            json!([])
        } else {
            json!("")
        };
        validate_step(
            step,
            parameters.get(&step.id).unwrap_or(&empty),
            step.required != Some(false),
        )?;
    }
    if let Action::Export { .. } | Action::Smtp { .. } = &task.action {
        let fields = resolve_fields(task, parameters)?;
        let format = match &task.action {
            Action::Export { format, .. } => format.as_str(),
            _ => "smtp",
        };
        if format == "ics" {
            let start = DateTime::parse_from_rfc3339(&fields["start"])
                .map_err(|_| "Invalid calendar start time")?;
            if let Some(end) = fields.get("end").filter(|s| !s.is_empty()) {
                let end =
                    DateTime::parse_from_rfc3339(end).map_err(|_| "Invalid calendar end time")?;
                if end <= start {
                    return Err("End time must be after start time".into());
                }
            }
            if let Some(minutes) = fields.get("minutes").filter(|s| !s.is_empty()) {
                let minutes: u32 = minutes.parse().map_err(|_| "Invalid reminder minutes")?;
                if minutes > 99999 {
                    return Err("Reminder minutes are too large".into());
                }
            }
        }
        if format == "eml" || format == "smtp" {
            let recipient = &fields["recipient"];
            if to_addresses(recipient).is_err() {
                return Err("Invalid email recipient".into());
            }
        }
    }
    if let Action::Foreground { recipe, .. } = &task.action {
        crate::foreground::validate_fields(recipe, &resolve_fields(task, parameters)?)?;
    }
    Ok(())
}
pub(crate) fn valid_email(value: &str) -> bool {
    let parts: Vec<_> = value.split('@').collect();
    parts.len() == 2
        && !parts[0].is_empty()
        && parts[1].contains('.')
        && !parts[1].starts_with('.')
        && !parts[1].ends_with('.')
        && !value
            .chars()
            .any(|c| c.is_whitespace() || "<>,;\"".contains(c))
}
fn validate_step(step: &Step, value: &Value, required: bool) -> Result<()> {
    let values: Vec<&str> = if step.kind == "multi_select" {
        value
            .as_array()
            .ok_or("Expected an array")?
            .iter()
            .map(|v| v.as_str().ok_or("Expected a string"))
            .collect::<std::result::Result<_, _>>()?
    } else {
        vec![value.as_str().ok_or("Expected a string")?]
    };
    if values.len() > 100 || values.iter().any(|v| v.chars().count() > 20000) {
        return Err("Answer is too long".into());
    }
    if required && values.iter().all(|v| v.trim().is_empty()) {
        return Err(format!("{}: answer required", step.prompt));
    }
    if step.kind == "multi_select"
        && (values.iter().any(|v| v.trim().is_empty())
            || values.iter().collect::<HashSet<_>>().len() != values.len())
    {
        return Err("Selections must be distinct and non-empty".into());
    }
    for value in values.into_iter().filter(|v| !v.trim().is_empty()) {
        if matches!(step.kind.as_str(), "select" | "multi_select")
            && step.allow_other != Some(true)
            && !step
                .options
                .as_ref()
                .is_some_and(|o| o.iter().any(|v| v == value))
        {
            return Err(format!("{}: unknown selection", step.prompt));
        }
        if step.kind == "email"
            && (if step.id == "recipient" {
                to_addresses(value).is_err()
            } else {
                !valid_email(value)
            })
        {
            return Err("Invalid email address".into());
        }
        if step.kind == "datetime" && DateTime::parse_from_rfc3339(value).is_err() {
            return Err("Invalid datetime; expected RFC3339 with timezone".into());
        }
        if value
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
        {
            return Err("Answer contains unsupported control characters".into());
        }
    }
    Ok(())
}
// One pass: a replacement containing braces is data and is never interpolated again.
pub fn template(input: &str, parameters: &Parameters) -> Result<String> {
    let mut result = String::new();
    let mut rest = input;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        let Some(end) = rest[start + 1..].find('}').map(|i| start + 1 + i) else {
            result.push_str(&rest[start..]);
            return Ok(result);
        };
        let key = &rest[start + 1..end];
        let is_key = key.starts_with(|c: char| c.is_ascii_lowercase())
            && key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if is_key {
            let value = parameters
                .get(key)
                .ok_or_else(|| format!("Unknown parameter {{{key}}}"))?;
            if let Some(text) = value.as_str() {
                result.push_str(text);
            } else if let Some(values) = value.as_array() {
                result.push_str(
                    &values
                        .iter()
                        .map(|v| v.as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            } else {
                return Err("Unexpected template value".into());
            }
        } else {
            result.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    Ok(result)
}
pub fn resolve_fields(task: &Task, parameters: &Parameters) -> Result<BTreeMap<String, String>> {
    let fields = match &task.action {
        Action::Export { fields, .. }
        | Action::Smtp { fields }
        | Action::Foreground { fields, .. } => fields,
        _ => return Ok(BTreeMap::new()),
    };
    // Missing optional steps still resolve to their empty typed value.
    let mut values = parameters.clone();
    for step in &task.steps {
        values.entry(step.id.clone()).or_insert_with(|| {
            if step.kind == "multi_select" {
                json!([])
            } else {
                json!("")
            }
        });
    }
    fields
        .iter()
        .map(|(k, v)| Ok((k.clone(), template(v, &values)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_config_is_valid() {
        config(serde_json::from_str(include_str!("../../config/tasks.json")).unwrap()).unwrap();
    }
    #[test]
    fn duplicate_ids_and_command_strings_rejected() {
        let mut value: Value =
            serde_json::from_str(include_str!("../../config/tasks.json")).unwrap();
        let duplicate = value["tasks"][0].clone();
        value["tasks"].as_array_mut().unwrap().push(duplicate);
        assert!(config(value).is_err());
        let mut value: Value =
            serde_json::from_str(include_str!("../../config/tasks.json")).unwrap();
        value["tasks"][0]["command"] = json!("sh deploy.sh {recipient}");
        assert!(config(value).is_err());
    }
    #[test]
    fn replacements_are_not_code_or_recursive() {
        let data = BTreeMap::from([("answer".into(), json!("$(Remove-Item *) {secret}"))]);
        assert_eq!(
            template("{answer}", &data).unwrap(),
            "$(Remove-Item *) {secret}"
        );
        assert!(template("{missing}", &data).is_err());
    }
    #[test]
    fn email_newline_injection_rejected() {
        assert!(!valid_email("a@b.com\r\nBcc: x@y.com"));
    }
}

/// Optional comma-separated bare email addresses; reject header injection.
pub(crate) fn cc_addresses(value: &str) -> Result<Vec<&str>> {
    optional_addresses(value, "CC")
}
pub(crate) fn bcc_addresses(value: &str) -> Result<Vec<&str>> {
    optional_addresses(value, "BCC")
}
fn optional_addresses<'a>(value: &'a str, field: &str) -> Result<Vec<&'a str>> {
    let error = || {
        format!("Enter valid {field} email addresses separated by commas (up to 100), or leave {field} blank.")
    };
    if value.chars().any(char::is_control) || value.len() > 8000 {
        return Err(error());
    }
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    let addresses: Vec<_> = value.split(',').map(str::trim).collect();
    if addresses.len() > 100 || addresses.iter().any(|a| !valid_email(a)) {
        return Err(error());
    }
    Ok(addresses)
}

/// Required To list: bounded, distinct and validated before MIME or SMTP submission.
pub(crate) fn to_addresses(value: &str) -> Result<Vec<&str>> {
    if value.len() > 8000 || value.chars().any(char::is_control) {
        return Err("Select 1–100 valid recipient addresses (up to 8,000 characters total)".into());
    }
    let addresses: Vec<_> = value.split(',').map(str::trim).collect();
    let mut seen = HashSet::new();
    if addresses.is_empty()
        || addresses.len() > 100
        || addresses
            .iter()
            .any(|v| v.len() > 254 || !valid_email(v) || !seen.insert(v.to_lowercase()))
    {
        return Err("Select 1–100 distinct, valid recipient email addresses".into());
    }
    Ok(addresses)
}
