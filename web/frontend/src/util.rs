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
