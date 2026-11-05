#[cfg(test)]
use crate::adapters;
use crate::{model::*, storage::AppState, validation};
use chrono::Utc;
use uuid::Uuid;

pub fn prepare(task: &Task, parameters: &Parameters) -> Result<Run> {
    if task.enabled == Some(false) {
        return Err("Workflow is disabled".into());
    }
    validation::validate_task(task)?;
    validation::parameters(task, parameters)?;
    if serde_json::to_vec(parameters)
        .map_err(|e| e.to_string())?
        .len()
        > 65536
    {
        return Err("Combined answers exceed 64 KB".into());
    }
    Ok(Run {
        id: Uuid::new_v4().to_string(),
        task_id: task.id.clone(),
        title: task.title.clone(),
        status: "running".into(),
        started_at: Utc::now().to_rfc3339(),
        finished_at: None,
        parameters: parameters.clone(),
        logs: vec![
            "Validated workflow definition and all parameters.".into(),
            "Started local execution.".into(),
        ],
        artifact: None,
        error: None,
    })
}
pub fn insert(store: &mut Store, run: Run) -> Result<()> {
    if store.runs.iter().any(|r| r.status == "running") {
        return Err("The engine is busy. Wait for the current workflow to finish.".into());
    }
    store.runs.insert(0, run);
    store.runs.truncate(200);
    Ok(())
}
#[cfg(test)]
pub fn execute(state: &AppState, task: Task, run: Run, allow_direct_send: bool) -> Result<Run> {
    let result = if let Action::Foreground { recipe, .. } = &task.action {
        let fields = validation::resolve_fields(&task, &run.parameters)?;
        crate::foreground::execute(
            &state.foreground,
            &run.id,
            recipe,
            &fields,
            &state.output,
            allow_direct_send,
        )
    } else {
        adapters::execute(&task, &run.parameters, &state.output, &run.id)
    };
    record_result(state, &task, run, result)
}
pub fn record_result(
    state: &AppState,
    task: &Task,
    mut run: Run,
    result: Result<(String, Vec<String>)>,
) -> Result<Run> {
    match result {
        Ok((path, logs)) => {
            run.status = "succeeded".into();
            run.artifact = Some(path);
            run.logs.extend(logs);
            run.logs.push("Workflow completed successfully.".into());
        }
        Err(error) => {
            if matches!(task.action, Action::Foreground { .. }) {
                if let Some(progress) = state.foreground.snapshot().filter(|p| p.run_id == run.id) {
                    run.logs.extend(progress.logs);
                }
            }
            run.status = "failed".into();
            run.logs
                .push("Workflow failed. No automatic retries were attempted.".into());
            run.error = Some(error);
        }
    }
    run.finished_at = Some(Utc::now().to_rfc3339());
    state.transaction(|store| {
        let record = store
            .runs
            .iter_mut()
            .find(|r| r.id == run.id)
            .ok_or("Run record missing")?;
        *record = run.clone();
        Ok(())
    })?;
    Ok(run)
}
#[cfg(test)]
pub fn manual(
    state: &AppState,
    id: &str,
    parameters: Parameters,
    allow_direct_send: bool,
) -> Result<Run> {
    let task = state
        .load_config()?
        .tasks
        .into_iter()
        .find(|t| t.id == id)
        .ok_or("Workflow not found")?;
    let mut run = prepare(&task, &parameters)?;
    if matches!(task.action, Action::Foreground { .. })
        && validation::resolve_fields(&task, &parameters)?
            .get("delivery")
            .is_some_and(|s| s == "Send directly")
    {
        if !allow_direct_send {
            return Err("Direct sending requires explicit authorization for this run.".into());
        }
        run.logs
            .push("User explicitly authorized direct sending for this invocation.".into());
    }
    state.transaction(|store| insert(store, run.clone()))?;
    execute(state, task, run, allow_direct_send)
}
