//! WBLD-1 (WB-P6) — the in-pane ASCII biome minimap.
//!
//! The Map right-pane renders the *compiled* world (from `/compile`) directly in
//! the terminal: the climate biome grid downsampled to the pane, with rivers and
//! settlements stamped over it. Deterministic and LLM-free — it reads the cached
//! [`CompiledLayers`] and needs no external binary, so it works on any terminal.
//!
//! This ASCII map is the always-available baseline. A later refinement (folded
//! into the WB-P8 map-first workflow) can show the full `plakat` raster in the
//! same pane on image-capable terminals, using the shared `ratatui-image`
//! `Picker` (`Picker::from_query_stdio` → `new_resize_protocol` → `StatefulImage`,
//! as the editor image-preview and story view already do), falling back to this
//! grid when the terminal can't display images or `plakat` is absent.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::world::types::Biome;

use super::app::WorldbuilderApp;

/// Glyph + colour for a biome cell.
fn biome_cell(b: Biome) -> (char, Color) {
    match b {
        Biome::Ocean => ('~', Color::Blue),
        Biome::IceCap => ('*', Color::White),
        Biome::Tundra => ('-', Color::Gray),
        Biome::Taiga => ('t', Color::Green),
        Biome::TemperateForest => ('T', Color::Green),
        Biome::TemperateGrassland => ('"', Color::LightGreen),
        Biome::Mediterranean => ('m', Color::LightYellow),
        Biome::ColdDesert => (',', Color::Gray),
        Biome::HotDesert => (':', Color::Yellow),
        Biome::Savanna => (';', Color::LightYellow),
        Biome::TropicalSeasonal => ('w', Color::LightGreen),
        Biome::TropicalRainforest => ('#', Color::Green),
    }
}

/// Downsample the compiled world to a `map_w × map_h` grid of `(glyph, colour)`
/// cells: the climate biome grid with rivers and settlements stamped on top
/// (settlement > river > biome). Pure — the render path and tests share it.
fn compose(
    layers: &crate::world::plausibility::CompiledLayers,
    map_w: usize,
    map_h: usize,
) -> Vec<Vec<(char, Color)>> {
    let climate = &layers.climate;
    let (sw, sh) = (climate.width, climate.height);
    let hydro = &layers.hydrology;
    let has_rivers = hydro.is_river.len() == sw * sh;

    // Settlements → the output cell they land in. 0 = none, 1 = town/village,
    // 2 = city; the higher rank wins a shared cell.
    let mut town_at = vec![0u8; map_w * map_h];
    for s in &layers.demographics.settlements {
        if s.x >= sw || s.y >= sh {
            continue;
        }
        let ox = (s.x * map_w / sw).min(map_w - 1);
        let oy = (s.y * map_h / sh).min(map_h - 1);
        let rank = if s.class == "city" { 2 } else { 1 };
        let slot = &mut town_at[oy * map_w + ox];
        if rank > *slot {
            *slot = rank;
        }
    }

    let mut grid = Vec::with_capacity(map_h);
    for oy in 0..map_h {
        let sy = oy * sh / map_h;
        let mut row = Vec::with_capacity(map_w);
        for ox in 0..map_w {
            let sx = ox * sw / map_w;
            let idx = sy * sw + sx;
            let cell = match town_at[oy * map_w + ox] {
                2 => ('◉', Color::LightRed),
                1 => ('•', Color::Red),
                _ if has_rivers && hydro.is_river[idx] && climate.biome[idx] != Biome::Ocean => {
                    ('≈', Color::Cyan)
                }
                _ => biome_cell(climate.biome[idx]),
            };
            row.push(cell);
        }
        grid.push(row);
    }
    grid
}

/// Map a source-grid cell `(sx, sy)` to the display cell it falls in, given the
/// source dimensions and the display dimensions. Pure; shared by the cursor
/// render and tests. Clamps into range.
pub(super) fn source_to_display(
    (sx, sy): (usize, usize),
    (sw, sh): (usize, usize),
    (dw, dh): (usize, usize),
) -> (usize, usize) {
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return (0, 0);
    }
    let dx = (sx * dw / sw).min(dw - 1);
    let dy = (sy * dh / sh).min(dh - 1);
    (dx, dy)
}

