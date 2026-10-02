//! WORLD-KEEP-2 (WK2-P2) — the one mapping between map cells and degrees.
//!
//! Every layer treats the grid as the whole planet: the top row is the north
//! pole, the bottom the south, the columns run once around. That is right for a
//! generated world and for a whole-world heightmap. It is wrong for a heightmap
//! that declares a regional scale — a 1200 km map weathered from pole to pole
//! grows ice at its top edge and jungle in its middle.
//!
//! [`LatMap`] is that mapping, in one place. By default it is the globe, with
//! the exact arithmetic the layers have always used (so nothing moves for an
//! existing world). When a heightmap declares BOTH a regional scale and a
//! `dem.center_lat`, it becomes the band of latitude the map's height really
//! covers around that centre (and the matching span of longitude around
//! `dem.center_lon`). Strictly opt-in: no `center_lat`, no change.
//!
//! Climate, weather, the scene brief, `set-coords` and landmark positions all
//! read latitude through this type, so they cannot disagree.

/// The geographic extent a map grid covers, in degrees. Cell coordinates are
/// `(x, y)` with `y = 0` the northern row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatMap {
    /// Latitude of the map's top and bottom EDGES.
    pub north: f64,
    pub south: f64,
    /// Longitude of the map's left and right EDGES.
    pub west: f64,
    pub east: f64,
}

impl Default for LatMap {
    fn default() -> Self {
        LatMap::globe()
    }
}

const EARTH_RADIUS_KM: f64 = 6371.0;

impl LatMap {
    /// The whole planet: pole to pole, once around.
    pub const fn globe() -> LatMap {
        LatMap { north: 90.0, south: -90.0, west: -180.0, east: 180.0 }
    }

    /// A regional map `wide_km × tall_km` on a planet of `radius_earth`,
    /// centred on `(center_lat, center_lon)`. The latitude band is as tall as
    /// the map really is; it is slid, not squeezed, if it would cross a pole.
    /// Degenerate input (non-finite, non-positive size) yields the globe.
    pub fn regional(center_lat: f64, center_lon: f64, wide_km: f64, tall_km: f64, radius_earth: f64) -> LatMap {
        let ok = |v: f64| v.is_finite();
        if !(ok(center_lat) && ok(center_lon) && ok(wide_km) && ok(tall_km)) || wide_km <= 0.0 || tall_km <= 0.0 {
            return LatMap::globe();
        }
        let km_per_deg = std::f64::consts::PI * EARTH_RADIUS_KM * radius_earth.max(0.01) / 180.0;
        let lat_span = (tall_km / km_per_deg).min(180.0);
        let c = center_lat.clamp(-90.0, 90.0);
        let (mut north, mut south) = (c + lat_span / 2.0, c - lat_span / 2.0);
        if north > 90.0 {
            south -= north - 90.0;
            north = 90.0;
        }
        if south < -90.0 {
            north = (north + (-90.0 - south)).min(90.0);
            south = -90.0;
        }
        // A degree of longitude shrinks with the cosine of latitude.
        let lon_km_per_deg = km_per_deg * c.to_radians().cos().max(0.05);
        let lon_span = (wide_km / lon_km_per_deg).min(360.0);
        let lon_c = center_lon.clamp(-180.0, 180.0);
        LatMap { north, south, west: lon_c - lon_span / 2.0, east: lon_c + lon_span / 2.0 }
    }

    /// Whether this is anything other than the whole planet.
    pub fn is_regional(&self) -> bool {
        *self != LatMap::globe()
    }

    /// Latitude at the CENTRE of row `y` on a grid `height` rows tall.
    pub fn row_lat(&self, y: usize, height: usize) -> f64 {
        if height == 0 {
            return (self.north + self.south) / 2.0;
        }
        self.north - (y as f64 + 0.5) / height as f64 * (self.north - self.south)
    }

    /// The row whose centre is nearest `lat` (clamped to the map).
    pub fn lat_row(&self, lat: f64, height: usize) -> usize {
        if height == 0 {
            return 0;
        }
        let span = self.north - self.south;
        if span <= 0.0 || !lat.is_finite() {
            return 0;
        }
        let y = (self.north - lat.clamp(self.south, self.north)) / span * height as f64 - 0.5;
        y.round().clamp(0.0, (height - 1) as f64) as usize
    }

