//! Petits utilitaires : dates locales (via l'API Intl du navigateur), couleurs, drapeaux.

use js_sys::{Date, Object, Reflect};
use wasm_bindgen::JsValue;

use crate::i18n::{is_fr, lang, t};

pub fn now_ms() -> f64 {
    Date::now()
}

pub fn parse_ms(iso: &str) -> f64 {
    Date::parse(iso)
}

fn options(pairs: &[(&str, &str)]) -> Object {
    let obj = Object::new();
    for (k, v) in pairs {
        let _ = Reflect::set(&obj, &JsValue::from_str(k), &JsValue::from_str(v));
    }
    obj
}

/// « ven. 2 oct. » ou « ven. 2 oct. · 10:30 », dans le fuseau horaire du visiteur.
pub fn local_date(iso: &str, with_time: bool) -> String {
    let date = Date::new(&JsValue::from_str(iso));
    if date.get_time().is_nan() {
        return iso.to_string();
    }
    let day = options(&[("weekday", "short"), ("day", "numeric"), ("month", "short")]);
    let mut out: String = date.to_locale_date_string(lang().locale(), &day).into();
    if with_time {
        let time = options(&[("hour", "2-digit"), ("minute", "2-digit")]);
        let t: String = date
            .to_locale_time_string_with_options("fr-FR", &time)
            .into();
        out.push_str(" · ");
        out.push_str(&t);
    }
    out
}

pub fn set_title(title: &str) {
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        if title.is_empty() {
            doc.set_title(t(
                "F1X — La Formule 1 dans ta poche",
                "F1X — Formula 1 in your pocket",
            ));
        } else {
            doc.set_title(&format!("{title} · F1X"));
        }
    }
}

pub fn team_style(constructor_id: &str) -> String {
    format!("--team:{}", team_color(constructor_id))
}

pub fn team_color(constructor_id: &str) -> &'static str {
    match constructor_id {
        "mercedes" => "#27F4D2",
        "ferrari" => "#E8002D",
        "red_bull" => "#3671C6",
        "mclaren" => "#FF8000",
        "aston_martin" => "#229971",
        "alpine" => "#FF87BC",
        "williams" => "#64C4FF",
        "rb" => "#6692FF",
        "haas" => "#B6BABD",
        "sauber" => "#52E252",
        "audi" => "#F50537",
        "cadillac" => "#C9A96E",
        _ => "#8A8A99",
    }
}

pub fn flag_country(country: &str) -> &'static str {
    match country {
        "Australia" => "🇦🇺",
        "Austria" => "🇦🇹",
        "Azerbaijan" => "🇦🇿",
        "Bahrain" => "🇧🇭",
        "Belgium" => "🇧🇪",
        "Brazil" => "🇧🇷",
        "Canada" => "🇨🇦",
        "China" => "🇨🇳",
        "France" => "🇫🇷",
        "Germany" => "🇩🇪",
        "Hungary" => "🇭🇺",
        "Italy" => "🇮🇹",
        "Japan" => "🇯🇵",
        "Malaysia" => "🇲🇾",
        "Mexico" => "🇲🇽",
        "Monaco" => "🇲🇨",
        "Netherlands" => "🇳🇱",
        "Portugal" => "🇵🇹",
        "Qatar" => "🇶🇦",
        "Saudi Arabia" => "🇸🇦",
        "Singapore" => "🇸🇬",
        "Spain" => "🇪🇸",
        "UAE" | "United Arab Emirates" => "🇦🇪",
        "UK" | "United Kingdom" => "🇬🇧",
        "USA" | "United States" => "🇺🇸",
        "Argentina" => "🇦🇷",
        "India" => "🇮🇳",
        "Korea" => "🇰🇷",
        "Morocco" => "🇲🇦",
        "Russia" => "🇷🇺",
        "South Africa" => "🇿🇦",
        "Sweden" => "🇸🇪",
        "Switzerland" => "🇨🇭",
        "Turkey" => "🇹🇷",
        "Vietnam" => "🇻🇳",
        _ => "🏁",
    }
}

