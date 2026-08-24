use futures::future::try_join_all;
use futures::stream::{self, TryStreamExt};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::Deserialize;
use std::env;
use std::error::Error;

const BASE_URL: &str = "https://slack.com/api";
const HOLIDAY_STATUS_EMOJI: &str = ":palm_tree:";
const ELIGIBLE_TITLE_KEYWORDS: &[&str] = &["engineer", "technical lead"];

fn is_engineer(title: &str) -> bool {
    let title = title.to_lowercase();
    ELIGIBLE_TITLE_KEYWORDS
        .iter()
        .any(|keyword| title.contains(keyword))
}

fn slack_error(endpoint: &str, error: Option<String>, needed: Option<String>) -> Box<dyn Error> {
    let error = error.unwrap_or_else(|| "unknown error".into());
    match needed {
        Some(needed) => format!("{endpoint}: {error} (needed scope: {needed})").into(),
        None => format!("{endpoint}: {error}").into(),
    }
}

#[derive(Deserialize)]
struct MembersResponse {
    ok: bool,
    error: Option<String>,
    needed: Option<String>,
    members: Option<Vec<String>>,
    response_metadata: Option<ResponseMetadata>,
}

#[derive(Deserialize)]
struct ResponseMetadata {
    next_cursor: Option<String>,
}

#[derive(Deserialize)]
struct UserInfoResponse {
    ok: bool,
    error: Option<String>,
    needed: Option<String>,
    user: Option<SlackUser>,
}

#[derive(Deserialize)]
struct SlackUser {
    is_bot: bool,
    deleted: bool,
    profile: SlackProfile,
}

#[derive(Deserialize)]
struct SlackProfile {
    status_emoji: String,
    title: String,
}

#[derive(Deserialize)]
struct PostMessageResponse {
    ok: bool,
    error: Option<String>,
    needed: Option<String>,
}

async fn fetch_members_page(
    client: &reqwest::Client,
    token: &str,
    channel: &str,
    cursor: &str,
) -> Result<(Vec<String>, Option<String>), Box<dyn Error>> {
    let mut params = vec![("channel", channel)];
    if !cursor.is_empty() {
        params.push(("cursor", cursor));
    }

    let res: MembersResponse = client
        .get(format!("{BASE_URL}/conversations.members"))
        .bearer_auth(token)
        .query(&params)
        .send()
        .await?
        .json()
        .await?;

    if !res.ok {
        return Err(slack_error("conversations.members", res.error, res.needed));
    }

    let next_cursor = res
        .response_metadata
        .and_then(|m| m.next_cursor)
        .filter(|c| !c.is_empty());

    Ok((res.members.unwrap_or_default(), next_cursor))
}

async fn get_channel_member_ids(
    client: &reqwest::Client,
    token: &str,
    channel: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let pages: Vec<Vec<String>> = stream::try_unfold(Some(String::new()), |cursor| async move {
        let Some(cursor) = cursor else {
            return Ok::<_, Box<dyn Error>>(None);
        };

        let (members, next_cursor) = fetch_members_page(client, token, channel, &cursor).await?;
        Ok(Some((members, next_cursor)))
    })
    .try_collect()
    .await?;

    Ok(pages.into_iter().flatten().collect())
}

async fn fetch_user(
    client: &reqwest::Client,
    token: &str,
    id: &str,
) -> Result<SlackUser, Box<dyn Error>> {
    let res: UserInfoResponse = client
        .get(format!("{BASE_URL}/users.info"))
        .bearer_auth(token)
        .query(&[("user", id)])
        .send()
        .await?
        .json()
        .await?;

    if !res.ok {
        return Err(slack_error("users.info", res.error, res.needed));
    }

    res.user.ok_or_else(|| "missing user in response".into())
}

async fn extract_users(
    client: &reqwest::Client,
    token: &str,
    member_ids: Vec<String>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let users = try_join_all(member_ids.iter().map(|id| fetch_user(client, token, id))).await?;

    Ok(member_ids
        .into_iter()
        .zip(users)
        .filter(|(_, user)| {
            !user.is_bot
                && !user.deleted
                && user.profile.status_emoji != HOLIDAY_STATUS_EMOJI
                && is_engineer(&user.profile.title)
        })
        .map(|(id, _)| id)
        .collect())
}

async fn post_standup_order(
    client: &reqwest::Client,
    token: &str,
    channel: &str,
    order: &[String],
) -> Result<(), Box<dyn Error>> {
    let lines: Vec<String> = order
        .iter()
        .enumerate()
        .map(|(i, id)| format!("{}. <@{}>", i + 1, id))
        .collect();
    let text = format!(":coffee: *Today's standup order:*\n{}", lines.join("\n"));

    let res: PostMessageResponse = client
        .post(format!("{BASE_URL}/chat.postMessage"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "channel": channel, "text": text }))
        .send()
        .await?
        .json()
        .await?;

    if !res.ok {
        return Err(slack_error("chat.postMessage", res.error, res.needed));
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let token = env::var("SLACK_BOT_TOKEN").expect("SLACK_BOT_TOKEN must be set");
    let channel = env::var("SLACK_CHANNEL_ID").expect("SLACK_CHANNEL_ID must be set");

    let client = reqwest::Client::new();

    let member_ids = get_channel_member_ids(&client, &token, &channel).await?;
    let mut order = extract_users(&client, &token, member_ids).await?;
    order.shuffle(&mut thread_rng());

    post_standup_order(&client, &token, &channel, &order).await?;

    Ok(())
}
