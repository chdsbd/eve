#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use chrono::DateTime;
use chrono_humanize;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use rocket::request::Form;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rocket::fairing::AdHoc;
use rocket::response::NamedFile;
use rocket::State;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use structopt::StructOpt;

#[derive(Debug)]
struct AppConfig {
    heroku_app_name: String,
    github_org_name: String,
    github_repo_name: String,
    github_app_id: String,
    github_app_private_key: String,
    github_app_install_id: String,
    github_id_to_slack_id: HashMap<i64, String>,
    slack_oauth_token: String,
}

#[get("/")]
fn root() -> &'static str {
    "Heroku Deploy Notifier"
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

    println!("{:#?}", claim);
    println!("{:#?}", private_key);

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

struct CreateAccessTokenForInstall {
    install_id: String,
    jwt: String,
}

/// https://developer.github.com/v3/apps/#create-an-installation-access-token-for-an-app
fn create_access_token_for_install(params: CreateAccessTokenForInstall) -> GithubAccessToken {
    let res = reqwest::blocking::Client::new()
        .post(&format!(
            "https://api.github.com/app/installations/{install_id}/access_tokens",
            install_id = params.install_id
        ))
        .header("User-Agent", "chdsbd/heroku-deploy-notifier")
        .header(AUTHORIZATION, format!("Bearer {}", params.jwt))
        .header(ACCEPT, "application/vnd.github.machine-man-preview+json")
        .send()
        .unwrap();

    println!("Bearer {}", params.jwt);
    res.error_for_status_ref().unwrap();

    res.json::<GithubAccessToken>().unwrap()
}

// https://devcenter.heroku.com/articles/deploy-hooks#http-post-hook

#[derive(FromForm, Debug)]
struct Event {
    app: String,
    user: String,
    url: String,
    head: String,
    head_long: String,
    prev_head: String,
    git_log: String,
    release: String,
}

#[derive(Deserialize, Debug)]
struct Actor {
    login: String,
    id: i64,
    html_url: String,
    avatar_url: String,
}
#[derive(Deserialize, Debug)]
struct CommitAuthor {
    date: String,
}
#[derive(Deserialize, Debug)]
struct Commit {
    message: String,
    url: String,
    author: CommitAuthor,
}

#[derive(Deserialize, Debug)]
struct CommitNode {
    sha: String,
    commit: Commit,
    author: Actor,
    html_url: String,
}

#[derive(Deserialize, Debug)]
struct CommitComparison {
    url: String,
    html_url: String,
    permalink_url: String,
    commits: Vec<CommitNode>,
}

struct FormattedCommitArgs {
    commit_url: String,
    commit_title: String,
    head_short: String,
    commit_author_login: String,
    relative_commit_time: String,
}
fn formatted_commit(params: FormattedCommitArgs) -> String {
    format!("<{commit_url}|{commit_title}> `{head_short}`\n{commit_author_login} committed {relative_commit_time}",commit_url=params.commit_url,commit_title=params.commit_title,head_short=params.head_short,commit_author_login=params.commit_author_login,relative_commit_time=params.relative_commit_time)
}

struct GetSlackMessage {
    heroku_app_name: String,
    body_message: String,
    release: String,
    html_compare_url: String,
}
fn get_slack_message(params: GetSlackMessage) -> Value {
    json!([
        {
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": format!("Your changes have been released to <http://https://dashboard.heroku.com/apps/{heroku_app_name}|`{heroku_app_name}`> on Heroku.",heroku_app_name=params.heroku_app_name)
            }
        },
        {
            "type": "divider"
        },
        {
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": params.body_message
            }
        },
        {
            "type": "divider"
        },
        {
            "type": "context",
            "elements": [
                {
                    "type": "mrkdwn",
                    "text": format!("<{html_compare_url}|Compare diff> | <https://dashboard.heroku.com/apps/{heroku_app_name}/activity/releases/{release}|Release log> | <https://dashboard.heroku.com/apps/{heroku_app_name}|Release activity> | {release}", heroku_app_name=params.heroku_app_name, release=params.release,html_compare_url=params.html_compare_url)
                }
            ]
        }
    ])
}

/// https://slack.com/api/chat.postMessage
fn slack_chat_post_message(token: &str, channel: &str, blocks: Value) {
    let client = reqwest::blocking::Client::new();
    println!("{}", blocks);
    let res = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(token)
        .json(&json!({"channel":channel, "blocks":blocks}))
        .send()
        .unwrap();
    res.error_for_status_ref().unwrap();
    println!("{:?}", res.json::<Value>().unwrap());
}

