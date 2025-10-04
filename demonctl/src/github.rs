use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::{Client, Url};
use serde::Deserialize;

const DEFAULT_API_URL: &str = "https://api.github.com/";
const DEFAULT_REPOSITORY: &str = "afewell-hh/Demon";

#[derive(Debug, Clone)]
pub struct WorkflowRun {
    pub id: u64,
    pub run_number: Option<u64>,
    pub html_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Artifact {
    pub id: u64,
}

pub struct GithubActionsClient {
    client: Client,
    base_api: Url,
    owner: String,
    repo: String,
}

impl GithubActionsClient {
    pub fn new(
        token: String,
        repo_override: Option<&str>,
        api_url_override: Option<&str>,
    ) -> Result<Self> {
        let (owner, repo) = resolve_repository(repo_override)?;
        let base_api = resolve_api_url(api_url_override)?;

        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("demonctl-ghcr-digests"),
        );
        headers.insert(
            "X-GitHub-Api-Version",
            HeaderValue::from_static("2022-11-28"),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/vnd.github+json"),
        );

        let auth_header = format!("Bearer {}", token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_header).context("Invalid GH_TOKEN value")?,
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .context("Failed to construct GitHub client")?;

        Ok(Self {
            client,
            base_api,
            owner,
            repo,
        })
    }

    pub async fn latest_successful_run(&self, workflow: &str, branch: &str) -> Result<WorkflowRun> {
        let mut url = self.base_api.join(&format!(
            "repos/{}/{}/actions/workflows/{}/runs",
            self.owner, self.repo, workflow
        ))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("branch", branch);
            pairs.append_pair("status", "success");
            pairs.append_pair("per_page", "1");
        }

        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to query workflow runs")?
            .error_for_status()
            .context("GitHub returned an error for workflow runs request")?;

        let runs: WorkflowRunsResponse = response
            .json()
            .await
            .context("Failed to decode workflow runs response")?;

        let run = runs.workflow_runs.into_iter().next().ok_or_else(|| {
            anyhow!(
                "No successful runs for workflow '{}' on branch '{}'",
                workflow,
                branch
            )
        })?;

        Ok(WorkflowRun {
            id: run.id,
            run_number: run.run_number,
            html_url: run.html_url,
        })
    }

    pub async fn find_artifact(&self, run_id: u64, artifact_name: &str) -> Result<Artifact> {
        let url = self.base_api.join(&format!(
            "repos/{}/{}/actions/runs/{}/artifacts",
            self.owner, self.repo, run_id
        ))?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to query workflow artifacts")?
            .error_for_status()
            .context("GitHub returned an error for artifacts request")?;

        let artifacts: ArtifactsResponse = response
            .json()
            .await
            .context("Failed to decode artifacts response")?;

        let artifact = artifacts
            .artifacts
            .into_iter()
            .find(|artifact| artifact.name == artifact_name)
            .ok_or_else(|| {
                anyhow!(
                    "Artifact '{}' not found in workflow run {}",
                    artifact_name,
                    run_id
                )
            })?;

        Ok(Artifact { id: artifact.id })
    }

    pub async fn download_artifact(&self, artifact_id: u64) -> Result<Vec<u8>> {
        let url = self.base_api.join(&format!(
            "repos/{}/{}/actions/artifacts/{}/zip",
            self.owner, self.repo, artifact_id
        ))?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to download artifact archive")?
            .error_for_status()
            .context("GitHub returned an error while downloading artifact archive")?;

        let bytes = response
            .bytes()
            .await
            .context("Failed to read artifact archive bytes")?;

        Ok(bytes.to_vec())
    }
}

fn resolve_repository(override_value: Option<&str>) -> Result<(String, String)> {
    let repository = if let Some(value) = override_value {
        value.to_string()
    } else {
        std::env::var("DEMONCTL_GITHUB_REPOSITORY")
            .or_else(|_| std::env::var("GITHUB_REPOSITORY"))
            .unwrap_or_else(|_| DEFAULT_REPOSITORY.to_string())
    };

    let (owner, repo) = repository
        .split_once('/')
        .context("Repository must be in the form <owner>/<repo>")?;

    Ok((owner.to_string(), repo.to_string()))
}

fn resolve_api_url(override_value: Option<&str>) -> Result<Url> {
    let api_url = if let Some(value) = override_value {
        value.to_string()
    } else {
        std::env::var("DEMONCTL_GITHUB_API_URL")
            .or_else(|_| std::env::var("GITHUB_API_URL"))
            .unwrap_or_else(|_| DEFAULT_API_URL.to_string())
    };

    let mut base_api = Url::parse(&api_url).context("Invalid GitHub API URL")?;

    let current_path = base_api.path().to_string();

    if current_path.is_empty() || current_path == "/" {
        base_api.set_path("/");
    } else {
        let normalized_path = if current_path.ends_with('/') {
            current_path
        } else {
            format!("{}/", current_path.trim_end_matches('/'))
        };
        base_api.set_path(&normalized_path);
    }

    Ok(base_api)
}

#[derive(Debug, Deserialize)]
struct WorkflowRunsResponse {
    workflow_runs: Vec<WorkflowRunResponse>,
}

#[derive(Debug, Deserialize)]
struct WorkflowRunResponse {
    id: u64,
    run_number: Option<u64>,
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArtifactsResponse {
    artifacts: Vec<ArtifactResponse>,
}

#[derive(Debug, Deserialize)]
struct ArtifactResponse {
    id: u64,
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_repository_uses_override_when_provided() {
        let (owner, repo) = resolve_repository(Some("acme/widgets")).unwrap();
        assert_eq!(owner, "acme");
        assert_eq!(repo, "widgets");
    }

    #[test]
    fn resolve_repository_defaults_when_not_set() {
        let (owner, repo) = resolve_repository(None).unwrap();
        assert_eq!(owner, "afewell-hh");
        assert_eq!(repo, "Demon");
    }

    #[test]
    fn resolve_api_url_normalizes_trailing_path() {
        let url = resolve_api_url(Some("https://github.example.org/api/v3"))
            .unwrap()
            .to_string();
        assert_eq!(url, "https://github.example.org/api/v3/");
    }
}
