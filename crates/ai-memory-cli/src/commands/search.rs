//! `ai-memory search` — run an FTS5 query against the wiki index.
//!
//! Thin HTTP client. Calls `GET /admin/search?q=…&limit=…`, scoped to one
//! project or, with `--global`, to none; renders the hits as human text or
//! JSON. Never opens the store directly.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::cli::SearchArgs;
use crate::config::Config;
use crate::http_client::{ServerEndpoint, get_json};

/// FTS5 hit, mirrors the `ai_memory_store::PageHit` fields used here. A
/// global search also names the hit's workspace and project
/// (`PageHitWithMeta`); a scoped one leaves them out, as before.
#[derive(Debug, Deserialize, Serialize)]
struct Hit {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workspace_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_name: Option<String>,
    path: String,
    title: String,
    snippet: String,
    rank: f64,
}

/// Run the `search` subcommand.
///
/// # Errors
/// Returns an error if the server is unreachable or returns non-2xx.
pub async fn run(config: &Config, args: SearchArgs) -> Result<()> {
    let ep = ServerEndpoint::from_config_resolving_auth(config).await;
    // A global search resolves no scope, so the marker walk and its stderr
    // notice are skipped and the current directory changes nothing.
    let scope = if args.global {
        None
    } else {
        Some(super::resolve_scope(
            config,
            args.workspace.as_deref(),
            args.project.as_deref(),
        )?)
    };
    let limit_str = args.limit.to_string();
    let mut params = vec![("q", args.query.as_str())];
    if let Some((workspace, project)) = &scope {
        params.push(("workspace", workspace.as_str()));
        params.push(("project", project.as_str()));
    }
    params.push(("limit", limit_str.as_str()));
    let hits: Vec<Hit> = get_json(&ep, "/admin/search", &params).await?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&hits)?);
    } else {
        print!("{}", render(&args.query, &hits));
    }
    Ok(())
}

fn render(query: &str, hits: &[Hit]) -> String {
    if hits.is_empty() {
        return format!("no results for {query:?}\n");
    }
    let mut out = format!("{} result(s) for {query:?}:\n", hits.len());
    for hit in hits {
        let location = match (&hit.workspace_name, &hit.project_name) {
            (Some(workspace), Some(project)) => format!("{workspace}/{project}  {}", hit.path),
            _ => hit.path.clone(),
        };
        out.push_str(&format!("  {location}  rank={:.4}\n", hit.rank));
        out.push_str(&format!("    {}\n", hit.title));
        out.push_str(&format!("    {}\n", hit.snippet));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(scope: Option<(&str, &str)>, path: &str) -> Hit {
        Hit {
            workspace_name: scope.map(|(w, _)| w.to_string()),
            project_name: scope.map(|(_, p)| p.to_string()),
            path: path.to_string(),
            title: "Title".to_string(),
            snippet: "snippet".to_string(),
            rank: -1.5,
        }
    }

    #[test]
    fn global_hits_name_their_workspace_and_project() {
        let out = render(
            "needle",
            &[
                hit(Some(("default", "api")), "notes/a.md"),
                hit(Some(("acme", "web")), "notes/b.md"),
            ],
        );
        assert!(out.starts_with("2 result(s) for \"needle\":\n"), "{out}");
        assert!(
            out.contains("  default/api  notes/a.md  rank=-1.5000\n"),
            "{out}"
        );
        assert!(
            out.contains("  acme/web  notes/b.md  rank=-1.5000\n"),
            "{out}"
        );
    }

    #[test]
    fn scoped_hits_render_as_before() {
        let out = render("needle", &[hit(None, "notes/a.md")]);
        assert_eq!(
            out,
            "1 result(s) for \"needle\":\n  notes/a.md  rank=-1.5000\n    Title\n    snippet\n"
        );
        assert_eq!(render("needle", &[]), "no results for \"needle\"\n");
    }

    #[test]
    fn scoped_json_keeps_its_fields() {
        let json = serde_json::to_value(hit(None, "notes/a.md")).unwrap();
        assert!(json.get("workspace_name").is_none(), "{json}");
        assert!(json.get("project_name").is_none(), "{json}");
        let json = serde_json::to_value(hit(Some(("default", "api")), "notes/a.md")).unwrap();
        assert_eq!(json["workspace_name"], "default");
        assert_eq!(json["project_name"], "api");
    }
}
