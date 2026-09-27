//! Publisher registration is supplied at build time, never requested from streamers.
use super::Account;

pub fn client_id(index: usize) -> &'static str {
    match index {
        0 => option_env!("ARIA_TWITCH_CLIENT_ID").unwrap_or(""),
        1 => option_env!("ARIA_GOOGLE_CLIENT_ID").unwrap_or(""),
        _ => "",
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.chars().any(char::is_whitespace)
}

pub fn available(index: usize, account: &Account) -> bool {
    valid_id(if account.client_id.is_empty() {
        client_id(index)
    } else {
        &account.client_id
    })
}

pub fn resolve(index: usize, account: &Account) -> anyhow::Result<Account> {
    let mut account = account.clone();
    // Retain existing registered installations and their encrypted sessions.
    if account.client_id.is_empty() {
        account.client_id = client_id(index).into();
    }
    anyhow::ensure!(
        valid_id(&account.client_id),
        "Website sign-in is not available in this build yet. ARIA's publisher must enable it; you do not need to create an API app."
    );
    Ok(account)
}

pub fn google_desktop_secret(id: &str) -> Option<&'static str> {
    // Google's installed-app client value is distributable, not a confidential web secret.
    (!id.is_empty() && id == client_id(1))
        .then_some(option_env!("ARIA_GOOGLE_DESKTOP_CLIENT_SECRET"))
        .flatten()
        .filter(|secret| !secret.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_registration_and_session_are_preserved() {
        let account = Account {
            client_id: "existing-client".into(),
            sealed_session: "sealed".into(),
            ..Default::default()
        };
        let resolved = resolve(0, &account).unwrap();
        assert_eq!(resolved.client_id, account.client_id);
        assert_eq!(resolved.sealed_session, account.sealed_session);
        assert!(
            resolve(
                0,
                &Account {
                    client_id: "bad id".into(),
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert!(google_desktop_secret("unrelated-client").is_none());
    }
}
