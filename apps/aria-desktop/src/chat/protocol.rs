//! Bounded, plain-text chat parsing. Remote messages never become markup or commands.
use anyhow::{Result, bail};
use reqwest::Url;
use serde_json::Value;
use std::collections::VecDeque;

pub const HISTORY: usize = 200;
#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub id: String,
    pub user_id: String,
    pub author: String,
    pub text: String,
    pub color: Option<[u8; 3]>,
}
pub enum Change {
    Add(Message),
    Delete(String),
    ClearUser(String),
    Clear,
}
pub fn command(line: &str) -> &str {
    let line = if line.starts_with('@') {
        line.split_once(' ').map_or("", |(_, s)| s)
    } else {
        line
    };
    let line = if line.starts_with(':') {
        line.split_once(' ').map_or("", |(_, s)| s)
    } else {
        line
    };
    line.split(' ').next().unwrap_or_default()
}
pub fn clean(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}
pub fn apply(messages: &mut VecDeque<Message>, change: Change) {
    match change {
        Change::Add(m) => {
            if !m.id.is_empty() && messages.iter().any(|old| old.id == m.id) {
                return;
            }
            if messages.len() >= HISTORY {
                messages.pop_front();
            }
            messages.push_back(m);
        }
        Change::Delete(id) => messages.retain(|m| m.id != id),
        Change::ClearUser(id) => messages.retain(|m| m.user_id != id),
        Change::Clear => messages.clear(),
    }
}
pub fn channel(input: &str) -> Result<String> {
    let input = input.trim();
    let value = if input.contains("://") {
        let url = Url::parse(input)?;
        if url.scheme() != "https" || !matches!(url.host_str(), Some("twitch.tv" | "www.twitch.tv"))
        {
            bail!("Enter a Twitch channel name or https://www.twitch.tv/channel URL.");
        }
        url.path().trim_matches('/').to_owned()
    } else {
        input.trim_start_matches('#').to_owned()
    };
    if value.is_empty()
        || value.len() > 25
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        bail!("Twitch channel names use 1–25 letters, numbers or underscores.");
    }
    Ok(value.to_ascii_lowercase())
}
pub fn video_id(input: &str) -> Result<String> {
    let input = input.trim();
    let value = if input.contains("://") {
        let url = Url::parse(input)?;
        if url.scheme() != "https" {
            bail!("Use an HTTPS YouTube video URL.");
        }
        match url.host_str() {
            Some("youtu.be" | "www.youtu.be") => url.path().trim_matches('/').to_owned(),
            Some("youtube.com" | "www.youtube.com" | "m.youtube.com") => {
                if let Some(value) = url
                    .query_pairs()
                    .find_map(|(k, v)| (k == "v").then(|| v.into_owned()))
                {
                    value
                } else if let Some(value) = url.path().strip_prefix("/live/") {
                    value.trim_end_matches('/').to_owned()
                } else {
                    bail!("Paste a live video URL, not a channel URL.");
                }
            }
            _ => bail!("Enter a YouTube live video URL or its 11-character video ID."),
        }
    } else {
        input.to_owned()
    };
    if value.len() != 11
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        bail!("YouTube video IDs contain 11 letters, numbers, underscores or hyphens.");
    }
    Ok(value)
}
fn unescape(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        if let Some(c) = chars.next() {
            result.push(match c {
                's' => ' ',
                ':' => ';',
                'r' => '\r',
                'n' => '\n',
                c => c,
            });
        }
    }
    result
}
pub fn irc(line: &str) -> Option<Change> {
    if line.len() > 16_384 {
        return None;
    }
    let (tags, rest) = if let Some(tagged) = line.strip_prefix('@') {
        tagged.split_once(' ')?
    } else {
        ("", line)
    };
    let tag = |key: &str| {
        tags.split(';').find_map(|t| {
            t.split_once('=')
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| unescape(v))
        })
    };
    let (prefix, rest) = if let Some(rest) = rest.strip_prefix(':') {
        rest.split_once(' ')?
    } else {
        ("", rest)
    };
    let (command, rest) = rest.split_once(' ').unwrap_or((rest, ""));
    let text = rest.split_once(" :").map_or("", |(_, text)| text);
    match command {
        "PRIVMSG" => Some(Change::Add(Message {
            id: clean(&tag("id").unwrap_or_default(), 128),
            user_id: clean(&tag("user-id").unwrap_or_default(), 128),
            author: clean(
                &tag("display-name")
                    .unwrap_or_else(|| prefix.split('!').next().unwrap_or("Viewer").into()),
                80,
            ),
            text: clean(
                text.strip_prefix("\u{1}ACTION ")
                    .unwrap_or(text)
                    .trim_end_matches('\u{1}'),
                2000,
            ),
            color: tag("color").and_then(|s| crate::chroma::parse_hex(&s).ok()),
        })),
        "CLEARMSG" => tag("target-msg-id").map(Change::Delete),
        "CLEARCHAT" => Some(tag("target-user-id").map_or(Change::Clear, Change::ClearUser)),
        _ => None,
    }
}
pub fn youtube(value: &Value) -> Vec<Change> {
    let Some(items) = value["items"].as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .take(2000)
        .filter_map(|item| {
            let s = &item["snippet"];
            match s["type"].as_str().unwrap_or_default() {
                "messageDeletedEvent" => Some(Change::Delete(
                    s["messageDeletedDetails"]["deletedMessageId"]
                        .as_str()?
                        .into(),
                )),
                "userBannedEvent" => Some(Change::ClearUser(
                    s["userBannedDetails"]["bannedUserDetails"]["channelId"]
                        .as_str()?
                        .into(),
                )),
                "tombstone" => Some(Change::Delete(item["id"].as_str()?.into())),
                _ => {
                    let text = s["displayMessage"].as_str()?;
                    Some(Change::Add(Message {
                        id: clean(item["id"].as_str().unwrap_or_default(), 256),
                        user_id: clean(
                            item["authorDetails"]["channelId"]
                                .as_str()
                                .unwrap_or_default(),
                            128,
                        ),
                        author: clean(
                            item["authorDetails"]["displayName"]
                                .as_str()
                                .unwrap_or("YouTube"),
                            80,
                        ),
                        text: clean(text, 2000),
                        color: None,
                    }))
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn targets_reject_injection_and_wrong_hosts() {
        assert_eq!(
            channel("https://www.twitch.tv/NekoUnix/").unwrap(),
            "nekounix"
        );
        for bad in [
            "x\r\nJOIN #other",
            "https://evil.test/a",
            "https://twitch.tv/a/b",
            "",
            "a b",
        ] {
            assert!(channel(bad).is_err(), "{bad}");
        }
        for value in [
            "https://youtu.be/abcdefghijk?t=2",
            "https://www.youtube.com/watch?v=abcdefghijk",
            "https://youtube.com/live/abcdefghijk",
        ] {
            assert_eq!(video_id(value).unwrap(), "abcdefghijk");
        }
        assert!(video_id("https://youtube.com.evil.test/watch?v=abcdefghijk").is_err());
        assert!(video_id("https://youtube.com/@example/live").is_err());
    }
    #[test]
    fn twitch_tags_duplicates_and_moderation() {
        let mut messages = VecDeque::new();
        let line = "@id=1;user-id=8;display-name=Test\\sUser;color=#AABBCC :test!test@tmi PRIVMSG #demo :Hi <script>!";
        assert_eq!(command(line), "PRIVMSG");
        assert_eq!(
            command(
                ":viewer!v@tmi PRIVMSG #demo :Please RECONNECT now NOTICE Login authentication failed"
            ),
            "PRIVMSG"
        );
        assert_eq!(command(":tmi.twitch.tv RECONNECT"), "RECONNECT");
        apply(&mut messages, irc(line).unwrap());
        apply(&mut messages, irc(line).unwrap());
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].author, "Test User");
        assert_eq!(messages[0].color, Some([170, 187, 204]));
        apply(
            &mut messages,
            irc("@target-msg-id=1 :tmi CLEARMSG #demo :Hi").unwrap(),
        );
        assert!(messages.is_empty());
        apply(&mut messages, irc(line).unwrap());
        apply(
            &mut messages,
            irc("@target-user-id=8 :tmi CLEARCHAT #demo :test").unwrap(),
        );
        assert!(messages.is_empty());
    }
    #[test]
    fn youtube_events_are_bounded_and_deleted() {
        let value = serde_json::json!({"items":[
            {"id":"a","snippet":{"type":"textMessageEvent","displayMessage":"Hello\nthere"},"authorDetails":{"channelId":"u","displayName":"Viewer"}},
            {"snippet":{"type":"messageDeletedEvent","messageDeletedDetails":{"deletedMessageId":"a"}}}
        ]});
        let mut messages = VecDeque::new();
        for change in youtube(&value) {
            apply(&mut messages, change);
        }
        assert!(messages.is_empty());
        for i in 0..500 {
            apply(
                &mut messages,
                Change::Add(Message {
                    id: i.to_string(),
                    user_id: String::new(),
                    author: String::new(),
                    text: String::new(),
                    color: None,
                }),
            );
        }
        assert_eq!(messages.len(), HISTORY);
        assert_eq!(messages[0].id, "300");
    }
}
