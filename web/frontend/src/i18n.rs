//! Français / English.
//!
//! La langue courante vit dans un `thread_local` (le WebAssembly est mono-thread) ; changer de
//! langue ([`toggle`]) modifie l'état lu par le routeur, qui reconstruit la page : tout relit
//! alors la nouvelle langue.
//! Les textes sont écrits en paires : `t("Calendrier", "Calendar")`, `tr!("Saison {s}", "Season {s}")`.

use std::cell::Cell;

use active::State;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::En => "en",
        }
    }

    /// Locale utilisée pour les dates (API `Intl` du navigateur).
    pub fn locale(self) -> &'static str {
        match self {
            Lang::Fr => "fr-FR",
            Lang::En => "en-GB",
        }
    }

    pub fn other(self) -> Lang {
        match self {
            Lang::Fr => Lang::En,
            Lang::En => Lang::Fr,
        }
    }
}

thread_local! {
    static LANG: Cell<Lang> = const { Cell::new(Lang::Fr) };
    /// État de la langue, lu par le routeur (créé par [`init`]).
    static LANG_STATE: Cell<Option<State<Lang>>> = const { Cell::new(None) };
}

/// Crée l'état de la langue (langue enregistrée, sinon celle du navigateur).
pub fn init() -> State<Lang> {
    let state = active::use_state(initial());
    LANG_STATE.with(|s| s.set(Some(state)));
    state
}

/// Passe à l'autre langue et la mémorise : la page se reconstruit dans cette langue.
pub fn toggle() {
    let next = lang().other();
    save(next);
    apply(next);
    if let Some(state) = LANG_STATE.with(Cell::get) {
        state.set(next);
    }
}

const STORAGE_KEY: &str = "f1x-lang";

pub fn lang() -> Lang {
    LANG.with(Cell::get)
}

pub fn is_fr() -> bool {
    lang() == Lang::Fr
}

/// Choisit le texte dans la langue courante.
pub fn t(fr: &'static str, en: &'static str) -> &'static str {
    if is_fr() { fr } else { en }
}

/// Applique la langue (sans l'enregistrer).
pub fn apply(l: Lang) {
    LANG.with(|c| c.set(l));
    if let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    {
        let _ = root.set_attribute("lang", l.code());
    }
}

/// Mémorise le choix de langue dans le navigateur.
pub fn save(l: Lang) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(STORAGE_KEY, l.code());
    }
}

/// Langue enregistrée, sinon celle du téléphone / navigateur (français par défaut pour « fr-* »).
pub fn initial() -> Lang {
    let window = web_sys::window();
    let stored = window
        .as_ref()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten());
    let code = stored
        .or_else(|| window.and_then(|w| w.navigator().language()))
        .unwrap_or_default();
    if code.starts_with("en") || (!code.is_empty() && !code.starts_with("fr")) {
        Lang::En
    } else {
        Lang::Fr
    }
}

/// `format!` bilingue : `tr!("Saison {s}", "Season {s}")`, arguments positionnels partagés.
#[macro_export]
macro_rules! tr {
    ($fr:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        if $crate::i18n::is_fr() { format!($fr $(, $arg)*) } else { format!($en $(, $arg)*) }
    };
}
