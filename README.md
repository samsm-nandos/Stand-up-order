# slack-standup-order-bot

Posts a random standup order for a Slack channel every weekday morning, via a GitHub Actions cron job.

## Setup

1. Create a Slack app at https://api.slack.com/apps ("From scratch").
2. Under **OAuth & Permissions**, add these Bot Token Scopes:
   - `channels:read`
   - `channels:history`
   - `groups:read`
   - `im:read`
   - `mpim:read`
   - `users:read`
   - `chat:write`
   - `commands`
3. Install the app to your workspace and invite the bot to the target channel (`/invite @your-bot`).
4. Copy the **Bot User OAuth Token** (starts with `xoxb-`).
5. In this repo's GitHub settings → Secrets and variables → Actions, add:
   - `SLACK_BOT_TOKEN` — the bot token from step 4
   - `SLACK_CHANNEL_ID` — the target channel's ID (right-click the channel in Slack → View channel details)
6. Adjust the cron schedule in `.github/workflows/standup.yml` if needed (it runs in UTC).

## CI builds

`.github/workflows/build.yml` compiles a release binary on every push to `main` and publishes it to a rolling `latest` GitHub Release. `.github/workflows/standup.yml` downloads that binary and runs it directly, instead of compiling from source on each scheduled run. After the first push to `main`, check the Actions tab to confirm the build workflow succeeded and a `latest` release exists before relying on the cron job.

## Testing

Trigger the workflow manually from the Actions tab ("Run workflow") to test without waiting for the schedule, or run locally:

```sh
SLACK_BOT_TOKEN=xoxb-... SLACK_CHANNEL_ID=C0123456789 cargo run
```
