# EVE
> A bot to notify Slack users when their GitHub changes have been deployed on Heroku

## Running

1. Create a GitHub App at https://github.com/settings/apps/new.
    - Uncheck the Webhook "Active" checkbox
    - Enable "Read-only" access to the "Contents" repository permission. This allows Eve to compare commits.
2. Download a private key to authenticate as the GitHub App
3. Create a Slack App at https://api.slack.com/apps.
    - Configure "Permissions" and add the "Bot Token Scopes" of `chat:write` and `im:write`
    - Install the app via "Install App to Workspace" and copy "Bot User OAuth Access Token"
3. Run Eve
```bash
SECRET=my-secret-key \
GITHUB_APP_ID=1047 \
GITHUB_APP_PRIVATE_KEY=$(cat acme-corp-eve.2020-01-01.private-key.pem) \
GITHUB_APP_INSTALL_ID=202154 \
SLACK_OAUTH_TOKEN='xoxb-c6768786-5f6c43dc-acbeba4045d90c08' \
GITHUB_SLACK_USER_IDS='1929960=UAXQFKA3C 7340772=UAYMB3CNS' \
cargo run

curl "localhost:8000/heroku_deploy_hook?auth_token=$SECRET&github_org_name=acme-corp&github_repo_name=blog"
```

## Development

```bash
# build
cargo build

# test
cargo test

# format
cargo format

# lint
cargo clippy --all-targets --all-features -- -D clippy::nursery
```

## Deployment to Heroku

1. Create a Heroku app
2. Initialize your Heroku app: `heroku git:remote -a my-app-name`
3. Add the [Rust buildpack](https://github.com/emk/heroku-buildpack-rust): `heroku buildpacks:set emk/rust`
4. Push your app to Heroku: `git push heroku master`
5. Configure environment variables via the dashboard or `heroku config:set KEY=VALUE`
6. Finished. You could add the app url as a Heroku Post Deploy Hook, like `https://my-app-name.herokuapp.com/heroku_deploy_hook?auth_token=my-secret-key`.
