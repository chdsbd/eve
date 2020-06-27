# Eve
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
export ROCKET_ENV=prod
GITHUB_PRIVATE_KEY=$(cat my-github-app-name.2020-01-01.private-key.pem)
cargo run -- \
    --github-app-id 2047 \
    --github-app-install-id 10012254 \
    --github-app-private-key="$GITHUB_PRIVATE_KEY" \
    --github-org-name acme-corp \
    --github-repo-name api \
    --github-slack-user-ids '1929960=UAXQFKA3C 7340772=UAYMB3CNS' \
    --heroku-app-name acme-corp-prod \
    --slack-oauth-token xoxb-c6768786-5f6c43dc-acbeba4045d90c08
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