pub fn flag_nationality(nationality: Option<&str>) -> &'static str {
    match nationality.unwrap_or_default() {
        "American" => "🇺🇸",
        "Argentine" | "Argentinian" => "🇦🇷",
        "Australian" => "🇦🇺",
        "Austrian" => "🇦🇹",
        "Belgian" => "🇧🇪",
        "Brazilian" => "🇧🇷",
        "British" => "🇬🇧",
        "Canadian" => "🇨🇦",
        "Chinese" => "🇨🇳",
        "Danish" => "🇩🇰",
        "Dutch" => "🇳🇱",
        "Finnish" => "🇫🇮",
        "French" => "🇫🇷",
        "German" => "🇩🇪",
        "Italian" => "🇮🇹",
        "Japanese" => "🇯🇵",
        "Mexican" => "🇲🇽",
        "Monegasque" => "🇲🇨",
        "New Zealander" => "🇳🇿",
        "Polish" => "🇵🇱",
        "Spanish" => "🇪🇸",
        "Swiss" => "🇨🇭",
        "Thai" => "🇹🇭",
        "Indian" => "🇮🇳",
        "Irish" => "🇮🇪",
        "Hungarian" => "🇭🇺",
        "Indonesian" => "🇮🇩",
        "Malaysian" => "🇲🇾",
        "Portuguese" => "🇵🇹",
        "Russian" => "🇷🇺",
        "Swedish" => "🇸🇪",
        "South African" => "🇿🇦",
        "Venezuelan" => "🇻🇪",
        "Colombian" => "🇨🇴",
        "Chilean" => "🇨🇱",
        "Uruguayan" => "🇺🇾",
        "Czech" => "🇨🇿",
        "Liechtensteiner" => "🇱🇮",
        "Rhodesian" => "🇿🇼",
        "Hong Kong" => "🇭🇰",
        "East German" => "🇩🇪",
        _ => "🏳️",
    }
}

pub fn wins_label(wins: &str) -> String {
    match (wins, is_fr()) {
        ("0", _) => String::new(),
        ("1", true) => "1 victoire".into(),
        ("1", false) => "1 win".into(),
        (n, true) => format!("{n} victoires"),
        (n, false) => format!("{n} wins"),
    }
}

/// Minuscule sans accents, pour la recherche (« Räikkönen » → « raikkonen »).
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' | 'č' | 'ć' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'ě' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ñ' | 'ń' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'ů' => 'u',
            'ý' | 'ÿ' => 'y',
            'š' | 'ś' => 's',
            'ž' | 'ź' | 'ż' => 'z',
            'ł' => 'l',
            'ř' => 'r',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// « 1985-01-07 » → « 7 janvier 1985 » / « 7 January 1985 ».
pub fn format_birth(date: &str) -> String {
    if !is_fr() {
        const EN: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        return match date.split('-').collect::<Vec<_>>().as_slice() {
            [y, m, d] => {
                let month = m
                    .parse::<usize>()
                    .ok()
                    .and_then(|m| EN.get(m.wrapping_sub(1)))
                    .copied()
                    .unwrap_or(m);
                format!("{} {month} {y}", d.trim_start_matches('0'))
            }
            _ => date.to_string(),
        };
    }
    const MONTHS: [&str; 12] = [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ];
    let parts: Vec<&str> = date.split('-').collect();
    match parts.as_slice() {
        [y, m, d] => {
            let month = m
                .parse::<usize>()
                .ok()
                .and_then(|m| MONTHS.get(m.wrapping_sub(1)))
                .copied()
                .unwrap_or(m);
            let day = d.trim_start_matches('0');
            format!("{} {month} {y}", if day == "1" { "1er" } else { day })
        }
        _ => date.to_string(),
    }
}

/// Âge en années à partir d'une date de naissance ISO.
pub fn age(date: &str) -> Option<u32> {
    let birth = Date::new(&JsValue::from_str(date));
    if birth.get_time().is_nan() {
        return None;
    }
    let now = Date::new_0();
    let mut years = now.get_full_year() as i32 - birth.get_full_year() as i32;
    if (now.get_month(), now.get_date()) < (birth.get_month(), birth.get_date()) {
        years -= 1;
    }
    u32::try_from(years).ok()
}

pub fn current_year() -> u32 {
    Date::new_0().get_full_year()
}

