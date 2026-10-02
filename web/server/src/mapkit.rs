//! Jeton MapKit JS (Apple Maps sur le site), signé avec une clé MapKit du compte développeur
//! Apple. Configuration (variables d'environnement) :
//! - `MAPKIT_KEY_ID` : identifiant de la clé (Keys → MapKit JS) ;
//! - `MAPKIT_TEAM_ID` (ou `APPLE_TEAM_ID`) : identifiant d'équipe ;
//! - `MAPKIT_KEY` : contenu du fichier `.p8` (PEM, ou base64 du PEM).
//!
//! Sans configuration, le site garde la carte OpenStreetMap.

use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

fn var(k: &str) -> Option<String> {
    std::env::var(k)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Jeton valable 30 minutes, limité à l'origine du site.
pub fn token(origin: &str) -> Option<String> {
    let key_id = var("MAPKIT_KEY_ID")?;
    let team = var("MAPKIT_TEAM_ID").or_else(|| var("APPLE_TEAM_ID"))?;
    let raw = var("MAPKIT_KEY")?;
    let pem = if raw.contains("BEGIN PRIVATE KEY") {
        raw.replace("\\n", "\n")
    } else {
        use std::io::Read;
        let mut out = String::new();
        let bytes = base64_decode(&raw)?;
        std::io::Cursor::new(bytes).read_to_string(&mut out).ok()?;
        out
    };
    let key = EncodingKey::from_ec_pem(pem.as_bytes()).ok()?;
    let mut header = Header::new(Algorithm::ES256);
    header.kid = Some(key_id);
    header.typ = Some("JWT".into());
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "iss": team,
        "iat": now,
        "exp": now + 1800,
        "origin": origin,
    });
    encode(&header, &claims, &key).ok()
}

/// Décodage base64 standard (pour une clé collée sur une seule ligne).
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        let v = ABC.iter().position(|&a| a == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_base64() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
    }
}
