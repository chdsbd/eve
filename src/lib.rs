mod github;
mod slack;

use chrono::DateTime;
use serde_json::{json, Value};

use std::collections::HashMap;

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

struct GetSlackMessage<'a> {
    heroku_app_name: &'a str,
    body_message: &'a str,
    release: &'a str,
    html_compare_url: &'a str,
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
pub fn handle_post_deploy_event(params: HandlePostDeployEvent) -> Result<(), EveError> {
    let body = github::compare(github::Compare {
        private_key: params.github_app_private_key,
        app_id: params.github_app_id,
        install_id: params.github_app_install_id,
        org: params.github_org,
        repo: params.github_repo,
        base: params.github_ref_base,
        head: params.github_ref_head,
    })?;

    // 1. find all commits from deploy
    // 2. find all github user ids from deploy
    // 3. create messages for each user listing the commits that were deployed
    //    to the app name.

    let mut github_id_to_message: HashMap<GithubUserId, Vec<String>> = HashMap::new();
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
                .next()
                .unwrap_or(&commit.commit.message)
                .to_string(),
            commit_url: commit.html_url.clone(),
            head_short: String::from(commit.sha.get(..7).unwrap_or(&commit.sha)),
            relative_commit_time: format!(
                "{}",
                chrono_humanize::HumanTime::from(
                    DateTime::parse_from_rfc3339(&commit.commit.author.date).map_err(|_| {
                        EveError::InternalError(format!(
                            "Could not parse date from commit author information. {}",
                            commit.commit.author.date
                        ))
                    })?
                )
            ),
        });

        my_entry.push(new_message);
    }

    let mut slack_messages_to_send = Vec::new();
    for (github_id, messages) in github_id_to_message.iter() {
        let slack_id = params.github_slack_users.get(github_id);
        if let Some(slack_id) = slack_id {
            let slack_msg = get_slack_message(GetSlackMessage {
                heroku_app_name: params.heroku_app_name,
                body_message: &messages.join("\n"),
                html_compare_url: &body.html_url,
                release: params.heroku_release,
            });
            slack_messages_to_send.push(format!("slack_id={} message={}", slack_id, slack_msg));
            slack::chat_post_message(params.slack_oauth_token, slack_id, slack_msg)?;
        }
    }
    slack_messages_to_send.join("\n");
    Ok(())
}
