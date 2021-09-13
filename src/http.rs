use rocket::config::{Config, Environment};
use rocket::State;
use rocket_contrib::json::Json;
use serde::Deserialize;

use crate::heroku;

#[get("/")]
const fn root() -> &'static str {
    "Heroku Deploy Notifier"
}

#[derive(Deserialize, Debug)]
struct WebhookReleaseApp {
    name: String,
}

#[derive(Deserialize, Debug)]
struct WebhookReleaseEventSlug {
    id: String,
    commit: String,
}
#[derive(Deserialize, Debug)]
struct WebhookReleaseEventData {
    app: WebhookReleaseApp,
    slug: WebhookReleaseEventSlug,
    version: i64,
}
#[derive(Deserialize, Debug)]
struct WebhookReleaseEvent {
    action: String,
    data: WebhookReleaseEventData,
}

#[post(
    "/heroku_webhook?<auth_token>&<github_org_name>&<github_repo_name>",
    data = "<task>"
)]
fn heroku_webhook(
    task: Json<WebhookReleaseEvent>,
    auth_token: String,
    github_org_name: String,
    github_repo_name: String,
    config: State<crate::cli::Opt>,
) -> Result<(), crate::EveError> {
    if auth_token != config.secret {
        return Err(crate::EveError::InternalError("invalid auth".to_string()));
    }

    if task.action != "update" {
        return Ok(());
    }
    let release = task.data.version;
    let app = &task.data.app.name;
    let heroku_token = &config.heroku_token;
    let base_ref = heroku::get_slug(
        app,
        &heroku::get_release(app, release - 1, heroku_token)?.slug.id,
        heroku_token,
    )?
    .commit;

    let head_ref = &task.data.slug.commit;

    Ok(crate::handle_post_deploy_event(
        crate::HandlePostDeployEvent {
            github_app_private_key: &config.github_app_private_key,
            github_app_id: &config.github_app_id,
            github_app_install_id: &config.github_app_install_id,
            github_org: &github_org_name,
            github_repo: &github_repo_name,
            github_ref_base: &base_ref,
            github_ref_head: head_ref,
            github_slack_users: &config.github_slack_user_ids,
            slack_oauth_token: &config.slack_oauth_token,
            heroku_release: &format!("v{}", release),
            heroku_app_name: app,
            now: chrono::Utc::now().into(),
        },
    )?)
}

pub fn start_server(opt: crate::cli::Opt) {
    let env = if opt.debug {
        Environment::Development
    } else {
        Environment::Production
    };
    let mut config = Config::new(env);
    config.port = opt.port;
    rocket::custom(config)
        .mount("/", routes![root, heroku_webhook])
        .manage(opt)
        .launch();
}
