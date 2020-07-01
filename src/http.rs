use rocket::config::{Config, Environment};
use rocket::request::LenientForm;
use rocket::State;

#[get("/")]
const fn root() -> &'static str {
    "Heroku Deploy Notifier"
}

// https://devcenter.heroku.com/articles/deploy-hooks#http-post-hook
#[derive(FromForm, Debug)]
struct Event {
    app: String,
    head_long: String,
    prev_head: String,
    release: String,
}

#[post("/heroku_deploy_hook?<auth_token>", data = "<task>")]
fn heroku_deploy_hook(
    task: LenientForm<Event>,
    auth_token: String,
    config: State<crate::cli::Opt>,
) -> Result<(), crate::EveError> {
    if auth_token != config.secret {
        return Err(crate::EveError::InternalError("invalid auth".to_string()));
    }
    Ok(crate::handle_post_deploy_event(
        crate::HandlePostDeployEvent {
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
        .mount("/", routes![root, heroku_deploy_hook])
        .manage(opt)
        .launch();
}
