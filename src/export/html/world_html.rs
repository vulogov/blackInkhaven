//! TDOC-4.5 — a bespoke `WorldDefinition` → narrative-prose renderer. Rather than
//! dumping the world's declaration as field lists, this writes it up as a readable
//! guide: the sky, the land, the waters, the peoples, their livelihood, the laws of
//! magic, and the history — in sentences, skipping whatever the author left blank.

use crate::world::types::WorldDefinition;

use super::markdown_html::escape_html;

/// Render a world as an HTML narrative.
pub fn render(def: &WorldDefinition) -> String {
    let name = escape_html(&def.name);
    let mut s = format!("<h1>{}</h1>\n", if name.is_empty() { "The World".into() } else { name.clone() });

    sky(&mut s, &name, def);
    land(&mut s, &name, def);
    waters(&mut s, def);
    peoples(&mut s, def);
    livelihood(&mut s, def);
    life(&mut s, def);
    magic(&mut s, def);
    history(&mut s, def);
    s
}

fn sky(s: &mut String, name: &str, def: &WorldDefinition) {
    let a = &def.astronomy;
    s.push_str("<h2>The sky</h2>\n<p>");
    s.push_str(&format!("<strong>{}</strong> turns beneath ", pick_name(name)));
    if a.star.class.trim().is_empty() {
        s.push_str("its star");
    } else {
        s.push_str(&format!("{} {} star", article(&a.star.class), escape_html(a.star.class.trim())));
    }
    if a.star.luminosity_solar > 0.0 {
        s.push_str(&format!(", some {} times as luminous as our own sun", num(a.star.luminosity_solar)));
    }
    if a.star.age_gyr > 0.0 {
        s.push_str(&format!(", and about {} billion years old", num(a.star.age_gyr)));
    }
    s.push_str(".</p>\n<p>");
    s.push_str(&format!("A day lasts {} hours", num(a.planet.day_length_hours)));
    if a.planet.axial_tilt_deg > 0.0 {
        s.push_str(&format!(", and the world leans {}° on its axis, so it keeps its seasons", num(a.planet.axial_tilt_deg)));
    }
    s.push('.');
    if let Some(y) = a.orbit.year_length_days {
        s.push_str(&format!(" Its year runs {} days.", num(y)));
    }
    s.push_str("</p>\n");
    if !a.moons.is_empty() {
        let names: Vec<String> = a.moons.iter().map(|m| format!("<em>{}</em>", escape_html(&m.name))).collect();
        s.push_str(&format!(
            "<p>{} cross its sky: {}.</p>\n",
            count(a.moons.len(), "moon", "moons"),
            join(&names)
        ));
    }
}

fn land(s: &mut String, name: &str, def: &WorldDefinition) {
    let Some(geo) = &def.geography else { return };
    if geo.regions.is_empty() && geo.landmarks.is_empty() {
        return;
    }
    s.push_str("<h2>The land</h2>\n");
    if !geo.regions.is_empty() {
        s.push_str(&format!(
            "<p>{} is divided into {}.</p>\n",
            pick_name(name),
            count(geo.regions.len(), "region", "regions")
        ));
        for r in &geo.regions {
            s.push_str(&format!("<p><strong>{}</strong>", escape_html(&r.name)));
            let mut tags = Vec::new();
            if !r.biome.trim().is_empty() {
                tags.push(escape_html(r.biome.trim()));
            }
            if !r.climate.trim().is_empty() {
                tags.push(escape_html(r.climate.trim()));
            }
            if !tags.is_empty() {
                s.push_str(&format!(" — {}", tags.join(", ")));
            }
            s.push('.');
            if !r.description.trim().is_empty() {
                s.push_str(&format!(" {}", escape_html(r.description.trim())));
            }
            s.push_str("</p>\n");
        }
    }
    if !geo.landmarks.is_empty() {
        s.push_str("<h3>Landmarks</h3>\n");
        for l in &geo.landmarks {
            s.push_str(&format!("<p><strong>{}</strong>", escape_html(&l.name)));
            let mut paren = Vec::new();
            if !l.kind.trim().is_empty() {
                paren.push(escape_html(l.kind.trim()));
            }
            if l.population > 0 {
                paren.push(format!("population {}", thousands(l.population)));
            }
            if !paren.is_empty() {
                s.push_str(&format!(" ({})", paren.join(", ")));
            }
            if !l.description.trim().is_empty() {
                s.push_str(&format!(" — {}", escape_html(l.description.trim())));
            }
            s.push_str(".</p>\n");
        }
    }
}

