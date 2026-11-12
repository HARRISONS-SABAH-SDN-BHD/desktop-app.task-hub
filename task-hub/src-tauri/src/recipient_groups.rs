use crate::{model::Result, storage::AppState, validation};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub members: Vec<String>,
}
pub fn list(state: &AppState) -> Result<Vec<Group>> {
    Ok(state.lock()?.recipient_groups.clone())
}
pub fn save(
    state: &AppState,
    id: Option<String>,
    name: String,
    members: Vec<String>,
) -> Result<()> {
    let name = name.trim().to_owned();
    if name.is_empty() || name.chars().count() > 100 || name.chars().any(char::is_control) {
        return Err("Enter a group name of 1–100 characters".into());
    }
    let mut members: Vec<String> = members
        .into_iter()
        .map(|s| s.trim().to_lowercase())
        .collect();
    members.sort();
    members.dedup();
    validation::to_addresses(&members.join(", "))?;
    state.transaction(|store| {
        let prior = id
            .as_ref()
            .and_then(|id| store.recipient_groups.iter().find(|g| g.id == *id));
        if id.is_some() && prior.is_none() {
            return Err("Group no longer exists. Refresh the recipient list.".into());
        }
        if members.iter().any(|m| {
            !store
                .recipients
                .iter()
                .any(|c| c.email.eq_ignore_ascii_case(m))
                && !prior.is_some_and(|g| g.members.contains(m))
        }) {
            return Err("Select group members from the saved recipient list".into());
        }
        if store
            .recipient_groups
            .iter()
            .any(|g| Some(&g.id) != id.as_ref() && g.name.eq_ignore_ascii_case(&name))
        {
            return Err("A group with this name already exists".into());
        }
        let group = Group {
            id: id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name,
            members,
        };
        if let Some(existing) = store.recipient_groups.iter_mut().find(|g| g.id == group.id) {
            *existing = group;
        } else {
            if store.recipient_groups.len() >= 1000 {
                return Err("At most 1,000 recipient groups are supported".into());
            }
            store.recipient_groups.push(group);
        }
        Ok(())
    })
}
pub fn remove(state: &AppState, id: String) -> Result<()> {
    state.transaction(|s| {
        s.recipient_groups.retain(|g| g.id != id);
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn groups_validate_deduplicate_and_persist() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::initialize(dir.path().into()).unwrap();
        crate::recipients::save(&state, "a@example.com".into(), "A".into()).unwrap();
        save(
            &state,
            None,
            "Team".into(),
            vec!["A@example.com".into(), "a@example.com".into()],
        )
        .unwrap();
        assert_eq!(list(&state).unwrap()[0].members.len(), 1);
        assert!(save(&state, None, "Team".into(), vec!["a@example.com".into()]).is_err());
        assert!(save(&state, None, "Empty".into(), vec![]).is_err());
        assert!(save(
            &state,
            None,
            "Other".into(),
            vec!["other@example.com".into()]
        )
        .is_err());
        drop(state);
        let state = AppState::initialize(dir.path().into()).unwrap();
        let group = list(&state).unwrap().remove(0);
        save(
            &state,
            Some(group.id.clone()),
            "Renamed".into(),
            group.members,
        )
        .unwrap();
        remove(&state, group.id).unwrap();
        assert!(list(&state).unwrap().is_empty());
    }
}
