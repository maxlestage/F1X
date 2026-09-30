//! Lexique de la F1 (français / anglais), avec recherche.

use web_sys::HtmlInputElement;
use yew::prelude::*;

use crate::components::*;
use crate::i18n::{is_fr, t};
use crate::tr;
use crate::util::fold;

/// (terme FR, terme EN, définition FR, définition EN)
const ENTRIES: &[(&str, &str, &str, &str)] = &[
    (
        "Pole position",
        "Pole position",
        "Première place sur la grille de départ, obtenue par le plus rapide des qualifications.",
        "First place on the starting grid, earned by the fastest driver in qualifying.",
    ),
    (
        "Qualifications (Q1, Q2, Q3)",
        "Qualifying (Q1, Q2, Q3)",
        "Trois séances éliminatoires : les plus lents sont éliminés en Q1 puis en Q2, les 10 meilleurs se battent pour la pole en Q3.",
        "Three knockout sessions: the slowest drop out in Q1 and Q2, the top 10 fight for pole in Q3.",
    ),
    (
        "Sprint",
        "Sprint",
        "Course courte (~100 km) disputée le samedi de certains week-ends, avec ses propres qualifications et des points pour les 8 premiers.",
        "Short race (~100 km) held on Saturday at some events, with its own qualifying and points for the top 8.",
    ),
    (
        "Barème des points",
        "Points system",
        "Course : 25, 18, 15, 12, 10, 8, 6, 4, 2, 1 pour les 10 premiers. Sprint : 8 à 1 pour les 8 premiers. Le point du meilleur tour a été supprimé en 2025.",
        "Race: 25, 18, 15, 12, 10, 8, 6, 4, 2, 1 for the top 10. Sprint: 8 to 1 for the top 8. The fastest-lap point was dropped in 2025.",
    ),
    (
        "Tour de formation",
        "Formation lap",
        "Tour lent avant le départ pour chauffer pneus et freins, puis les voitures reprennent leur place sur la grille.",
        "Slow lap before the start to warm tyres and brakes, after which cars return to their grid slots.",
    ),
    (
        "Voiture de sécurité (SC)",
        "Safety Car (SC)",
        "Voiture qui neutralise la course après un incident : les pilotes roulent en file derrière elle sans dépasser, les écarts se resserrent.",
        "Car that neutralises the race after an incident: drivers queue up behind it without overtaking, gaps close up.",
    ),
    (
        "Voiture de sécurité virtuelle (VSC)",
        "Virtual Safety Car (VSC)",
        "Neutralisation sans voiture sur la piste : chaque pilote doit respecter un temps minimal, les écarts sont à peu près conservés.",
        "Neutralisation without a car on track: every driver must respect a minimum time, gaps are roughly kept.",
    ),
    (
        "Drapeau jaune",
        "Yellow flag",
        "Danger sur le côté de la piste : ralentir, interdiction de dépasser dans la zone. Double jaune : danger sur la piste, prêt à s'arrêter.",
        "Hazard beside the track: slow down, no overtaking in that zone. Double yellow: hazard on track, be ready to stop.",
    ),
    (
        "Drapeau rouge",
        "Red flag",
        "Course interrompue : toutes les voitures rentrent lentement dans la voie des stands.",
        "Session stopped: every car returns slowly to the pit lane.",
    ),
    (
        "Drapeau bleu",
        "Blue flag",
        "Montré à un pilote qui va être doublé : il doit laisser passer la voiture plus rapide.",
        "Shown to a driver about to be lapped: they must let the faster car through.",
    ),
    (
        "Drapeau à damier",
        "Chequered flag",
        "Fin de la séance ou de la course.",
        "End of the session or race.",
    ),
    (
        "Drapeau noir / noir et blanc",
        "Black / black-and-white flag",
        "Noir : pilote disqualifié. Noir et blanc : avertissement pour conduite antisportive.",
        "Black: driver disqualified. Black and white: warning for unsporting driving.",
    ),
    (
        "Arrêt aux stands",
        "Pit stop",
        "Passage au garage pendant la course pour changer les pneus (environ 2 s à l'arrêt, ~20 s perdues au total avec la voie des stands).",
        "Stop at the garage during the race to change tyres (about 2 s stationary, ~20 s lost overall including the pit lane).",
    ),
    (
        "Undercut",
        "Undercut",
        "S'arrêter avant un rival pour profiter de pneus neufs plus rapides et ressortir devant lui après son propre arrêt.",
        "Pitting before a rival to use faster fresh tyres and come out ahead after they stop.",
    ),
    (
        "Overcut",
        "Overcut",
        "Rester en piste plus longtemps qu'un rival pour gagner du temps en air libre, puis ressortir devant après son arrêt.",
        "Staying out longer than a rival to gain time in clean air, then emerging ahead after stopping.",
    ),
    (
        "Gommes (tendre, medium, dur)",
        "Compounds (soft, medium, hard)",
        "Pneus secs : tendre (rouge) = rapide mais s'use vite, medium (jaune) = compromis, dur (blanc) = lent mais durable. Au moins deux gommes différentes en course sur le sec.",
        "Dry tyres: soft (red) = fast but wears quickly, medium (yellow) = balance, hard (white) = slower but durable. At least two different compounds in a dry race.",
    ),
    (
        "Intermédiaires et pluie",
        "Intermediates and wets",
        "Pneus verts pour piste humide, bleus pour forte pluie : ils évacuent l'eau pour éviter l'aquaplaning.",
        "Green tyres for a damp track, blue ones for heavy rain: they clear water to avoid aquaplaning.",
    ),
    (
        "Dégradation",
        "Degradation",
        "Perte de performance des pneus au fil des tours (usure, surchauffe) : elle dicte la stratégie d'arrêts.",
        "Loss of tyre performance over the laps (wear, overheating): it drives pit stop strategy.",
    ),
    (
        "Graining",
        "Graining",
        "Petits morceaux de gomme qui se détachent et se recollent sur le pneu, qui glisse davantage.",
        "Small pieces of rubber that tear off and stick back to the tyre, making it slide more.",
    ),
    (
        "Blistering",
        "Blistering",
        "Cloques dues à une surchauffe à l'intérieur du pneu.",
        "Blisters caused by overheating inside the tyre.",
    ),
    (
        "Secteurs",
        "Sectors",
        "Le tour est découpé en trois secteurs chronométrés. Violet : meilleur temps de tous, vert : record personnel, jaune : plus lent.",
        "The lap is split into three timed sectors. Purple: overall best, green: personal best, yellow: slower.",
    ),
    (
        "Écart (gap) et intervalle",
        "Gap and interval",
        "L'écart est la distance en secondes avec le leader, l'intervalle avec la voiture juste devant.",
        "The gap is the distance in seconds to the leader, the interval to the car just ahead.",
    ),
    (
        "Doublé (lapped)",
        "Lapped",
        "Pilote rattrapé et dépassé par le leader : il a un tour (ou plus) de retard.",
        "Driver caught and passed by the leader: one lap (or more) down.",
    ),
    (
        "DRS (jusqu'en 2025)",
        "DRS (until 2025)",
        "Aileron arrière ouvrant, autorisé à moins d'une seconde de la voiture de devant dans certaines zones, pour faciliter les dépassements. Remplacé en 2026 par l'aérodynamique active et le mode dépassement.",
        "Opening rear wing, allowed within one second of the car ahead in set zones to help overtaking. Replaced in 2026 by active aero and overtake mode.",
    ),
    (
        "Aérodynamique active (2026)",
        "Active aerodynamics (2026)",
        "Depuis 2026, les ailerons avant et arrière changent de position : faible traînée en ligne droite, appui maximal en courbe.",
        "Since 2026, front and rear wings change position: low drag on straights, maximum downforce in corners.",
    ),
    (
        "Mode dépassement (2026)",
        "Overtake mode (2026)",
        "Surplus d'énergie électrique qu'un pilote proche de la voiture de devant peut utiliser pour tenter un dépassement.",
        "Extra electrical energy a driver close to the car ahead can use to attempt an overtake.",
    ),
    (
        "Unité de puissance",
        "Power unit",
        "Moteur V6 turbo hybride. Depuis 2026 : environ moitié thermique, moitié électrique, avec carburant 100 % durable.",
        "Hybrid turbo V6 engine. Since 2026: roughly half combustion, half electric, running on 100% sustainable fuel.",
    ),
    (
        "ERS / MGU-K",
        "ERS / MGU-K",
        "Système de récupération d'énergie : le MGU-K récupère l'énergie au freinage et la restitue en accélération.",
        "Energy recovery system: the MGU-K harvests energy under braking and releases it under acceleration.",
    ),
    (
        "Effet de sol",
        "Ground effect",
        "Appui aérodynamique créé sous la voiture par le fond plat et ses tunnels, qui la plaque au sol.",
        "Downforce generated under the car by the floor and its tunnels, sucking it to the ground.",
    ),
    (
        "Appui aérodynamique",
        "Downforce",
        "Force qui plaque la voiture au sol pour aller plus vite en courbe, au prix de plus de traînée en ligne droite.",
        "Force pressing the car down to go faster through corners, at the cost of more drag on straights.",
    ),
    (
        "Parc fermé",
        "Parc fermé",
        "Après les qualifications, les réglages de la voiture sont gelés jusqu'à la course (sauf exceptions encadrées).",
        "After qualifying, car setups are frozen until the race (with limited exceptions).",
    ),
    (
        "Pénalités",
        "Penalties",
        "5 ou 10 secondes ajoutées au temps (ou purgées au stand), drive-through (traverser la voie des stands), stop-and-go (s'arrêter 10 s), recul sur la grille.",
        "5 or 10 seconds added (or served at a stop), drive-through (pass through the pit lane), stop-and-go (stop for 10 s), grid drop.",
    ),
    (
        "Limites de piste",
        "Track limits",
        "Les quatre roues ne doivent pas franchir entièrement les lignes blanches : sinon tour annulé, puis pénalité en cas de récidive.",
        "All four wheels may not fully cross the white lines: otherwise the lap is deleted, then a penalty for repeat offences.",
    ),
    (
        "Pilote de réserve",
        "Reserve driver",
        "Pilote prêt à remplacer un titulaire malade ou blessé.",
        "Driver ready to replace an ill or injured race driver.",
    ),
    (
        "Plafond budgétaire",
        "Cost cap",
        "Limite de dépenses annuelle imposée aux écuries pour réduire les écarts entre équipes riches et modestes.",
        "Annual spending limit imposed on teams to narrow the gap between rich and smaller outfits.",
    ),
    (
        "Championnat constructeurs",
        "Constructors' championship",
        "Classement des écuries : addition des points de leurs deux pilotes. Il existe depuis 1958.",
        "Team standings: the sum of both drivers' points. It has existed since 1958.",
    ),
];

