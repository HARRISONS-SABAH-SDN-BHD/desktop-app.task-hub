//! Filename-only suggestions and deterministic period rendering. No file contents or AI service.
use crate::model::Result;
use chrono::{Datelike, NaiveDate};
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize)]
pub struct Suggestion {
    pub pattern: String,
    pub count: usize,
    pub samples: Vec<String>,
}
pub fn render(pattern: &str, day: NaiveDate, previous: bool) -> Result<String> {
    if pattern.is_empty()
        || pattern.len() > 600
        || pattern
            .chars()
            .any(|c| c.is_control() || "/\\:<>\"|".contains(c))
    {
        return Err("Use a filename pattern only, without folders or Windows-invalid characters (up to 600 bytes).".into());
    }
    let day = if previous {
        day.with_day(1)
            .unwrap()
            .pred_opt()
            .ok_or("Date out of range")?
    } else {
        day
    };
    let tokens = Regex::new(r"\{\{\s*([^{}]+?)\s*\}\}").unwrap();
    let mut error = None;
    let output = tokens
        .replace_all(pattern, |capture: &regex::Captures| {
            let key = capture[1].trim();
            let value = match key {
                "date" => format!("{}-{}-{}", day.day(), day.month(), day.year()),
                "iso_date" => day.format("%Y-%m-%d").to_string(),
                "year" => day.format("%Y").to_string(),
                "yy" => day.format("%y").to_string(),
                "month" => day.format("%B").to_string(),
                "MONTH" => day.format("%B").to_string().to_uppercase(),
                "month_lower" => day.format("%B").to_string().to_lowercase(),
                "month_short" => day.format("%b").to_string(),
                "MONTH_SHORT" => day.format("%b").to_string().to_uppercase(),
                "month_short_lower" => day.format("%b").to_string().to_lowercase(),
                "mm" => day.format("%m").to_string(),
                "month_number" => day.month().to_string(),
                "dd" => day.format("%d").to_string(),
                "day" => day.day().to_string(),
                _ => {
                    error = Some(format!("Unknown filename field: {key}"));
                    String::new()
                }
            };
            value
        })
        .into_owned();
    if let Some(error) = error {
        return Err(error);
    }
    if output.contains(['{', '}']) {
        return Err("Unclosed or unsupported filename field".into());
    }
    if output.starts_with('.')
        || output.starts_with("~$")
        || output.ends_with(['.', ' '])
        || output.len() > 1024
    {
        return Err("Use a regular report filename".into());
    }
    let base = output
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_uppercase();
    if [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ]
    .contains(&base.as_str())
    {
        return Err("Reserved Windows report filename".into());
    }
    Ok(output)
}
pub fn matches(pattern: &str, name: &str) -> Result<bool> {
    let mut expression = String::from("(?i)^");
    for ch in pattern.chars() {
        match ch {
            '*' => expression.push_str(".*"),
            '?' => expression.push('.'),
            _ => expression.push_str(&regex::escape(&ch.to_string())),
        }
    }
    expression.push('$');
    Ok(Regex::new(&expression)
        .map_err(|_| "Invalid filename pattern")?
        .is_match(name))
}
pub fn infer(names: &[String]) -> Vec<Suggestion> {
    let iso=Regex::new(r"(?P<y>(?:19|20)\d{2})(?P<a>[-_.])(?P<m>0?[1-9]|1[0-2])(?P<b>[-_.])(?P<d>3[01]|[12]\d|0?[1-9])").unwrap();
    let dmy=Regex::new(r"(?P<d>3[01]|[12]\d|0?[1-9])(?P<a>[-_.])(?P<m>0?[1-9]|1[0-2])(?P<b>[-_.])(?P<y>(?:19|20)\d{2})").unwrap();
    let year = Regex::new(r"(?:19|20)\d{2}").unwrap();
    let month=Regex::new(r"(?i)(^|[^a-z])(january|february|march|april|may|june|july|august|september|october|november|december|jan|feb|mar|apr|jun|jul|aug|sep|oct|nov|dec)([^a-z]|$)").unwrap();
    let mut groups: BTreeMap<String, Suggestion> = BTreeMap::new();
    for name in names {
        let mut pattern = name.clone();
        for (re, is_iso) in [(&iso, true), (&dmy, false)] {
            if let Some(c) = re.captures(name) {
                if c["a"] != c["b"]
                    || NaiveDate::from_ymd_opt(
                        c["y"].parse().unwrap(),
                        c["m"].parse().unwrap(),
                        c["d"].parse().unwrap(),
                    )
                    .is_none()
                {
                    continue;
                }
                let m = if is_iso || c["m"].starts_with('0') {
                    "{{mm}}"
                } else {
                    "{{month_number}}"
                };
                let d = if is_iso || c["d"].starts_with('0') {
                    "{{dd}}"
                } else {
                    "{{day}}"
                };
                let replacement = if is_iso {
                    format!("{{{{year}}}}{}{m}{}{d}", &c["a"], &c["b"])
                } else {
                    format!("{d}{}{m}{}{{{{year}}}}", &c["a"], &c["b"])
                };
                let range = c.get(0).unwrap();
                pattern.replace_range(range.start()..range.end(), &replacement);
                break;
            }
        }
        pattern = year.replace_all(&pattern, "{{year}}").into_owned();
        pattern = month
            .replace_all(&pattern, |c: &regex::Captures| {
                let value = &c[2];
                let short = value.len() == 3;
                let token = if value == value.to_uppercase() {
                    if short {
                        "MONTH_SHORT"
                    } else {
                        "MONTH"
                    }
                } else if value == value.to_lowercase() {
                    if short {
                        "month_short_lower"
                    } else {
                        "month_lower"
                    }
                } else if short {
                    "month_short"
                } else {
                    "month"
                };
                format!("{}{{{{{token}}}}}{}", &c[1], &c[3])
            })
            .into_owned();
        if pattern == *name || !pattern.contains("{{") {
            continue;
        }
        let item = groups.entry(pattern.clone()).or_insert(Suggestion {
            pattern,
            count: 0,
            samples: vec![],
        });
        item.count += 1;
        if item.samples.len() < 3 {
            item.samples.push(name.clone());
        }
    }
    let mut values: Vec<_> = groups.into_values().collect();
    values.sort_by(|a, b| b.count.cmp(&a.count).then(a.pattern.cmp(&b.pattern)));
    values.truncate(12);
    values
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suggestions_use_filename_evidence_and_render_dates_and_months() {
        let names = vec![
            "4-11-2026 report.xlsx".into(),
            "5-11-2026 report.xlsx".into(),
            "NOV report 2026.pdf".into(),
            "OCT report 2026.pdf".into(),
        ];
        let suggestions = infer(&names);
        assert!(suggestions
            .iter()
            .any(|s| s.count == 2 && s.pattern == "{{day}}-{{month_number}}-{{year}} report.xlsx"));
        let month = suggestions
            .iter()
            .find(|s| s.pattern.contains("MONTH_SHORT"))
            .unwrap();
        assert_eq!(month.count, 2);
        let day = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        assert_eq!(
            render(&month.pattern, day, false).unwrap(),
            "NOV report 2026.pdf"
        );
        assert_eq!(
            render("{{day}}-{{mm}}-{{year}} report.xlsx", day, false).unwrap(),
            "5-11-2026 report.xlsx"
        );
        assert_eq!(
            render(
                "{{MONTH_SHORT}} {{year}}.xlsx",
                NaiveDate::from_ymd_opt(2027, 1, 1).unwrap(),
                true
            )
            .unwrap(),
            "DEC 2026.xlsx"
        );
    }
    #[test]
    fn patterns_cannot_escape_a_folder_and_matching_is_whole_filename() {
        let day = NaiveDate::from_ymd_opt(2026, 11, 5).unwrap();
        for pattern in [
            "../report.xlsx",
            "C:\\report.xlsx",
            "{{unknown}}.pdf",
            "{{month.pdf",
        ] {
            assert!(render(pattern, day, false).is_err());
        }
        assert!(matches("NOV*.xlsx", "nov report.xlsx").unwrap());
        assert!(!matches("NOV*.xlsx", "OLD NOV report.xlsx").unwrap());
    }
}