/// Render the Map pane. Falls back to a hint when there is no compiled world yet.
pub(super) fn render_map(frame: &mut Frame, app: &WorldbuilderApp, area: Rect) {
    let Some(layers) = app.compiled_layers.as_ref() else {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "Run /compile for the ASCII map, or /map for the plakat raster.",
                Style::new().dim(),
            )),
            area,
        );
        return;
    };
    let climate = &layers.climate;
    let (sw, sh) = (climate.width, climate.height);
    if sw == 0 || sh == 0 || climate.biome.len() != sw * sh {
        frame.render_widget(
            Paragraph::new(Span::styled("(empty climate grid)", Style::new().dim())),
            area,
        );
        return;
    }

    // Leave the last two rows for a scale line + legend.
    let map_h = area.height.saturating_sub(2) as usize;
    let map_w = area.width as usize;
    if map_h == 0 || map_w == 0 {
        return;
    }

    let hydro = &layers.hydrology;
    let grid = compose(layers, map_w, map_h);

    // MAPED-P1 — in edit mode, the source-space cursor maps to one display cell.
    let cursor_disp = if app.map_edit {
        Some(source_to_display(app.map_cursor, (sw, sh), (map_w, map_h)))
    } else {
        None
    };

    let mut lines: Vec<Line> = Vec::with_capacity(map_h + 2);
    for (y, row) in grid.iter().enumerate() {
        let spans: Vec<Span> = row
            .iter()
            .enumerate()
            .map(|(x, &(ch, color))| {
                let mut st = Style::new().fg(color);
                if cursor_disp == Some((x, y)) {
                    st = st.add_modifier(Modifier::REVERSED);
                }
                Span::styled(ch.to_string(), st)
            })
            .collect();
        lines.push(Line::from(spans));
    }

    // Scale / readout line.
    if app.map_edit {
        let (cx, cy) = app.map_cursor;
        let idx = cy.min(sh - 1) * sw + cx.min(sw - 1);
        let biome = climate.biome.get(idx).map(|b| b.as_str()).unwrap_or("?");
        let elev = layers.geology.heightmap.get(idx).copied().unwrap_or(0.0);
        let sea = if elev <= layers.geology.sea_level { " · sea" } else { "" };
        lines.push(Line::from(Span::styled(
            format!("✎ ({cx},{cy}) · {biome} · elev {elev:.2}{sea}  ·  hjkl move · Shift fine · Esc leave"),
            Style::new().fg(Color::Yellow),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!(
                "grid {sw}×{sh} → {map_w}×{map_h} · {} river cell(s) · {} settlement(s) · e: edit",
                hydro.river_count,
                layers.demographics.settlements.len(),
            ),
            Style::new().dim(),
        )));
    }
    lines.push(Line::from(vec![
        Span::styled("~", Style::new().fg(Color::Blue)),
        Span::raw(" sea  "),
        Span::styled("≈", Style::new().fg(Color::Cyan)),
        Span::raw(" river  "),
        Span::styled("#T", Style::new().fg(Color::Green)),
        Span::raw(" forest  "),
        Span::styled(":", Style::new().fg(Color::Yellow)),
        Span::raw(" desert  "),
        Span::styled("•", Style::new().fg(Color::Red)),
        Span::styled("◉", Style::new().fg(Color::LightRed)),
        Span::raw(" town/city"),
    ]));

    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::plausibility::compile_layers;
    use crate::world::types::WorldDefinition;

    fn terra() -> WorldDefinition {
        let body = r#"{
            name: "Terra"
            seed: 0x5151
            astronomy: {
                star: { luminosity_solar: 1.0 }
                planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4, day_length_hours: 24.0 }
                orbit: { semi_major_axis_au: 1.0 }
                calendar: { months: 12, month_length_days: 30 }
            }
        }"#;
        WorldDefinition::from_hjson(body).unwrap()
    }

    #[test]
    fn biome_cell_is_total_over_every_variant() {
        // Exhaustive match — if a biome is added, this fails to compile, forcing a
        // glyph decision. All variants must produce a printable, non-space glyph.
        for b in [
            Biome::Ocean,
            Biome::IceCap,
            Biome::Tundra,
            Biome::Taiga,
            Biome::TemperateForest,
            Biome::TemperateGrassland,
            Biome::Mediterranean,
            Biome::ColdDesert,
            Biome::HotDesert,
            Biome::Savanna,
            Biome::TropicalSeasonal,
            Biome::TropicalRainforest,
        ] {
            let (ch, _) = biome_cell(b);
            assert!(!ch.is_whitespace(), "{b:?} maps to whitespace");
        }
    }

    #[test]
    fn compose_fills_the_requested_dimensions() {
        let layers = compile_layers(&terra());
        let grid = compose(&layers, 40, 20);
        assert_eq!(grid.len(), 20);
        assert!(grid.iter().all(|r| r.len() == 40));
        // A Terra-like world has ocean, so at least one sea glyph must appear.
        let sea = grid.iter().flatten().filter(|&&(c, _)| c == '~').count();
        assert!(sea > 0, "expected some ocean cells in the downsampled map");
    }

    #[test]
    fn source_to_display_maps_and_clamps() {
        // A 96×64 source onto a 48×16 display: top-left → (0,0), bottom-right
        // stays in range, and a mid cell scales proportionally.
        assert_eq!(source_to_display((0, 0), (96, 64), (48, 16)), (0, 0));
        assert_eq!(source_to_display((95, 63), (96, 64), (48, 16)), (47, 15));
        assert_eq!(source_to_display((48, 32), (96, 64), (48, 16)), (24, 8));
        // Degenerate dims never panic.
        assert_eq!(source_to_display((5, 5), (0, 10), (10, 10)), (0, 0));
    }

    #[test]
    fn compose_degrades_when_grid_is_smaller_than_source() {
        // 1×1 output must not panic or index out of bounds.
        let layers = compile_layers(&terra());
        let grid = compose(&layers, 1, 1);
        assert_eq!(grid.len(), 1);
        assert_eq!(grid[0].len(), 1);
    }
}