#[function_component]
pub fn GlossaryPage() -> Html {
    let query = use_state(String::new);
    let q = fold(&query);
    let oninput = {
        let query = query.clone();
        Callback::from(move |e: InputEvent| {
            query.set(e.target_unchecked_into::<HtmlInputElement>().value())
        })
    };
    let fr = is_fr();
    let list: Vec<(&str, &str)> = ENTRIES
        .iter()
        .map(|(tf, te, df, de)| if fr { (*tf, *df) } else { (*te, *de) })
        .filter(|(term, def)| q.is_empty() || fold(term).contains(&q) || fold(def).contains(&q))
        .collect();
    html! {
        <Layout title={t("Lexique", "Glossary")} tab={Tab::Archives}>
            <input class="search" type="search" value={(*query).clone()} {oninput}
                   placeholder={t("Rechercher (drapeau, undercut, DRS…)", "Search (flag, undercut, DRS…)")} aria-label={t("Rechercher", "Search")} />
            <p class="section-intro">{ tr!("{} définitions", "{} definitions", list.len()) }</p>
            <dl class="glossary">
                { for list.iter().map(|(term, def)| html! {
                    <div class="card"><dt>{ *term }</dt><dd>{ *def }</dd></div>
                }) }
            </dl>
        </Layout>
    }
}