    /// Longitude at the centre of column `x` on a grid `width` columns wide.
    pub fn col_lon(&self, x: usize, width: usize) -> f64 {
        if width == 0 {
            return (self.west + self.east) / 2.0;
        }
        self.west + (x as f64 + 0.5) / width as f64 * (self.east - self.west)
    }

    /// The column whose centre is nearest `lon` (clamped to the map).
    pub fn lon_col(&self, lon: f64, width: usize) -> usize {
        if width == 0 {
            return 0;
        }
        let span = self.east - self.west;
        if span <= 0.0 || !lon.is_finite() {
            return 0;
        }
        let x = (lon.clamp(self.west, self.east) - self.west) / span * width as f64 - 0.5;
        x.round().clamp(0.0, (width - 1) as f64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_globe_is_the_arithmetic_the_layers_always_used() {
        let g = LatMap::globe();
        assert!(!g.is_regional());
        let h = 120usize;
        for y in [0usize, 1, 59, 60, 119] {
            // climate_layer / row_to_latitude: 90 − (y+0.5)/h·180
            assert_eq!(g.row_lat(y, h), 90.0 - (y as f64 + 0.5) / h as f64 * 180.0);
        }
        let w = 160usize;
        for x in [0usize, 80, 159] {
            assert_eq!(g.col_lon(x, w), (x as f64 + 0.5) / w as f64 * 360.0 - 180.0);
        }
        // lat_to_row / lon_to_col, including the clamps.
        assert_eq!(g.lat_row(90.0, h), 0);
        assert_eq!(g.lat_row(-90.0, h), h - 1);
        assert_eq!(g.lat_row(200.0, h), 0);
        assert_eq!(g.lon_col(-180.0, w), 0);
        assert_eq!(g.lon_col(999.0, w), w - 1);
        let row = g.lat_row(45.0, h);
        assert!((g.row_lat(row, h) - 45.0).abs() <= 180.0 / h as f64 + 1e-9);
        // Degenerate grids do not panic.
        assert_eq!(g.row_lat(0, 0), 0.0);
        assert_eq!(g.lat_row(10.0, 0), 0);
    }

    #[test]
    fn a_regional_map_covers_the_band_its_height_really_spans() {
        // 1200 km tall on an Earth-sized planet ≈ 10.8° of latitude.
        let m = LatMap::regional(45.0, 10.0, 1600.0, 1200.0, 1.0);
        assert!(m.is_regional());
        let span = m.north - m.south;
        assert!((span - 10.79).abs() < 0.05, "span {span}");
        assert!((m.north - 50.4).abs() < 0.05 && (m.south - 39.6).abs() < 0.05);
        // Rows run north → south inside the band, and invert cleanly.
        assert!(m.row_lat(0, 120) > m.row_lat(119, 120));
        assert!(m.row_lat(0, 120) < m.north && m.row_lat(119, 120) > m.south);
        for y in [0usize, 30, 60, 119] {
            assert_eq!(m.lat_row(m.row_lat(y, 120), 120), y);
        }
        // Longitude widens with latitude: 1600 km at 45° is more degrees than at the equator.
        let eq = LatMap::regional(0.0, 10.0, 1600.0, 1200.0, 1.0);
        assert!((m.east - m.west) > (eq.east - eq.west));
        assert!((m.col_lon(80, 160) - 10.0).abs() < 0.2, "centre column sits near center_lon");
        // A latitude off the map clamps to its edge row.
        assert_eq!(m.lat_row(80.0, 120), 0);
        assert_eq!(m.lat_row(-10.0, 120), 119);
    }

    #[test]
    fn a_band_that_would_cross_a_pole_is_slid_not_squeezed() {
        let m = LatMap::regional(88.0, 0.0, 1000.0, 2000.0, 1.0); // ~18° tall, centred near the pole
        assert_eq!(m.north, 90.0);
        assert!((m.north - m.south - 17.99).abs() < 0.05);
        // A bigger planet covers fewer degrees with the same kilometres.
        let big = LatMap::regional(0.0, 0.0, 1000.0, 2000.0, 2.0);
        assert!((big.north - big.south) < 10.0);
        // Garbage in → the globe, never a panic or a NaN band.
        assert_eq!(LatMap::regional(f64::NAN, 0.0, 1.0, 1.0, 1.0), LatMap::globe());
        assert_eq!(LatMap::regional(0.0, 0.0, 0.0, 100.0, 1.0), LatMap::globe());
    }
}
