#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;
use rocket::request::Form;
use rocket::State;

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
fn heroku_deploy_hook(
    task: Form<Event>,
    config: State<eve::cli::Opt>,
) -> Result<(), eve::EveError> {
    Ok(eve::handle_post_deploy_event(eve::HandlePostDeployEvent {
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
    })?)
}

fn main() {
    let opt = eve::cli::parse_args();
    rocket::ignite()
        .mount("/", routes![root, heroku_deploy_hook])
        .manage(opt)
        .launch();
}
