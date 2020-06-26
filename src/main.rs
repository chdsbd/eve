#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;

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
fn heroku_deploy_hook(task: Form<Event>, config: State<Opt>) -> Result<(), ()> {
    eve::handle_post_deploy_event(eve::HandlePostDeployEvent {
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
    Ok(())
}

#[derive(Deserialize, Debug)]
struct User {
    github_id: i64,
    slack_id: String,
}

#[derive(Debug, PartialEq)]
enum ParseGithubSlackIdError<'a> {
    MissingEquals(&'a str),
    GitHubIdParseErr(&'a str),
    SlackIdParseErr(&'a str),
}

impl<'a> std::fmt::Display for ParseGithubSlackIdError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEquals(s) => write!(f, "invalid KEY=value: no `=` found in `{}`", s),
            Self::GitHubIdParseErr(s) => write!(f, "could not parse GitHub ID from `{}`", s),
            Self::SlackIdParseErr(s) => write!(f, "could not parse Slack ID from `{}`", s),
        }
    }
}

/// Parse mapping of github to slack ids
///
/// modified from https://github.com/clap-rs/clap/blob/f72b728ed7ba32e7f1ca33db832c61cc7adfea8f/clap_derive/examples/keyvalue.rs#L6-L18
fn parse_github_id_slack_id_many(
    s: &str,
) -> Result<HashMap<GitHubUserId, SlackUserId>, ParseGithubSlackIdError> {
    let mut users = HashMap::new();
    for mapping in s.split_whitespace() {
        let pos = mapping
            .find('=')
            .ok_or_else(|| ParseGithubSlackIdError::MissingEquals(s))?;
        let github_id: GitHubUserId = mapping[..pos]
            .parse()
            .map_err(|_| ParseGithubSlackIdError::GitHubIdParseErr(&mapping[..pos]))?;
        let slack_id: SlackUserId = mapping[pos + 1..]
            .parse()
            .map_err(|_| ParseGithubSlackIdError::SlackIdParseErr(&mapping[..pos]))?;
        users.insert(github_id, slack_id);
    }
    Ok(users)
}

#[cfg(test)]
mod test_parse_github_id {
    use super::*;

    #[test]
    fn test_successful() {
        let mut expected = HashMap::new();
        expected.insert(1929960, "UAXQFKA3C".to_string());
        let actual = parse_github_id_slack_id_many("1929960=UAXQFKA3C").unwrap();
        assert_eq!(actual, expected);
    }
    #[test]
    fn test_successful_many() {
        let mut expected = HashMap::new();
        expected.insert(1929960, "UAXQFKA3C".to_string());
        expected.insert(7340772, "UAYMB3CNS".to_string());
        let actual = parse_github_id_slack_id_many("1929960=UAXQFKA3C 7340772=UAYMB3CNS").unwrap();
        assert_eq!(actual, expected);
    }
    #[test]
    fn test_missing_equals() {
        let actual = parse_github_id_slack_id_many("1929960 UAXQFKA3C");
        assert_eq!(
            format!("{}", actual.err().unwrap()),
            "invalid KEY=value: no `=` found in `1929960 UAXQFKA3C`".to_string()
        )
    }
    #[test]
    fn test_invalid_github_id() {
        let actual = parse_github_id_slack_id_many("HC29960=UAXQFKA3C");
        assert_eq!(
            format!("{}", actual.err().unwrap()),
            "could not parse GitHub ID from `HC29960`".to_string()
        )
    }
    #[test]
    #[should_panic(expected = "called `Option::unwrap()`")]
    fn test_invalid_slack_id() {
        let actual = parse_github_id_slack_id_many("1929960=ZAXQFKA3C");
        assert_eq!(
            format!("{}", actual.err().unwrap()),
            "could not parse Slack ID from `ZAXQFKA3C`".to_string()
        )
    }
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
