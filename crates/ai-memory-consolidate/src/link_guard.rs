//! Drops cross-scope links the model invented before pages are written.
//!
//! Prompt rules lower the rate of made-up `[[project:path]]` /
//! `[[_global:path]]` targets but cannot rule it out, and every unresolved one
//! becomes a `broken_link` lint finding. Resolution is no-create and fails
//! closed, and it matches the link indexer and lint exactly (workspace and
//! project by exact name), so guard, store and lint agree. A target in a
//! project the acting user cannot read counts as unresolved whether or not the
//! page exists, so the outcome is no existence oracle (#708).

use std::collections::{HashMap, HashSet};

use ai_memory_core::{LinkTarget, UserId, WorkspaceId};
use ai_memory_store::{LinkTargetKey, LinkTargetResolution, ReaderPool};
use ai_memory_wiki::{
    WritePageRequest, delink_cross_scope_wikilinks, extract_links, extract_relation_links,
};
use tracing::warn;

type TargetKey = (Option<String>, Option<String>, String);

fn key(t: &LinkTarget) -> TargetKey {
    (t.workspace.clone(), t.project.clone(), t.path.to_string())
}

/// `None` for a same-project link.
fn store_key(t: &LinkTarget) -> Option<LinkTargetKey> {
    Some((t.workspace.clone(), t.project.clone()?, t.path.to_string()))
}

fn cross_scope_targets(req: &WritePageRequest) -> Vec<LinkTarget> {
    let mut targets: Vec<LinkTarget> = extract_links(&req.body, &req.path);
    targets.extend(extract_relation_links(&req.frontmatter));
    targets.retain(|t| t.project.is_some());
    targets
}

