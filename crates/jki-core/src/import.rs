use crate::{Account, AccountType};
use url::Url;

pub fn parse_otpauth_uri(uri: &str) -> Option<Account> {
    let url = Url::parse(uri).ok()?;
    if url.scheme() != "otpauth" { return None; }

    let host = url.host_str()?; // totp
    if host != "totp" { return None; }

    // Path is /Label or /Issuer:Label
    let path = url.path().trim_start_matches('/');
    let (issuer_raw, name_raw) = if let Some(pos) = path.find(':') {
        (Some(path[..pos].to_string()), path[pos+1..].to_string())
    } else {
        (None, path.to_string())
    };

    let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    let secret = query.get("secret")?.clone();
    let digits = query.get("digits").and_then(|d| d.parse::<u32>().ok()).unwrap_or(6);
    let issuer_query = query.get("issuer").cloned();
    
    let issuer = issuer_raw.and_then(|s| {
        urlencoding::decode(&s.replace('+', " ")).ok().map(|d| d.into_owned())
    });
    let name = urlencoding::decode(&name_raw.replace('+', " ")).ok()?.into_owned();

    let effective_issuer = issuer.clone().or(issuer_query);
    let account_type = if effective_issuer.as_deref() == Some("Steam") {
        AccountType::Steam
    } else if effective_issuer.as_deref() == Some("BattleNet") {
        AccountType::Blizzard
    } else {
        AccountType::Standard
    };

    Some(Account {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        issuer: effective_issuer,
        account_type,
        secret,
        digits,
        algorithm: "SHA1".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_otpauth() {
        let uri = "otpauth://totp/Google:test@gmail.com?secret=JBSWY3DPEHPK3PXP&issuer=Google";
        let acc = parse_otpauth_uri(uri).unwrap();
        assert_eq!(acc.name, "test@gmail.com");
        assert_eq!(acc.issuer, Some("Google".to_string()));
        assert_eq!(acc.secret, "JBSWY3DPEHPK3PXP");
        assert_eq!(acc.account_type, AccountType::Standard);
    }

    #[test]
    fn test_parse_steam_otpauth() {
        let uri = "otpauth://totp/Steam:username?secret=JBSWY3DPEHPK3PXP&issuer=Steam";
        let acc = parse_otpauth_uri(uri).unwrap();
        assert_eq!(acc.name, "username");
        assert_eq!(acc.issuer, Some("Steam".to_string()));
        assert_eq!(acc.account_type, AccountType::Steam);
    }

    #[test]
    fn test_parse_blizzard_otpauth() {
        let uri = "otpauth://totp/BattleNet:username?secret=JBSWY3DPEHPK3PXP&issuer=BattleNet";
        let acc = parse_otpauth_uri(uri).unwrap();
        assert_eq!(acc.name, "username");
        assert_eq!(acc.issuer, Some("BattleNet".to_string()));
        assert_eq!(acc.account_type, AccountType::Blizzard);
    }

    #[test]
    fn test_parse_uri_with_encoding() {
        let uri = "otpauth://totp/My%20Service:user%20name?secret=JBSWY3DPEHPK3PXP";
        let acc = parse_otpauth_uri(uri).unwrap();
        assert_eq!(acc.name, "user name");
        assert_eq!(acc.issuer, Some("My Service".to_string()));

        let uri2 = "otpauth://totp/Service:user%2Bname?secret=123";
        let acc2 = parse_otpauth_uri(uri2).unwrap();
        assert_eq!(acc2.name, "user+name");
    }

    #[test]
    fn test_parse_uri_with_plus_sign() {
        // WinAuth style: '+' represents space in the path
        let uri = "otpauth://totp/FF14+Service:user+name?secret=123";
        let acc = parse_otpauth_uri(uri).unwrap();
        assert_eq!(acc.name, "user name");
        assert_eq!(acc.issuer, Some("FF14 Service".to_string()));
    }

    #[test]
    fn test_parse_invalid_uri() {
        assert!(parse_otpauth_uri("invalid").is_none());
        assert!(parse_otpauth_uri("otpauth://hotp/test?secret=123").is_none());
    }
}
