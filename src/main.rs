#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use chrono::DateTime;
use chrono_humanize;
use rocket::request::Form;
use serde::Deserialize;
use serde_json::json;

#[get("/")]
fn hello() -> &'static str {
    "Hello, world!"
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

const HEROKU_APP_NAME: &'static str = "kodiak-app";
const GITHUB_ORG_NAME: &'static str = "chdsbd";
const GITHUB_REPO_NAME: &'static str = "kodiak";

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
fn heroku_deploy_hook(task: Form<Event>) -> String {
    // https://developer.github.com/v3/repos/commits/#compare-two-commits
    // https://api.github.com/repos/chdsbd/kodiak/compare/7c68a71a87d12cc2404aed192840674af84f3df4...master
    let github_compare_url = format!(
        "https://api.github.com/repos/{org}/{repo}/compare/{base}...{head}",
        org = GITHUB_ORG_NAME,
        repo = GITHUB_REPO_NAME,
        base = task.prev_head,
        head = task.head_long
    );

    let client = reqwest::blocking::Client::builder()
        .user_agent("chdsbd/heroku-deploy-notifier")
        .build()
        .unwrap();
    let res = client.get(&github_compare_url).send().unwrap();

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
                    "text": format!("Your changes have been released to <http://https://dashboard.heroku.com/apps/{heroku_app_name}|`{heroku_app_name}`> on Heroku.",heroku_app_name=HEROKU_APP_NAME)
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
                        "text": format!("<{html_compare_url}|Compare diff> | <https://dashboard.heroku.com/apps/{heroku_app_name}/activity/releases/{release}|Release log> | <https://dashboard.heroku.com/apps/{heroku_app_name}|Release activity> | {release}", heroku_app_name=HEROKU_APP_NAME, release=task.release,html_compare_url=body.html_url)
                    }
                ]
            }
        ]
    });

    format!("{}", slack_message)
}

fn main() {
    rocket::ignite()
        .mount("/", routes![hello, heroku_deploy_hook])
        .launch();
}
