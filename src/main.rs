#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

use eva;

use rocket::request::Form;
use serde::Deserialize;

use rocket::State;
use std::collections::HashMap;
use structopt::StructOpt;

#[get("/")]
const fn root() -> &'static str {
    "Heroku Deploy Notifier"
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
fn heroku_deploy_hook(task: Form<Event>, config: State<Opt>) -> &str {
    eva::handle_update(eva::HandleUpdate {
        github_app_private_key: &config.github_app_private_key,
        github_app_id: &config.github_app_id,
        github_app_install_id: &config.github_app_install_id,
        github_org: &config.github_org_name,
        github_repo: &config.github_repo_name,
        github_ref_base: &task.prev_head,
        github_ref_head: &task.head_long,
        github_slack_users: &config.github_slack_user_ids,
        slack_oauth_token: &config.slack_oauth_token,
        heroku_release: &task.release,
        heroku_app_name: &task.app,
    })
    .unwrap();
    "OK"
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