fn waters(s: &mut String, def: &WorldDefinition) {
    let Some(hy) = &def.hydrology else { return };
    if hy.rivers.is_empty() && hy.lakes.is_empty() && hy.seas.is_empty() && hy.rainfall.trim().is_empty() {
        return;
    }
    s.push_str("<h2>The waters</h2>\n<p>");
    if !hy.rainfall.trim().is_empty() {
        s.push_str(&format!("The world is a {} one. ", escape_html(hy.rainfall.trim())));
    }
    let mut clauses = Vec::new();
    if !hy.rivers.is_empty() {
        clauses.push(format!("its rivers include {}", water_list(&hy.rivers)));
    }
    if !hy.lakes.is_empty() {
        clauses.push(format!("its lakes, {}", water_list(&hy.lakes)));
    }
    if !hy.seas.is_empty() {
        clauses.push(format!("its seas, {}", water_list(&hy.seas)));
    }
    if !clauses.is_empty() {
        let joined = clauses.join("; ");
        let mut chars = joined.chars();
        let cap: String = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default() + chars.as_str();
        s.push_str(&cap);
        s.push('.');
    }
    s.push_str("</p>\n");
    for w in def.hydrology.iter().flat_map(|h| h.rivers.iter().chain(&h.lakes).chain(&h.seas)) {
        if !w.description.trim().is_empty() {
            s.push_str(&format!("<p><strong>{}</strong> — {}.</p>\n", escape_html(&w.name), escape_html(w.description.trim())));
        }
    }
}

fn peoples(s: &mut String, def: &WorldDefinition) {
    if def.nations.is_empty() && def.cultures.is_empty() {
        return;
    }
    s.push_str("<h2>The peoples</h2>\n");
    if !def.nations.is_empty() {
        s.push_str(&format!("<p>{} hold the land.</p>\n", count(def.nations.len(), "nation", "nations")));
        for n in &def.nations {
            s.push_str(&format!("<p><strong>{}</strong>", escape_html(&n.name)));
            if !n.relations.is_empty() {
                let rels: Vec<String> = n
                    .relations
                    .iter()
                    .map(|r| format!("{} {}", escape_html(r.stance.trim()), escape_html(&r.with)))
                    .collect();
                s.push_str(&format!(" — {}", join(&rels)));
            }
            s.push_str(".</p>\n");
        }
    }
    for c in &def.cultures {
        s.push_str(&format!("<p>The people of <strong>{}</strong>", escape_html(&c.nation)));
        let mut clauses = Vec::new();
        if !c.ethos.trim().is_empty() {
            clauses.push(format!("are {}", escape_html(c.ethos.trim())));
        }
        if !c.belief.trim().is_empty() {
            clauses.push(format!("hold that {}", escape_html(c.belief.trim())));
        }
        if !c.language.trim().is_empty() {
            clauses.push(format!("speak a {} tongue", escape_html(c.language.trim())));
        }
        if clauses.is_empty() {
            s.push_str(" endure");
        } else {
            s.push(' ');
            s.push_str(&join(&clauses));
        }
        s.push_str(".</p>\n");
    }
}

fn livelihood(s: &mut String, def: &WorldDefinition) {
    let Some(e) = &def.economy else { return };
    if e.tech_level.trim().is_empty() && e.currency.trim().is_empty() && e.trade_goods.is_empty() && e.resources.is_empty() {
        return;
    }
    s.push_str("<h2>Livelihood</h2>\n<p>");
    if !e.tech_level.trim().is_empty() {
        s.push_str(&format!("Theirs is {} {} age. ", article(&e.tech_level), escape_html(e.tech_level.trim())));
    }
    let mut clauses = Vec::new();
    if !e.currency.trim().is_empty() {
        clauses.push(format!("they reckon wealth in {}", escape_html(e.currency.trim())));
    }
    if !e.trade_goods.is_empty() {
        clauses.push(format!("they trade in {}", join(&esc_all(&e.trade_goods))));
    }
    if !e.resources.is_empty() {
        clauses.push(format!("their land yields {}", join(&esc_all(&e.resources))));
    }
    if !clauses.is_empty() {
        let joined = join(&clauses);
        let mut chars = joined.chars();
        let cap: String = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default() + chars.as_str();
        s.push_str(&cap);
        s.push('.');
    }
    s.push_str("</p>\n");
}

