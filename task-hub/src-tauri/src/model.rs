use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type Parameters = BTreeMap<String, Value>;
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub schema_version: u32,
    pub tasks: Vec<Task>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub icon: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    pub steps: Vec<Step>,
    pub action: Action,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub prompt: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_other: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    Export {
        format: String,
        fields: BTreeMap<String, String>,
    },
    Smtp {
        fields: BTreeMap<String, String>,
    },
    Foreground {
        recipe: String,
        fields: BTreeMap<String, String>,
    },
    Script {
        script_id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub task_id: String,
    pub title: String,
    pub status: String,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    pub parameters: Parameters,
    pub logs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub task: Task,
    pub parameters: Parameters,
    pub next_run: String,
    pub repeat: String,
    pub enabled: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub recipient_groups: Vec<crate::recipient_groups::Group>,
    #[serde(default)]
    pub recipients: Vec<crate::recipients::Contact>,
    #[serde(default)]
    pub calendar_links: Vec<crate::gmail::calendar::CalendarLink>,
    pub runs: Vec<Run>,
    pub schedules: Vec<Schedule>,
    #[serde(default)]
    pub email_routines: Vec<crate::email_routines::EmailRoutine>,
    #[serde(default)]
    pub native_notification_version: u32,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub calendar_links: Vec<crate::gmail::calendar::CalendarLink>,
    pub runs: Vec<Run>,
    pub schedules: Vec<Schedule>,
    pub output_dir: String,
    pub foreground: Option<crate::foreground::Progress>,
    pub email_routines: Vec<crate::email_routines::EmailRoutine>,
}
