#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use chrono::DateTime;
use chrono_humanize;
use rocket::request::Form;
use serde::{Serialize,Deserialize};
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde_json::{json,Value};

use rocket::State;
use rocket::response::NamedFile;
use rocket::fairing::AdHoc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use jsonwebtoken::{Algorithm, EncodingKey, Header};

#[derive(Debug)]
struct AppConfig {
    heroku_app_name: String,
    github_org_name: String,
    github_repo_name: String,
    github_app_id: String,
    github_app_private_key: String,
    github_app_install_id: String
}




#[get("/")]
fn hello() -> &'static str {
    "Hello, world!"
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
        iss: app_identifier.to_owned()
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
    jwt: String
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
        .send().unwrap();

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

#[post("/heroku_deploy_hook", data = "<task>")]
fn heroku_deploy_hook(task: Form<Event>, config: State<AppConfig>) -> String {
    // https://developer.github.com/v3/repos/commits/#compare-two-commits
    // https://api.github.com/repos/chdsbd/kodiak/compare/7c68a71a87d12cc2404aed192840674af84f3df4...master
    let github_compare_url = format!(
        "https://api.github.com/repos/{org}/{repo}/compare/{base}...{head}",
        org = config.github_org_name,
        repo = config.github_repo_name,
        base = task.prev_head,
        head = task.head_long
    );

    let jwt = generate_jwt(&config.github_app_private_key,&config.github_app_id).unwrap();
    println!("{:?}", jwt);
    let access_token = create_access_token_for_install(CreateAccessTokenForInstall {
        jwt,
        install_id: config.github_app_install_id.clone()
    });

    println!("{:?}", access_token);

    let client = reqwest::blocking::Client::builder()
        .user_agent("chdsbd/heroku-deploy-notifier")
        .build()
        .unwrap();
    let res = client.get(&github_compare_url).header("Authorization", format!("Bearer {}", access_token.token)).send().unwrap();

    res.error_for_status_ref().unwrap();
    let body = res.json::<CommitComparison>().unwrap();

    // 1. find all commits from deploy
    // 2. find all github user ids from deploy
    // 3. create messages for each user listing the commits that were deployed
    //    to the app name.
    //

    let body_commits = body
        .commits
        .iter()
        .map(|x| {
            formatted_commit(FormattedCommitArgs {
                commit_author_login: x.author.login.clone(),
                commit_title: x
                    .commit
                    .message
                    .clone()
                    .splitn(2, '\n')
                    .nth(0)
                    .unwrap_or(&x.commit.message)
                    .to_string(),
                commit_url: x.html_url.clone(),
                head_short: String::from(x.sha.get(..7).unwrap()),
                relative_commit_time: format!(
                    "{}",
                    chrono_humanize::HumanTime::from(
                        DateTime::parse_from_rfc3339(&x.commit.author.date).unwrap()
                    )
                ),
            })
        })
        .collect::<Vec<String>>()
        .join("\n");

    let slack_message = json!({
        "blocks": [
            {
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("Your changes have been released to <http://https://dashboard.heroku.com/apps/{heroku_app_name}|`{heroku_app_name}`> on Heroku.",heroku_app_name=config.heroku_app_name)
                }
            },
            {
                "type": "divider"
            },
            {
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": body_commits
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
                        "text": format!("<{html_compare_url}|Compare diff> | <https://dashboard.heroku.com/apps/{heroku_app_name}/activity/releases/{release}|Release log> | <https://dashboard.heroku.com/apps/{heroku_app_name}|Release activity> | {release}", heroku_app_name=config.heroku_app_name, release=task.release,html_compare_url=body.html_url)
                    }
                ]
            }
        ]
    });

    println!("{:#?}",config);

    format!("{}", slack_message)
}

fn main() {
    let heroku_app_name = std::env::var("HEROKU_APP_NAME").unwrap();
    let github_org_name = std::env::var("GITHUB_ORG_NAME").unwrap();
    let github_repo_name = std::env::var("GITHUB_REPO_NAME").unwrap();
    let github_app_id = std::env::var("GITHUB_APP_ID").unwrap();
    let github_app_private_key = std::env::var("GITHUB_APP_PRIVATE_KEY").unwrap();
    let config = AppConfig {heroku_app_name,
        github_org_name,github_repo_name,github_app_private_key,github_app_id, github_app_install_id: std::env::var("GITHUB_APP_INSTALL_ID").unwrap()};
    rocket::ignite()
        .mount("/", routes![hello, heroku_deploy_hook])
        .manage(config)
        .launch();
}
