#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

mod github;
mod slack;

use chrono::DateTime;
use rocket::request::Form;
use serde::Deserialize;
use serde_json::{json, Value};

use rocket::State;
use std::collections::HashMap;
use structopt::StructOpt;

#[get("/")]
fn root() -> &'static str {
    "Heroku Deploy Notifier"
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

#[post("/heroku_deploy_hook", data = "<task>")]
fn heroku_deploy_hook(task: Form<Event>, config: State<Opt>) -> String {
    let body = github::compare(github::Compare {
        private_key: &config.github_app_private_key,
        app_id: &config.github_app_id,
        install_id: &config.github_app_install_id,
        org: &config.github_org_name,
        repo: &config.github_repo_name,
        base: &task.prev_head,
        head: &task.head_long,
    });

    // 1. find all commits from deploy
    // 2. find all github user ids from deploy
    // 3. create messages for each user listing the commits that were deployed
    //    to the app name.

    let mut github_id_to_message: HashMap<i64, Vec<String>> = HashMap::new();
    for commit in body.commits.iter() {
        let author_id = commit.author.id;

        let my_entry: &mut Vec<String> = github_id_to_message
            .entry(author_id)
            .or_insert_with(Vec::new);

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
            slack::chat_post_message(&config.slack_oauth_token, slack_id, slack_msg)
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
            acc.insert(x.github_id, x.slack_id);
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