/// Nom du pays dans la langue courante (l'API l'écrit en anglais).
pub fn country_fr(country: &str) -> &str {
    if !is_fr() {
        return match country {
            "UK" => "United Kingdom",
            "USA" => "United States",
            "UAE" => "United Arab Emirates",
            "Korea" => "South Korea",
            other => other,
        };
    }
    match country {
        "Argentina" => "Argentine",
        "Australia" => "Australie",
        "Austria" => "Autriche",
        "Azerbaijan" => "Azerbaïdjan",
        "Bahrain" => "Bahreïn",
        "Belgium" => "Belgique",
        "Brazil" => "Brésil",
        "China" => "Chine",
        "Germany" => "Allemagne",
        "Hungary" => "Hongrie",
        "India" => "Inde",
        "Italy" => "Italie",
        "Japan" => "Japon",
        "Korea" => "Corée du Sud",
        "Malaysia" => "Malaisie",
        "Mexico" => "Mexique",
        "Morocco" => "Maroc",
        "Netherlands" => "Pays-Bas",
        "Russia" => "Russie",
        "Saudi Arabia" => "Arabie saoudite",
        "Singapore" => "Singapour",
        "South Africa" => "Afrique du Sud",
        "Spain" => "Espagne",
        "Sweden" => "Suède",
        "Switzerland" => "Suisse",
        "Turkey" => "Turquie",
        "UAE" | "United Arab Emirates" => "Émirats arabes unis",
        "UK" | "United Kingdom" => "Royaume-Uni",
        "USA" | "United States" => "États-Unis",
        "Vietnam" => "Viêt Nam",
        other => other,
    }
}

/// Nom d'une session du week-end (clé française venant de `Race::sessions`).
pub fn session_label(name: &str) -> &str {
    if is_fr() {
        return name;
    }
    match name {
        "Essais libres 1" => "Practice 1",
        "Essais libres 2" => "Practice 2",
        "Essais libres 3" => "Practice 3",
        "Qualifs sprint" => "Sprint qualifying",
        "Qualifications" => "Qualifying",
        "Course" => "Race",
        other => other,
    }
}

// ---------- Données personnelles (stockées sur l'appareil) ----------

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// Lit une valeur JSON enregistrée dans le navigateur.
pub fn load<T: serde::de::DeserializeOwned>(key: &str) -> Option<T> {
    let text = local_storage()?.get_item(key).ok().flatten()?;
    serde_json::from_str(&text).ok()
}

/// Enregistre une valeur JSON dans le navigateur.
pub fn store<T: serde::Serialize>(key: &str, value: &T) {
    if let (Some(s), Ok(text)) = (local_storage(), serde_json::to_string(value)) {
        let _ = s.set_item(key, &text);
    }
}

pub fn fav_driver() -> Option<String> {
    load("f1x-fav-driver")
}

pub fn fav_team() -> Option<String> {
    load("f1x-fav-team")
}

/// Télécharge un fichier CSV (séparateur « ; », compatible Excel FR / Numbers).
pub fn download_csv(filename: &str, rows: &[Vec<String>]) {
    use wasm_bindgen::JsCast;
    let esc = |c: &String| {
        if c.contains([';', '"', '\n']) {
            format!("\"{}\"", c.replace('"', "\"\""))
        } else {
            c.clone()
        }
    };
    let text = String::from("\u{feff}")
        + &rows
            .iter()
            .map(|r| r.iter().map(esc).collect::<Vec<_>>().join(";"))
            .collect::<Vec<_>>()
            .join("\n");
    let parts = js_sys::Array::of1(&JsValue::from_str(&text));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("text/csv;charset=utf-8");
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    let doc = web_sys::window().and_then(|w| w.document());
    if let Some(a) = doc
        .and_then(|d| d.create_element("a").ok())
        .and_then(|e| e.dyn_into::<web_sys::HtmlAnchorElement>().ok())
    {
        a.set_href(&url);
        a.set_download(filename);
        a.click();
    }
    let _ = web_sys::Url::revoke_object_url(&url);
}

/// Nombre aléatoire dans [0, n).
pub fn random(n: usize) -> usize {
    ((js_sys::Math::random() * n as f64) as usize).min(n.saturating_sub(1))
}
