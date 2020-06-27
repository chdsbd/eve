use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum GitHubError {
    JsonWebTokenCreation(jsonwebtoken::errors::Error),
    HttpError(reqwest::Error),
}

impl std::convert::From<reqwest::Error> for GitHubError {
    fn from(e: reqwest::Error) -> Self {
        Self::HttpError(e)
    }
}

impl std::convert::From<jsonwebtoken::errors::Error> for GitHubError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        Self::JsonWebTokenCreation(e)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Claim {
    /// Issued at
    iat: u64,
    /// Expiration time
    exp: u64,
    /// Issuer
    iss: String,
}

/// Create an authentication token to make application requests.
/// https://developer.github.com/apps/building-github-apps/authenticating-with-github-apps/#authenticating-as-a-github-app
/// This is different from authenticating as an installation
fn generate_jwt(
    private_key: &str,
    app_identifier: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now_unix_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("problem getting current time");
    let claim = Claim {
        iat: now_unix_time.as_secs(),
        exp: (now_unix_time + Duration::from_secs(10 * 60)).as_secs(),
        iss: app_identifier.to_owned(),
    };

    jsonwebtoken::encode(
        &Header::new(Algorithm::RS256),
        &claim,
        &EncodingKey::from_rsa_pem(private_key.as_ref())?,
    )
}

#[derive(Debug, Deserialize)]
struct GithubAccessToken {
    expires_at: String,
    permissions: Value,
    repository_selection: String,
    token: String,
}

struct CreateAccessTokenForInstall<'a> {
    install_id: &'a str,
    jwt: &'a str,
}

/// https://developer.github.com/v3/apps/#create-an-installation-access-token-for-an-app
fn create_access_token_for_install(
    params: CreateAccessTokenForInstall,
) -> Result<GithubAccessToken, GitHubError> {
    let res = reqwest::blocking::Client::new()
        .post(&format!(
            "https://api.github.com/app/installations/{install_id}/access_tokens",
            install_id = params.install_id
        ))
        .header("User-Agent", "chdsbd/heroku-deploy-notifier")
        .header(AUTHORIZATION, format!("Bearer {}", params.jwt))
        .header(ACCEPT, "application/vnd.github.machine-man-preview+json")
        .send()?;
    res.error_for_status_ref()?;

    Ok(res.json::<GithubAccessToken>()?)
}

#[derive(Deserialize, Debug)]
pub struct Actor {
    pub login: String,
    pub id: i64,
    pub html_url: String,
    pub avatar_url: String,
}
#[derive(Deserialize, Debug)]
pub struct CommitAuthor {
    pub date: String,
}
#[derive(Deserialize, Debug)]
pub struct Commit {
    pub message: String,
    pub url: String,
    pub author: CommitAuthor,
}

#[derive(Deserialize, Debug)]
pub struct CommitNode {
    pub sha: String,
    pub commit: Commit,
    pub author: Actor,
    pub html_url: String,
}

#[derive(Deserialize, Debug)]
pub struct CommitComparison {
    pub url: String,
    pub html_url: String,
    pub permalink_url: String,
    pub commits: Vec<CommitNode>,
}

pub struct Compare<'a> {
    pub private_key: &'a str,
    pub app_id: &'a str,
    pub install_id: &'a str,
    pub org: &'a str,
    pub repo: &'a str,
    pub base: &'a str,
    pub head: &'a str,
}

pub fn compare(params: Compare) -> Result<CommitComparison, GitHubError> {
    // https://developer.github.com/v3/repos/commits/#compare-two-commits
    // https://api.github.com/repos/chdsbd/kodiak/compare/7c68a71a87d12cc2404aed192840674af84f3df4...master
    let github_compare_url = format!(
        "https://api.github.com/repos/{org}/{repo}/compare/{base}...{head}",
        org = params.org,
        repo = params.repo,
        base = params.base,
        head = params.head
    );

    let jwt = generate_jwt(params.private_key, params.app_id)?;
    let access_token = create_access_token_for_install(CreateAccessTokenForInstall {
        jwt: &jwt,
        install_id: params.install_id,
    })?;

    let client = reqwest::blocking::Client::builder()
        .user_agent("chdsbd/heroku-deploy-notifier")
        .build()?;
    let res = client
        .get(&github_compare_url)
        .header("Authorization", format!("Bearer {}", access_token.token))
        .send()?;

    res.error_for_status_ref()?;
    Ok(res.json::<CommitComparison>()?)
}
