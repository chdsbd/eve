use serde::Deserialize;
use std::collections::HashMap;
use structopt::StructOpt;

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
        let actual =
            parse_github_id_slack_id_many("1929960=UAXQFKA3C").expect("should successfully parse");
        assert_eq!(actual, expected);
    }
    #[test]
    fn test_successful_many() {
        let mut expected = HashMap::new();
        expected.insert(1929960, "UAXQFKA3C".to_string());
        expected.insert(7340772, "UAYMB3CNS".to_string());
        let actual = parse_github_id_slack_id_many("1929960=UAXQFKA3C 7340772=UAYMB3CNS")
            .expect("should successfully parse");
        assert_eq!(actual, expected);
    }
    #[test]
    fn test_missing_equals() {
        let actual = parse_github_id_slack_id_many("1929960 UAXQFKA3C");
        assert_eq!(
            format!("{}", actual.err().expect("should have error")),
            "invalid KEY=value: no `=` found in `1929960 UAXQFKA3C`".to_string()
        )
    }
    #[test]
    fn test_invalid_github_id() {
        let actual = parse_github_id_slack_id_many("HC29960=UAXQFKA3C");
        assert_eq!(
            format!("{}", actual.err().expect("should have error")),
            "could not parse GitHub ID from `HC29960`".to_string()
        )
    }
    #[test]
    #[should_panic(expected = "should have error")]
    fn test_invalid_slack_id() {
        let actual = parse_github_id_slack_id_many("1929960=ZAXQFKA3C");
        assert_eq!(
            format!("{}", actual.err().expect("should have error")),
            "could not parse Slack ID from `ZAXQFKA3C`".to_string()
        )
    }
}

/// Parse boolean from string.
///
/// modified from https://github.com/TeXitoi/structopt/blob/b1174e5c9c0001386d7c0ca5e106f606d955eed1/examples/true_or_false.rs#L5-L11
fn true_or_false(s: &str) -> Result<bool, &'static str> {
    match s {
        "true" | "1" => Ok(true),
        "false" | "0" | "" => Ok(false),
        _ => Err("expected `true`, `1`, `false` or `0`"),
    }
}

pub type GitHubUserId = i64;
pub type SlackUserId = String;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
pub struct Opt {
    /// a secret token for authenticating requests.
    #[structopt(env = "SECRET")]
    pub secret: String,

    /// Github App ID for authenticating with GitHub API
    #[structopt(env = "GITHUB_APP_ID")]
    pub github_app_id: String,

    /// Github App private key for authenticating with GitHub API
    #[structopt(env = "GITHUB_APP_PRIVATE_KEY")]
    pub github_app_private_key: String,

    /// Github App installation ID
    #[structopt(env = "GITHUB_APP_INSTALL_ID")]
    pub github_app_install_id: String,

    /// Heroku API token
    #[structopt(env = "HEROKU_TOKEN")]
    pub heroku_token: String,

    /// Slack OAuth Token for sending Slack messages to users
    #[structopt(env = "SLACK_OAUTH_TOKEN")]
    pub slack_oauth_token: String,

    /// github id to slack id mappings
    ///
    /// ex: for github_id 1929960 and slack_id UAXQFKA3C, write -U 1929960=UAXQFKA3C
    #[structopt(env="GITHUB_SLACK_USER_IDS", parse(try_from_str = parse_github_id_slack_id_many), number_of_values = 1)]
    pub github_slack_user_ids: HashMap<GitHubUserId, SlackUserId>,

    /// enable debug mode for http server.
    #[structopt(env="DEBUG", parse(try_from_str = true_or_false), default_value="false")]
    pub debug: bool,

    /// configure port for http server.
    #[structopt(env = "PORT", default_value = "8000")]
    pub port: u16,
}

pub fn parse_args() -> Opt {
    Opt::from_args()
}
