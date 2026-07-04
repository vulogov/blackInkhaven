//! WORLD-10 (S-P1) — the scene world-context *composition*. A pure function that
//! assembles what an author needs while writing a scene: the place, the local
//! season + weather on the scene's day, and the culture of the nearest realm.
//! Both `realworld scene` (CLI) and the editor footer chip / `Ctrl+B W` overview
//! call this — one source of truth.

use crate::world::compile::culture_layer::CultureOutput;
use crate::world::compile::polities_layer::PolitiesOutput;
use crate::world::proposals::PlaceLink;
use crate::world::types::AstronomyOutput;
use crate::world::weather::weather_at;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SceneBrief {
    pub place: Option<String>,
    pub biome: Option<String>,
    pub climate_zone: Option<String>,
    pub day_of_year: Option<f64>,
    pub season: Option<String>,
    pub conditions: Option<String>,
    pub realm: Option<String>,
    pub ethos: Option<String>,
    pub belief: Option<String>,
    pub tongue: Option<String>,
}

impl SceneBrief {
    pub fn is_empty(&self) -> bool {
        self.place.is_none() && self.season.is_none() && self.realm.is_none()
    }

    /// The one-line footer chip, e.g.
    /// `Karthage · late autumn, cool & wet · Karon (mercantile)`. `None` when
    /// nothing resolved.
    pub fn chip(&self) -> Option<String> {
        let mut parts: Vec<String> = Vec::new();
        if let Some(p) = &self.place {
            parts.push(p.clone());
        }
        match (&self.season, &self.conditions) {
            (Some(s), Some(c)) => parts.push(format!("{s}, {c}")),
            (Some(s), None) => parts.push(s.clone()),
            _ => {}
        }
        if let Some(r) = &self.realm {
            let ethos_head = self
                .ethos
                .as_deref()
                .and_then(|e| e.split([',', ' ']).next())
                .filter(|s| !s.is_empty());
            match ethos_head {
                Some(h) => parts.push(format!("{r} ({h})")),
                None => parts.push(r.clone()),
            }
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" · "))
        }
    }
}

fn non_empty(s: &str) -> Option<String> {
    (!s.trim().is_empty()).then(|| s.to_string())
}

pub fn scene_brief(
    astro: &AstronomyOutput,
    polities: &PolitiesOutput,
    cultures: &CultureOutput,
    place: Option<&PlaceLink>,
    day: Option<f64>,
    latitude: Option<f64>,
) -> SceneBrief {
    let mut b = SceneBrief::default();

    if let Some(p) = place {
        b.place = non_empty(&p.name);
        b.biome = non_empty(&p.biome);
        b.climate_zone = non_empty(&p.climate_zone);
    }

    if let (Some(day), Some(lat)) = (day, latitude) {
        let w = weather_at(astro, day, lat);
        b.day_of_year = Some(day);
        b.season = Some(w.season);
        b.conditions = Some(w.descriptor);
    }

    // The culture of the realm whose capital sits nearest the place.
    if let Some(p) = place {
        if let Some((i, pol)) = polities.polities.iter().enumerate().min_by_key(|(_, q)| {
            let dx = q.capital_pos.0 as i64 - p.x as i64;
            let dy = q.capital_pos.1 as i64 - p.y as i64;
            dx * dx + dy * dy
        }) {
            b.realm = Some(pol.name.clone());
            if let Some(c) = cultures.cultures.get(i) {
                b.ethos = Some(c.ethos.clone());
                b.belief = Some(c.belief.clone());
                b.tongue = Some(c.language_profile.clone());
            }
        }
    }

    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::compile_astronomy;
    use crate::world::compile::polities_layer::Polity;
    use crate::world::types::WorldDefinition;

    fn astro() -> AstronomyOutput {
        let def = WorldDefinition::from_hjson(
            r#"{ name: "E", astronomy: {
                star: { class: "G2V", luminosity_solar: 1.0 }
                planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4, day_length_hours: 24.0 }
                orbit: { semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365 }
                moons: []
                calendar: { months: 12, month_length_days: 30 }
            } }"#,
        )
        .unwrap();
        compile_astronomy(&def.astronomy)
    }

    fn place() -> PlaceLink {
        PlaceLink {
            place_id: uuid::Uuid::nil(),
            name: "Karthage".into(),
            biome: "mediterranean".into(),
            climate_zone: "warm-temperate".into(),
            hydrology_basis: "river_mouth".into(),
            population: 40_000,
            x: 5,
            y: 4,
        }
    }

    fn peoples() -> (PolitiesOutput, CultureOutput) {
        let pol = PolitiesOutput {
            polities: vec![Polity {
                name: "Karon".into(),
                capital: "the mediterranean city".into(),
                capital_pos: (6, 4),
                member_count: 3,
                population: 50_000,
            }],
            relations: Vec::new(),
        };
        let cul = crate::world::compile::compile_culture(
            &pol,
            &["mediterranean".to_string()],
            7,
        );
        (pol, cul)
    }

    #[test]
    fn brief_assembles_place_weather_and_people() {
        let a = astro();
        let (pol, cul) = peoples();
        let p = place();
        let b = scene_brief(&a, &pol, &cul, Some(&p), Some(180.0), Some(45.0));
        assert_eq!(b.place.as_deref(), Some("Karthage"));
        assert_eq!(b.biome.as_deref(), Some("mediterranean"));
        assert!(b.season.is_some());
        assert_eq!(b.realm.as_deref(), Some("Karon"));
        assert!(b.ethos.is_some() && b.tongue.is_some());
        // The chip folds it into one line.
        assert!(b.chip().unwrap().starts_with("Karthage · "));
    }

    #[test]
    fn empty_without_place_or_time() {
        let a = astro();
        let (pol, cul) = peoples();
        let b = scene_brief(&a, &pol, &cul, None, None, None);
        assert!(b.is_empty());
        assert!(b.chip().is_none());
    }

    #[test]
    fn date_only_still_briefs_the_season() {
        let a = astro();
        let (pol, cul) = peoples();
        let b = scene_brief(&a, &pol, &cul, None, Some(180.0), Some(45.0));
        assert!(b.place.is_none());
        assert!(b.season.is_some());
        assert!(b.chip().is_some());
    }
}
