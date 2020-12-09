#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;

pub mod cli;
mod github;
pub mod http;
mod slack;

use chrono::{DateTime, FixedOffset};
use serde_json::{json, Value};

use std::collections::HashMap;

/// https://api.slack.com/reference/surfaces/formatting#escaping
fn escape_mrkdwn(text: &str) -> String {
    text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
}

struct GetSlackMessage<'a> {
    heroku_app_name: &'a str,
    commits: &'a Vec<Commit<'a>>,
    release: &'a str,
    html_compare_url: &'a str,
}
fn get_slack_message(params: GetSlackMessage) -> Value {
    let commit_messages = params.commits.iter().map(|commit| {
        let sha_short = &commit.sha[..7];
        let relative_commit_time = &chrono_humanize::HumanTime::from(commit.date).to_string();
        format!("<{commit_url}|{commit_title}> `{head_short}`\n{commit_author_login} committed {relative_commit_time}",
            commit_url=commit.url,
            commit_title=escape_mrkdwn(commit.title),
            head_short=sha_short,
            commit_author_login=commit.author_login,
            relative_commit_time=relative_commit_time
        )

    }).collect::<Vec<String>>().join("\n");
    json!([
        {
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": format!("Your changes have been released to <https://dashboard.heroku.com/apps/{heroku_app_name}|`{heroku_app_name}`> on Heroku.",heroku_app_name=params.heroku_app_name)
            }
        },
        {
            "type": "divider"
        },
        {
            "type": "section",
            "text": {
                "type": "mrkdwn",
                "text": commit_messages
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

#[derive(Debug)]
pub enum EveError {
    SlackError(slack::SlackError),
    GitHubError(github::GitHubError),
    InternalError(String),
}

impl std::convert::From<slack::SlackError> for EveError {
    fn from(e: slack::SlackError) -> Self {
        Self::SlackError(e)
    }
}

impl std::convert::From<github::GitHubError> for EveError {
    fn from(e: github::GitHubError) -> Self {
        Self::GitHubError(e)
    }
}

pub type GithubUserId = i64;
pub type SlackUserId = String;

pub struct HandlePostDeployEvent<'a> {
    pub github_app_private_key: &'a str,
    pub github_app_id: &'a str,
    pub github_app_install_id: &'a str,
    pub github_org: &'a str,
    pub github_repo: &'a str,
    pub github_ref_base: &'a str,
    pub github_ref_head: &'a str,
    pub github_slack_users: &'a HashMap<GithubUserId, SlackUserId>,
    pub slack_oauth_token: &'a str,
    pub heroku_release: &'a str,
    pub heroku_app_name: &'a str,
}
struct Commit<'a> {
    author_login: &'a str,
    title: &'a str,
    url: &'a str,
    sha: &'a str,
    date: DateTime<FixedOffset>,
}
pub fn handle_post_deploy_event(params: HandlePostDeployEvent) -> Result<(), EveError> {
    // get the comments for the deploy.
    let body = github::compare(github::Compare {
        private_key: params.github_app_private_key,
        app_id: params.github_app_id,
        install_id: params.github_app_install_id,
        org: params.github_org,
        repo: params.github_repo,
        base: params.github_ref_base,
        head: params.github_ref_head,
    })?;

    // aggregate the commit messages per user to insert into Slack message.
    let mut github_id_to_message: HashMap<GithubUserId, Vec<Commit>> = HashMap::new();
    for commit in body.commits.iter() {
        let author_id = commit.author.id;

        let github_user_messages = github_id_to_message
            .entry(author_id)
            .or_insert_with(Vec::new);

        // select the "title" of the commit be slicing off the string at first
        // new line character.
        let commit_title = commit
            .commit
            .message
            .splitn(2, '\n')
            .next()
            .unwrap_or(&commit.commit.message);
        // get a nice looking short commit.
        let commit_date =
            DateTime::parse_from_rfc3339(&commit.commit.author.date).map_err(|_| {
                EveError::InternalError(format!(
                    "Could not parse date from commit author information. {}",
                    commit.commit.author.date
                ))
            })?;
        github_user_messages.push(Commit {
            author_login: &commit.author.login,
            title: commit_title,
            url: &commit.html_url,
            sha: &commit.sha,
            date: commit_date,
        });
    }

    // send messages to each Slack user with GitHub commits.
    for (github_id, commits) in github_id_to_message.iter() {
        let slack_id = params.github_slack_users.get(github_id);
        if let Some(slack_id) = slack_id {
            let slack_msg = get_slack_message(GetSlackMessage {
                heroku_app_name: params.heroku_app_name,
                commits,
                html_compare_url: &body.html_url,
                release: params.heroku_release,
            });
            slack::chat_post_message(params.slack_oauth_token, slack_id, slack_msg)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_escaping_slack_messages() {
        let res = get_slack_message(GetSlackMessage {
            heroku_app_name: "",
            commits: &vec![Commit {
                author_login: "ghost",
                title: "Fix <Foo/> & some other thing",
                url: "https://example.org",
                sha: "56b515000c090c0ba5f285c6e19f9451788413f1",
                date: DateTime::parse_from_rfc3339("2015-12-19T16:39:57-08:00").unwrap(),
            }],
            release: "heroku-release-id",
            html_compare_url: "https://github.com/repos/ghost/repo/compare/7c68a71a87d12cc2404aed192840674af84f3df4...master",
        });
        insta::assert_display_snapshot!(serde_json::to_string_pretty(&res).unwrap());
    }
}
