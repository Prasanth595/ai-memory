//! Drops cross-scope links the model invented before a page is written.
//!
//! Prompt rules lower the rate of made-up `[[project:path]]` /
//! `[[_global:path]]` targets but cannot rule it out, and every unresolved one
//! becomes a `broken_link` lint finding. Resolution here is no-create and fails
//! closed: a target whose workspace, project or page cannot be found is
//! de-linked, never created.

use std::collections::HashSet;

use ai_memory_core::LinkTarget;
use ai_memory_store::{ReaderPool, lookup_existing_scope};
use ai_memory_wiki::{
    WritePageRequest, delink_cross_scope_wikilinks, extract_links, extract_relation_links,
};
use tracing::warn;

type TargetKey = (Option<String>, Option<String>, String);

fn key(t: &LinkTarget) -> TargetKey {
    (t.workspace.clone(), t.project.clone(), t.path.to_string())
}

/// Whether the latest version of `target` exists. Store errors count as
/// unresolved: an unverifiable link is not worth a dangling row.
async fn resolves(
    reader: &ReaderPool,
    own_workspace_name: Option<&str>,
    target: &LinkTarget,
) -> bool {
    let (Some(project), path) = (target.project.as_deref(), target.path.as_str()) else {
        return false;
    };
    let workspace_name = match (target.workspace.as_deref(), own_workspace_name) {
        (Some(name), _) | (None, Some(name)) => name,
        (None, None) => return false,
    };
    let Ok(scope) = lookup_existing_scope(reader, workspace_name, project).await else {
        return false;
    };
    matches!(
        reader
            .latest_page_id_by_ids(scope.workspace_id, scope.project_id, path.to_owned())
            .await,
        Ok(Some(_))
    )
}

/// Strip unresolved cross-scope links from `req`'s body and `relations:`
/// frontmatter. Same-scope and resolving links are left as written.
pub(crate) async fn strip_unresolved_cross_scope_links(
    reader: &ReaderPool,
    req: &mut WritePageRequest,
) {
    let mut candidates: Vec<LinkTarget> = extract_links(&req.body, &req.path);
    candidates.extend(extract_relation_links(&req.frontmatter));
    candidates.retain(|t| t.project.is_some());
    if candidates.is_empty() {
        return;
    }

    let own_name = reader
        .workspace_name_by_id(req.workspace_id)
        .await
        .ok()
        .flatten();
    let mut seen = HashSet::new();
    let mut live: HashSet<TargetKey> = HashSet::new();
    let mut dropped = 0usize;
    for target in &candidates {
        if !seen.insert(key(target)) {
            continue;
        }
        if resolves(reader, own_name.as_deref(), target).await {
            live.insert(key(target));
        } else {
            dropped += 1;
        }
    }
    if dropped == 0 {
        return;
    }
    warn!(
        path = %req.path.as_str(),
        dropped,
        "dropped cross-scope links that do not resolve to an existing page",
    );

    req.body = delink_cross_scope_wikilinks(&req.body, &req.path, |t| live.contains(&key(t)));
    retain_resolved_relations(&mut req.frontmatter, &live);
}

/// Remove unresolved cross-scope entries from `relations:`; a relation left
/// empty is removed, and so is an empty `relations` map.
fn retain_resolved_relations(frontmatter: &mut serde_json::Value, live: &HashSet<TargetKey>) {
    let Some(relations) = frontmatter
        .get_mut("relations")
        .and_then(|v| v.as_object_mut())
    else {
        return;
    };
    for targets in relations.values_mut() {
        let Some(list) = targets.as_array_mut() else {
            continue;
        };
        list.retain(|v| {
            let Some(raw) = v.as_str() else {
                return true;
            };
            let probe = serde_json::json!({ "relations": { "causes": [raw] } });
            match extract_relation_links(&probe).first() {
                Some(t) if t.project.is_some() => live.contains(&key(t)),
                _ => true,
            }
        });
    }
    relations.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
    if relations.is_empty()
        && let Some(map) = frontmatter.as_object_mut()
    {
        map.remove("relations");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_relation_is_removed_and_local_kept() {
        let mut fm = serde_json::json!({
            "relations": {
                "causes": ["other:gone.md", "local/note.md"],
                "fixes": ["other:gone2.md"],
            }
        });
        retain_resolved_relations(&mut fm, &HashSet::new());
        assert_eq!(
            fm["relations"]["causes"],
            serde_json::json!(["local/note.md"])
        );
        assert!(fm["relations"].get("fixes").is_none());

        let mut only = serde_json::json!({"title": "t", "relations": {"fixes": ["x:y.md"]}});
        retain_resolved_relations(&mut only, &HashSet::new());
        assert!(only.get("relations").is_none());
        assert_eq!(only["title"], "t");
    }
}
