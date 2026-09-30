//! Petits utilitaires : dates locales (via l'API Intl du navigateur), couleurs, drapeaux.

use js_sys::{Date, Object, Reflect};
use wasm_bindgen::JsValue;

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
    let mut out: String = date.to_locale_date_string("fr-FR", &day).into();
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
            doc.set_title("F1X — La Formule 1 dans ta poche");
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
    match wins {
        "0" => String::new(),
        "1" => "1 victoire".into(),
        n => format!("{n} victoires"),
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

/// « 1985-01-07 » → « 7 janvier 1985 ».
pub fn format_birth(date: &str) -> String {
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