/// Strip unresolved cross-scope links from the body and `relations:`
/// frontmatter of every request in one write. Same-scope links and links that
/// resolve to an existing page, or to a page another request of this write
/// creates, are left as written. `viewer` is the acting database user
/// (`None` skips the read check).
///
/// Store errors count as unresolved: an unverifiable link is not worth a
/// dangling row.
pub(crate) async fn strip_unresolved_cross_scope_links(
    reader: &ReaderPool,
    viewer: Option<UserId>,
    reqs: &mut [WritePageRequest],
) {
    let candidates: Vec<Vec<LinkTarget>> = reqs.iter().map(cross_scope_targets).collect();
    if candidates.iter().all(Vec::is_empty) {
        return;
    }

    let mut wanted: HashMap<WorkspaceId, HashSet<LinkTargetKey>> = HashMap::new();
    for (req, targets) in reqs.iter().zip(&candidates) {
        wanted
            .entry(req.workspace_id)
            .or_default()
            .extend(targets.iter().filter_map(store_key));
    }
    let mut resolved: HashMap<WorkspaceId, HashMap<LinkTargetKey, LinkTargetResolution>> =
        HashMap::new();
    for (workspace_id, keys) in wanted {
        let found = reader
            .resolve_link_targets(workspace_id, viewer, keys.into_iter().collect())
            .await
            .unwrap_or_default();
        resolved.insert(workspace_id, found);
    }

    // Pages this write creates exist by the time the links are indexed, so a
    // link to one is live under any spelling of its project.
    let created: HashSet<(WorkspaceId, ai_memory_core::ProjectId, &str)> = reqs
        .iter()
        .map(|r| (r.workspace_id, r.project_id, r.path.as_str()))
        .collect();
    let live_per_req: Vec<HashSet<TargetKey>> = reqs
        .iter()
        .zip(&candidates)
        .map(|(req, targets)| {
            targets
                .iter()
                .filter(|t| {
                    let hit = store_key(t).and_then(|k| resolved.get(&req.workspace_id)?.get(&k));
                    hit.is_some_and(|r| {
                        r.page_exists
                            || created.contains(&(r.workspace_id, r.project_id, t.path.as_str()))
                    })
                })
                .map(key)
                .collect()
        })
        .collect();

    for ((req, targets), live) in reqs.iter_mut().zip(&candidates).zip(&live_per_req) {
        let dropped = targets
            .iter()
            .map(key)
            .collect::<HashSet<_>>()
            .difference(live)
            .count();
        if dropped == 0 {
            continue;
        }
        warn!(
            path = %req.path.as_str(),
            dropped,
            "dropped cross-scope links that do not resolve to an existing page",
        );
        req.body = delink_cross_scope_wikilinks(&req.body, &req.path, |t| live.contains(&key(t)));
        retain_resolved_relations(&mut req.frontmatter, live);
    }
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

    use ai_memory_core::{NewUser, PagePath, ProjectId, Tier};
    use ai_memory_store::{AccessMode, GrantLevel, Store, TOKEN_HASH_LEN};
    use ai_memory_wiki::Wiki;

    fn request(ws: WorkspaceId, proj: ProjectId, path: &str, body: &str) -> WritePageRequest {
        WritePageRequest {
            workspace_id: ws,
            project_id: proj,
            path: PagePath::new(path).unwrap(),
            frontmatter: serde_json::json!({"title": path}),
            body: body.into(),
            tier: Tier::Semantic,
            pinned: false,
            title: None,
            admission_ctx: None,
            author_id: None,
            actor: ai_memory_core::ActorContext::anonymous(),
            evidence: Vec::new(),
        }
    }

    struct Fixture {
        store: Store,
        wiki: Wiki,
        ws: WorkspaceId,
        _tmp: tempfile::TempDir,
    }

    impl Fixture {
        async fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let store = Store::open(tmp.path()).unwrap();
            let ws = store
                .writer
                .get_or_create_workspace("default")
                .await
                .unwrap();
            let wiki = Wiki::new(tmp.path(), store.writer.clone()).unwrap();
            Self {
                store,
                wiki,
                ws,
                _tmp: tmp,
            }
        }

        async fn project_with_page(&self, name: &str, path: &str) -> ProjectId {
            let proj = self
                .store
                .writer
                .get_or_create_project(self.ws, name, None)
                .await
                .unwrap();
            self.wiki
                .write_page(request(self.ws, proj, path, "target"))
                .await
                .unwrap();
            proj
        }

        async fn user(&self, name: &str, byte: u8) -> UserId {
            self.store
                .writer
                .create_user(
                    NewUser {
                        username: name.to_owned(),
                        name: None,
                        email: None,
                    },
                    [byte; TOKEN_HASH_LEN],
                )
                .await
                .unwrap()
        }

        async fn guard(
            &self,
            viewer: Option<UserId>,
            from: ProjectId,
            body: &str,
        ) -> WritePageRequest {
            let mut reqs = [request(self.ws, from, "concepts/from.md", body)];
            strip_unresolved_cross_scope_links(&self.store.reader, viewer, &mut reqs).await;
            let [req] = reqs;
            req
        }
    }

    /// The guard matches projects by exact name like the link indexer and
    /// lint: a canonical or legacy-basename key is not a link target, even
    /// though scope lookup would accept it, and an exact name stays valid when
    /// another project's legacy key collides with it.
    #[tokio::test]
    async fn projects_match_by_exact_name_only() {
        let fx = Fixture::new().await;
        let own = fx.project_with_page("own", "x.md").await;
        let long = fx.project_with_page("acme-api", "notes/x.md").await;
        {
            let conn = rusqlite::Connection::open(fx.store.db_path()).unwrap();
            conn.execute(
                "UPDATE projects SET legacy_name = 'api', canonical_name = 'acme/api' \
                 WHERE id = ?1",
                rusqlite::params![long.as_bytes()],
            )
            .unwrap();
        }

        let out = fx
            .guard(
                None,
                own,
                "[[acme-api:notes/x.md]] [[api:notes/x.md]] [[acme/api:notes/x.md]]",
            )
            .await;
        assert_eq!(out.body, "[[acme-api:notes/x.md]] notes/x.md notes/x.md");

        // A second project now answers to `api` by exact name while
        // `acme-api` still answers to it by legacy key: scope lookup would
        // call that ambiguous, the indexer links to the exact match.
        fx.project_with_page("api", "notes/y.md").await;
        let out = fx
            .guard(None, own, "[[api:notes/y.md]] [[api:notes/x.md]]")
            .await;
        assert_eq!(out.body, "[[api:notes/y.md]] notes/x.md");
    }

    /// #708: a target in a project the acting user cannot read resolves the
    /// same whether or not the page exists, so the guard is no existence
    /// oracle. Legitimate controls: a granted reader and an open project.
    #[tokio::test]
    async fn unreadable_target_is_indistinguishable_from_a_missing_one() {
        let fx = Fixture::new().await;
        let own = fx
            .store
            .writer
            .get_or_create_project(fx.ws, "own", None)
            .await
            .unwrap();
        let open = fx.project_with_page("pub", "notes/p.md").await;
        fx.store
            .writer
            .set_new_project_mode(AccessMode::Restricted)
            .await
            .unwrap();
        let secret = fx.project_with_page("secret", "notes/s.md").await;
        let alice = fx.user("alice", 1).await;
        let bob = fx.user("bob", 2).await;
        fx.store
            .writer
            .grant_memory(alice, secret, GrantLevel::Read, None)
            .await
            .unwrap();
        let _ = open;

        let body = "[[secret:notes/s.md]] [[secret:notes/nope.md]] [[pub:notes/p.md]]";
        let bob_out = fx.guard(Some(bob), own, body).await;
        assert_eq!(bob_out.body, "notes/s.md notes/nope.md [[pub:notes/p.md]]");

        let alice_out = fx.guard(Some(alice), own, body).await;
        assert_eq!(
            alice_out.body,
            "[[secret:notes/s.md]] notes/nope.md [[pub:notes/p.md]]"
        );

        // No viewer (root / no database users) is not restricted.
        let root_out = fx.guard(None, own, body).await;
        assert_eq!(root_out.body, alice_out.body);
    }

    /// A page the same write creates is a live target under every spelling
    /// of its project; a page nobody creates is still dropped.
    #[tokio::test]
    async fn links_to_pages_created_in_the_same_write_are_kept() {
        let fx = Fixture::new().await;
        let own = fx
            .store
            .writer
            .get_or_create_project(fx.ws, "own", None)
            .await
            .unwrap();
        let mut reqs = [
            request(
                fx.ws,
                own,
                "concepts/a.md",
                "[[own:concepts/b.md]] [[default/own:concepts/b.md]] [[own:concepts/c.md]]",
            ),
            request(fx.ws, own, "concepts/b.md", "b"),
        ];
        strip_unresolved_cross_scope_links(&fx.store.reader, None, &mut reqs).await;
        assert_eq!(
            reqs[0].body,
            "[[own:concepts/b.md]] [[default/own:concepts/b.md]] concepts/c.md"
        );
    }
}
