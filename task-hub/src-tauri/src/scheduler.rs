use crate::{model::*, runner, storage::AppState};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

pub fn create(
    state: &AppState,
    task_id: &str,
    parameters: Parameters,
    next_run: String,
    repeat: String,
) -> Result<()> {
    if !["once", "daily", "weekly"].contains(&repeat.as_str()) {
        return Err("Invalid recurrence".into());
    }
    let when = DateTime::parse_from_rfc3339(&next_run).map_err(|_| "Invalid schedule timestamp")?;
    if when <= Utc::now() {
        return Err("Schedule time must be in the future".into());
    }
    let task = state
        .load_config()?
        .tasks
        .into_iter()
        .find(|t| t.id == task_id)
        .ok_or("Workflow not found")?;
    runner::prepare(&task, &parameters)?;
    if matches!(task.action, Action::Foreground { .. }) {
        return Err(
            "Foreground automation must be started interactively; it cannot be scheduled.".into(),
        );
    }
    state.transaction(|store| {
        if store.schedules.len() >= 50 {
            return Err("Schedule limit reached (50). Remove completed schedules first.".into());
        }
        store.schedules.push(Schedule {
            id: Uuid::new_v4().to_string(),
            task,
            parameters,
            next_run: when.with_timezone(&Utc).to_rfc3339(),
            repeat,
            enabled: true,
        });
        Ok(())
    })
}
pub fn next_time(
    current: DateTime<Utc>,
    now: DateTime<Utc>,
    repeat: &str,
) -> Option<DateTime<Utc>> {
    let seconds = match repeat {
        "daily" => 86400,
        "weekly" => 604800,
        _ => return None,
    };
    let elapsed = now.signed_duration_since(current).num_seconds().max(0);
    current.checked_add_signed(Duration::seconds((elapsed / seconds + 1) * seconds))
}
pub fn tick(state: &AppState) -> Result<()> {
    let now = Utc::now();
    // Claim a due schedule and persist its running record in one atomic transaction.
    let claimed = state.transaction(|store| {
        if store.runs.iter().any(|r| r.status == "running") {
            return Ok(None);
        }
        let Some(index) = store.schedules.iter().position(|s| {
            s.enabled && DateTime::parse_from_rfc3339(&s.next_run).is_ok_and(|date| date <= now)
        }) else {
            return Ok(None);
        };
        let schedule = store.schedules[index].clone();
        if matches!(schedule.task.action, Action::Foreground { .. }) {
            store.schedules[index].enabled = false;
            return Ok(None);
        }
        let run = runner::prepare(&schedule.task, &schedule.parameters)?;
        let due = DateTime::parse_from_rfc3339(&schedule.next_run)
            .map_err(|e| e.to_string())?
            .with_timezone(&Utc);
        if let Some(next) = next_time(due, now, &schedule.repeat) {
            store.schedules[index].next_run = next.to_rfc3339();
        } else {
            store.schedules[index].enabled = false;
        }
        runner::insert(store, run.clone())?;
        Ok(Some((schedule.task, run)))
    })?;
    if let Some((task, run)) = claimed {
        runner::execute(state, task, run, false)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreground_recipes_cannot_be_scheduled_or_replayed_from_storage() {
        let root = tempfile::tempdir().unwrap();
        let state = AppState::initialize(root.path().into()).unwrap();
        state
            .save_config(
                serde_json::from_str(include_str!("../../config/examples/library.tasks.json"))
                    .unwrap(),
            )
            .unwrap();
        let parameters: Parameters =
            serde_json::from_value(serde_json::json!({"provider":"Gmail","browser":"Google Chrome","recipient":"test@example.com","subject":"Test","message":"Test body","delivery":"Draft"}))
                .unwrap();
        assert!(create(
            &state,
            "foreground_email",
            parameters.clone(),
            (Utc::now() + Duration::hours(1)).to_rfc3339(),
            "once".into()
        )
        .unwrap_err()
        .contains("interactively"));
        let task = state
            .load_config()
            .unwrap()
            .tasks
            .into_iter()
            .find(|t| t.id == "foreground_email")
            .unwrap();
        state
            .transaction(|store| {
                store.schedules.push(Schedule {
                    id: "unattended".into(),
                    task,
                    parameters,
                    next_run: (Utc::now() - Duration::minutes(1)).to_rfc3339(),
                    repeat: "once".into(),
                    enabled: true,
                });
                Ok(())
            })
            .unwrap();
        tick(&state).unwrap();
        assert!(!state.lock().unwrap().schedules[0].enabled);
        assert!(state.lock().unwrap().runs.is_empty());
    }
    #[test]
    fn missed_intervals_are_coalesced_and_cadence_is_preserved() {
        let start = "2026-01-01T09:00:00Z".parse::<DateTime<Utc>>().unwrap();
        let now = "2026-01-04T12:00:00Z".parse::<DateTime<Utc>>().unwrap();
        assert_eq!(
            next_time(start, now, "daily").unwrap().to_rfc3339(),
            "2026-01-05T09:00:00+00:00"
        );
        assert!(next_time(start, now, "once").is_none());
    }
    #[test]
    fn one_time_schedule_executes_exactly_once() {
        let root = tempfile::tempdir().unwrap();
        let state = AppState::initialize(root.path().into()).unwrap();
        state
            .save_config(
                serde_json::from_str(include_str!("../../config/examples/library.tasks.json"))
                    .unwrap(),
            )
            .unwrap();
        let config = state.load_config().unwrap();
        let task = config
            .tasks
            .into_iter()
            .find(|t| t.id == "draft_email")
            .unwrap();
        let parameters = serde_json::from_value(
            serde_json::json!({"recipient":"a@example.com","subject":"Hi","message":"Hello"}),
        )
        .unwrap();
        state
            .transaction(|store| {
                store.schedules.push(Schedule {
                    id: "test".into(),
                    task,
                    parameters,
                    next_run: (Utc::now() - Duration::minutes(1)).to_rfc3339(),
                    repeat: "once".into(),
                    enabled: true,
                });
                Ok(())
            })
            .unwrap();
        tick(&state).unwrap();
        tick(&state).unwrap();
        let store = state.lock().unwrap();
        assert_eq!(store.runs.len(), 1);
        assert_eq!(store.runs[0].status, "succeeded");
        assert!(!store.schedules[0].enabled);
    }
}
