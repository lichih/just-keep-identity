use crate::{Account, AccountType};
use url::Url;

pub fn parse_otpauth_uri(uri: &str) -> Option<Account> {
    let url = Url::parse(uri).ok()?;
    if url.scheme() != "otpauth" { return None; }

    let host = url.host_str()?; // totp
    if host != "totp" { return None; }

    // Path is /Label or /Issuer:Label
    let path = url.path().trim_start_matches('/');
    let (issuer, name) = if let Some(pos) = path.find(':') {
        (Some(path[..pos].to_string()), path[pos+1..].to_string())
    } else {
        (None, path.to_string())
    };

    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    let secret = query.get("secret")?.clone();
    let digits = query.get("digits").and_then(|d| d.parse::<u32>().ok()).unwrap_or(6);
    let issuer_from_query = query.get("issuer").cloned();
    
    let account_type = if issuer.as_deref() == Some("Steam") || issuer_from_query.as_deref() == Some("Steam") {
        AccountType::Steam
    } else if issuer.as_deref() == Some("BattleNet") || issuer_from_query.as_deref() == Some("BattleNet") {
        AccountType::Blizzard
    } else {
        AccountType::Standard
    };

    Some(Account {
        name: name.replace('+', " "), // Unescape spaces from WinAuth format
        issuer: issuer.or(issuer_from_query),
        secret,
        digits,
        algorithm: "SHA1".to_string(), // Default for TOTP
        account_type,
    })
}