#[post("/heroku_deploy_hook", data = "<task>")]
fn heroku_deploy_hook(task: Form<Event>, config: State<Opt>) -> String {
    // https://developer.github.com/v3/repos/commits/#compare-two-commits
    // https://api.github.com/repos/chdsbd/kodiak/compare/7c68a71a87d12cc2404aed192840674af84f3df4...master
    let github_compare_url = format!(
        "https://api.github.com/repos/{org}/{repo}/compare/{base}...{head}",
        org = config.github_org_name,
        repo = config.github_repo_name,
        base = task.prev_head,
        head = task.head_long
    );

    let jwt = generate_jwt(&config.github_app_private_key, &config.github_app_id).unwrap();
    println!("{:?}", jwt);
    let access_token = create_access_token_for_install(CreateAccessTokenForInstall {
        jwt,
        install_id: config.github_app_install_id.clone(),
    });

    println!("{:?}", access_token);

    let client = reqwest::blocking::Client::builder()
        .user_agent("chdsbd/heroku-deploy-notifier")
        .build()
        .unwrap();
    let res = client
        .get(&github_compare_url)
        .header("Authorization", format!("Bearer {}", access_token.token))
        .send()
        .unwrap();

    res.error_for_status_ref().unwrap();
    let body = res.json::<CommitComparison>().unwrap();

    // 1. find all commits from deploy
    // 2. find all github user ids from deploy
    // 3. create messages for each user listing the commits that were deployed
    //    to the app name.

    let mut github_id_to_message: HashMap<i64, Vec<String>> = HashMap::new();
    for commit in body.commits.iter() {
        let author_id = commit.author.id;

        let my_entry: &mut Vec<String> =
            github_id_to_message.entry(author_id).or_insert(Vec::new());

        let new_message: String = formatted_commit(FormattedCommitArgs {
            commit_author_login: commit.author.login.clone(),
            commit_title: commit
                .commit
                .message
                .clone()
                .splitn(2, '\n')
                .nth(0)
                .unwrap_or(&commit.commit.message)
                .to_string(),
            commit_url: commit.html_url.clone(),
            head_short: String::from(commit.sha.get(..7).unwrap()),
            relative_commit_time: format!(
                "{}",
                chrono_humanize::HumanTime::from(
                    DateTime::parse_from_rfc3339(&commit.commit.author.date).unwrap()
                )
            ),
        });

        my_entry.push(new_message);
    }

    let mut slack_messages_to_send = Vec::new();
    for (github_id, messages) in github_id_to_message.iter() {
        let slack_id = config.github_slack_user_ids.get(github_id);
        if let Some(slack_id) = slack_id {
            let slack_msg = get_slack_message(GetSlackMessage {
                heroku_app_name: config.heroku_app_name.clone(),
                body_message: messages.join("\n"),
                html_compare_url: body.html_url.clone(),
                release: task.release.clone(),
            });
            slack_messages_to_send.push(format!("slack_id={} message={}", slack_id, slack_msg));
            slack_chat_post_message(&config.slack_oauth_token, slack_id, slack_msg)
        }
    }
    slack_messages_to_send.join("\n")
}

#[derive(Deserialize, Debug)]
struct User {
    github_id: i64,
    slack_id: String,
}

use std::error::Error;

fn parse_github_id_slack_id_many(
    s: &str,
) -> Result<HashMap<GitHubUserId, SlackUserId>, Box<dyn Error>> {
    Ok(s.split_whitespace()
        .map(|x| {
            let pos = x
                .find('=')
                .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))
                .unwrap();
            let github_id: i64 = x[..pos].parse().unwrap();
            let slack_id: String = x[pos + 1..].parse().unwrap();
            User {
                github_id,
                slack_id,
            }
        })
        .fold(HashMap::new(), |mut acc, x| {
            acc.insert(x.github_id, x.slack_id.clone());
            acc
        }))
}

type GitHubUserId = i64;
type SlackUserId = String;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    /// Slug name of Heroku app
    #[structopt(long, env = "HEROKU_APP_NAME")]
    heroku_app_name: String,
    /// Name of GitHub organization corresponding to deploy
    #[structopt(long, env = "GITHUB_ORG_NAME")]
    github_org_name: String,

    /// Name of GitHub repository corresponding to deploy
    #[structopt(long, env = "GITHUB_REPO_NAME")]
    github_repo_name: String,

    /// Github App ID for authenticating with GitHub API
    #[structopt(long, env = "GITHUB_APP_ID")]
    github_app_id: String,

    /// Github App private key for authenticating with GitHub API
    #[structopt(long, env = "GITHUB_APP_PRIVATE_KEY")]
    github_app_private_key: String,

    /// Github App installation ID
    #[structopt(long, env = "GITHUB_APP_INSTALL_ID")]
    github_app_install_id: String,

    /// Slack OAuth Token for sending Slack messages to users
    #[structopt(long, env = "SLACK_OAUTH_TOKEN")]
    slack_oauth_token: String,

    /// github id to slack id mappings
    ///
    /// ex: for github_id 1929960 and slack_id UAXQFKA3C, write -U 1929960=UAXQFKA3C
    #[structopt(short = "U", long, env="GITHUB_SLACK_USER_IDS", parse(try_from_str = parse_github_id_slack_id_many), number_of_values = 1)]
    github_slack_user_ids: HashMap<GitHubUserId, SlackUserId>,
}

fn main() {
    let opt = Opt::from_args();
    rocket::ignite()
        .mount("/", routes![root, heroku_deploy_hook])
        .manage(opt)
        .launch();
}