fn life(s: &mut String, def: &WorldDefinition) {
    let Some(eco) = &def.ecology else { return };
    if eco.regions.is_empty() {
        return;
    }
    s.push_str("<h2>Living things</h2>\n");
    for r in &eco.regions {
        s.push_str(&format!("<p>In the <strong>{}</strong>, ", escape_html(&r.biome)));
        let mut clauses = Vec::new();
        if !r.flora.is_empty() {
            clauses.push(format!("{} grow", join(&esc_all(&r.flora))));
        }
        if !r.fauna.is_empty() {
            clauses.push(format!("{} range", join(&esc_all(&r.fauna))));
        }
        if clauses.is_empty() {
            s.push_str("life finds its balance");
        } else {
            s.push_str(&join(&clauses));
        }
        if !r.keystone.trim().is_empty() {
            s.push_str(&format!("; the {} holds the web together", escape_html(r.keystone.trim())));
        }
        s.push_str(".</p>\n");
    }
}

fn magic(s: &mut String, def: &WorldDefinition) {
    let Some(m) = &def.magic else { return };
    if !m.enabled || m.rules.is_empty() {
        return;
    }
    s.push_str("<h2>The laws of magic</h2>\n");
    s.push_str(&format!("<p>The world answers to {}.</p>\n", count(m.rules.len(), "law", "laws")));
    for r in &m.rules {
        s.push_str("<p>");
        if !r.kind.trim().is_empty() {
            s.push_str(&format!("<strong>{}</strong> — ", escape_html(r.kind.trim())));
        }
        s.push_str(&escape_html(r.description.trim()));
        s.push_str(".</p>\n");
    }
}

fn history(s: &mut String, def: &WorldDefinition) {
    let Some(h) = &def.history else { return };
    if h.events.is_empty() {
        return;
    }
    let mut events = h.events.clone();
    events.sort_by_key(|e| e.year);
    s.push_str("<h2>History</h2>\n");
    s.push_str(&format!("<p>Its chronicle records {}.</p>\n", count(events.len(), "event", "events")));
    for e in &events {
        s.push_str(&format!(
            "<p><strong>{}</strong> <em>({})</em>",
            escape_html(&e.title),
            year_label(e.year)
        ));
        if !e.description.trim().is_empty() {
            s.push_str(&format!(" — {}", escape_html(e.description.trim())));
        }
        s.push_str(".</p>\n");
    }
}

// ── helpers ─────────────────────────────────────────────────────────

fn pick_name(name: &str) -> String {
    if name.is_empty() { "The world".into() } else { format!("<strong>{name}</strong>") }
}

fn esc_all(items: &[String]) -> Vec<String> {
    items.iter().map(|s| escape_html(s)).collect()
}

fn water_list(w: &[crate::world::types::NamedWater]) -> String {
    join(&w.iter().map(|x| format!("<em>{}</em>", escape_html(&x.name))).collect::<Vec<_>>())
}

fn join(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        _ => {
            let (last, rest) = items.split_last().unwrap();
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}

fn count(n: usize, singular: &str, plural: &str) -> String {
    if n == 1 {
        format!("one {singular}")
    } else {
        format!("{n} {plural}")
    }
}

fn article(word: &str) -> &'static str {
    match word.trim().chars().next().map(|c| c.to_ascii_lowercase()) {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

fn num(f: f64) -> String {
    if (f - f.round()).abs() < 1e-9 {
        format!("{}", f.round() as i64)
    } else {
        format!("{f:.1}")
    }
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn year_label(y: i64) -> String {
    match y {
        0 => "in the present day".into(),
        y if y < 0 => format!("{} years ago", thousands((-y) as u64)),
        y => format!("{} years hence", thousands(y as u64)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_narrative_from_world_hjson() {
        let body = r#"{
          name: "Aethel"
          astronomy: {
            star: { class: "G2V", luminosity_solar: 1.0, age_gyr: 4.6 }
            planet: { mass_earth: 1, radius_earth: 1, axial_tilt_deg: 23, day_length_hours: 24 }
            orbit: { semi_major_axis_au: 1.0, year_length_days: 360 }
            moons: [ { name: "Luin", period_days: 27 } ]
            calendar: { months: 12, month_length_days: 30, weekdays: 7, month_names: [], day_names: [] }
          }
          history: { events: [ { year: -300, title: "The Drowning", description: "the sea took the low coast" } ] }
        }"#;
        let def = WorldDefinition::from_hjson(body).expect("parse");
        let html = render(&def);
        assert!(html.contains("<h1>Aethel</h1>"));
        assert!(html.contains("a G2V star"));
        assert!(html.contains("<em>Luin</em>"));
        assert!(html.contains("A day lasts 24 hours"));
        assert!(html.contains("<h2>History</h2>"));
        assert!(html.contains("The Drowning"));
        assert!(html.contains("300 years ago"));
    }
}
