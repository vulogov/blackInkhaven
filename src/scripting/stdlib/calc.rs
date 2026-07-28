//! RESRCH-3 (R3-C, brought forward) — `calc.*` Bund words: physical constants
//! and unit-conversion words that turn the research assistant's `/calc` into a
//! useful, deterministic calculator. Pure values + arithmetic — no store, no
//! network, no AI; the computation is its own proof.
//!
//! Constants push an SI-base value (metres, seconds, m/s …); conversions pop a
//! number and push the converted value. Each is registered as `calc.<name>` with
//! a best-effort short alias `<name>` (collisions with builtins simply keep the
//! `calc.`-prefixed form).

use std::cell::RefCell;
use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use serde_json::Value as Json;

use super::helpers::{pull, push};

/// A constant word: push a fixed `f64`.
macro_rules! kpush {
    ($fn:ident, $v:expr) => {
        fn $fn(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            push(vm, Value::from_float($v));
            Ok(vm)
        }
    };
}

/// Coerce a popped value to `f64`, accepting both float and integer literals
/// (a bare `100` parses as an integer, not a float).
fn as_f64(v: &Value) -> std::result::Result<f64, BundError> {
    v.cast_float()
        .or_else(|_| v.cast_int().map(|i| i as f64))
        .map_err(|e| easy_error::err_msg(format!("calc: expected a number ({e})")))
}

/// Pop one number off the stack, coercing int→float (R4-A/B helper).
fn pop_f(vm: &mut VM, tag: &str) -> std::result::Result<f64, BundError> {
    let v = pull(vm, tag).map_err(|e| easy_error::err_msg(e.to_string()))?;
    as_f64(&v)
}

/// A conversion word: pop a number, push `f(x)`.
macro_rules! kconv {
    ($fn:ident, $f:expr) => {
        fn $fn(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            let v = pull(vm, "calc").map_err(|e| easy_error::err_msg(e.to_string()))?;
            let x = as_f64(&v)?;
            let f: fn(f64) -> f64 = $f;
            push(vm, Value::from_float(f(x)));
            Ok(vm)
        }
    };
}

/// A binary word: pop `b` then `a` (so `( a b -- y )`), push `f(a, b)`.
macro_rules! kbin {
    ($fn:ident, $tag:literal, $f:expr) => {
        fn $fn(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            let b = pop_f(vm, $tag)?;
            let a = pop_f(vm, $tag)?;
            let f: fn(f64, f64) -> f64 = $f;
            push(vm, Value::from_float(f(a, b)));
            Ok(vm)
        }
    };
}

// ── constants (SI base: metres, seconds, m/s) ───────────────────────────────
kpush!(k_pi, std::f64::consts::PI);
kpush!(k_tau, std::f64::consts::TAU);
kpush!(k_e, std::f64::consts::E);
kpush!(k_c, 299_792_458.0); // speed of light, m/s
kpush!(k_grav, 6.674_30e-11); // gravitational constant G
kpush!(k_gee, 9.806_65); // standard gravity g0, m/s²
kpush!(k_au, 1.495_978_707e11); // astronomical unit, m
kpush!(k_ly, 9.460_730_472_580_8e15); // light-year, m
kpush!(k_pc, 3.085_677_581_491_367e16); // parsec, m
kpush!(k_year, 31_557_600.0); // Julian year, s (365.25 d)
kpush!(k_day, 86_400.0);
kpush!(k_hour, 3_600.0);
kpush!(k_minute, 60.0);

// ── conversions ─────────────────────────────────────────────────────────────
kconv!(c_km2mi, |x| x * 0.621_371_192);
kconv!(c_mi2km, |x| x * 1.609_344);
kconv!(c_m2ft, |x| x * 3.280_839_895);
kconv!(c_ft2m, |x| x * 0.304_8);
kconv!(c_m2mi, |x| x * 0.000_621_371_192);
kconv!(c_mi2m, |x| x * 1609.344);
kconv!(c_c2f, |x| x * 9.0 / 5.0 + 32.0);
kconv!(c_f2c, |x| (x - 32.0) * 5.0 / 9.0);
kconv!(c_c2k, |x| x + 273.15);
kconv!(c_k2c, |x| x - 273.15);
kconv!(c_kg2lb, |x| x * 2.204_622_621_8);
kconv!(c_lb2kg, |x| x * 0.453_592_37);
kconv!(c_kmh2mph, |x| x * 0.621_371_192);
kconv!(c_mph2kmh, |x| x * 1.609_344);
kconv!(c_au2km, |x| x * 1.495_978_707e8);
kconv!(c_km2au, |x| x / 1.495_978_707e8);
kconv!(c_ly2km, |x| x * 9.460_730_472_580_8e12);
kconv!(c_km2ly, |x| x / 9.460_730_472_580_8e12);
kconv!(c_ly2au, |x| x * 63_241.077);
kconv!(c_au2ly, |x| x / 63_241.077);
kconv!(c_pc2ly, |x| x * 3.261_563_777);
kconv!(c_ly2pc, |x| x / 3.261_563_777);
kconv!(c_deg2rad, |x| x * std::f64::consts::PI / 180.0);
kconv!(c_rad2deg, |x| x * 180.0 / std::f64::consts::PI);

// ── scientific math substrate (R4-A) ─────────────────────────────────────────
// Registered under `calc.<name>` (short alias best-effort, like the conversions),
// so the `calc.`-prefixed form never collides with bundcore's own arithmetic.
kconv!(m_sqrt, |x| x.sqrt());
kconv!(m_cbrt, |x| x.cbrt());
kconv!(m_exp, |x| x.exp());
kconv!(m_ln, |x| x.ln());
kconv!(m_log10, |x| x.log10());
kconv!(m_log2, |x| x.log2());
kconv!(m_sin, |x| x.sin());
kconv!(m_cos, |x| x.cos());
kconv!(m_tan, |x| x.tan());
kconv!(m_asin, |x| x.asin());
kconv!(m_acos, |x| x.acos());
kconv!(m_atan, |x| x.atan());
kconv!(m_abs, |x| x.abs());
kconv!(m_floor, |x| x.floor());
kconv!(m_ceil, |x| x.ceil());
kconv!(m_round, |x| x.round());
kbin!(m_pow, "calc.pow", |a, b| a.powf(b));
kbin!(m_atan2, "calc.atan2", |a, b| a.atan2(b)); // ( y x -- rad )
kbin!(m_hypot, "calc.hypot", |a, b| a.hypot(b));

// ── astronomy & planetology (R4-B) ───────────────────────────────────────────
// Deterministic formulas in convenient units; they pair with the `calc.world.*`
// readers (e.g. `world.star_mass world.au kepler_period`). Constants reuse the
// `calc.*` set (G = `calc.grav`, g₀ = `calc.gee`).

const SOLAR_CONSTANT_WM2: f64 = 1361.0; // mean insolation at 1 AU, W/m²
const EARTH_ESCAPE_MS: f64 = 11_186.0; // Earth escape velocity, m/s
const STD_GRAVITY: f64 = 9.806_65; // g₀, m/s²

/// `kepler_period` — `( a M -- T )`: Kepler's third law in solar units.
/// a [AU], M [solar masses] → T [years]; `T = √(a³ / M)`.
fn a_kepler_period(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let m = pop_f(vm, "kepler_period")?;
    let a = pop_f(vm, "kepler_period")?;
    if m <= 0.0 {
        return Err(easy_error::err_msg("kepler_period: mass must be > 0"));
    }
    push(vm, Value::from_float((a.powi(3) / m).sqrt()));
    Ok(vm)
}

/// `surface_gravity` — `( M R -- g )`: M [Earth masses], R [Earth radii] →
/// g [m/s²]; `g = (M / R²) · g₀`.
fn a_surface_gravity(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let r = pop_f(vm, "surface_gravity")?;
    let m = pop_f(vm, "surface_gravity")?;
    if r == 0.0 {
        return Err(easy_error::err_msg("surface_gravity: radius must be ≠ 0"));
    }
    push(vm, Value::from_float((m / (r * r)) * STD_GRAVITY));
    Ok(vm)
}

/// `escape_velocity` — `( M R -- v )`: M [Earth masses], R [Earth radii] →
/// v [m/s]; `v = v⊕·√(M / R)`.
fn a_escape_velocity(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let r = pop_f(vm, "escape_velocity")?;
    let m = pop_f(vm, "escape_velocity")?;
    if r <= 0.0 {
        return Err(easy_error::err_msg("escape_velocity: radius must be > 0"));
    }
    push(vm, Value::from_float(EARTH_ESCAPE_MS * (m / r).sqrt()));
    Ok(vm)
}

/// `insolation` — `( L d -- S )`: L [solar luminosities], d [AU] → S [W/m²];
/// inverse-square, `S = (L / d²) · 1361`.
fn a_insolation(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let d = pop_f(vm, "insolation")?;
    let l = pop_f(vm, "insolation")?;
    if d == 0.0 {
        return Err(easy_error::err_msg("insolation: distance must be ≠ 0"));
    }
    push(vm, Value::from_float((l / (d * d)) * SOLAR_CONSTANT_WM2));
    Ok(vm)
}

/// `synodic_period` — `( T1 T2 -- Tsyn )`: `1/Tsyn = |1/T1 − 1/T2|` (same time unit).
fn a_synodic_period(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let t2 = pop_f(vm, "synodic_period")?;
    let t1 = pop_f(vm, "synodic_period")?;
    if t1 == 0.0 || t2 == 0.0 {
        return Err(easy_error::err_msg("synodic_period: periods must be ≠ 0"));
    }
    let diff = (1.0 / t1 - 1.0 / t2).abs();
    if diff == 0.0 {
        return Err(easy_error::err_msg("synodic_period: equal periods (no synodic cycle)"));
    }
    push(vm, Value::from_float(1.0 / diff));
    Ok(vm)
}

/// `angular_size` — `( size dist -- θ )`: `θ = 2·atan(size / (2·dist))` [rad].
fn a_angular_size(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let dist = pop_f(vm, "angular_size")?;
    let size = pop_f(vm, "angular_size")?;
    if dist == 0.0 {
        return Err(easy_error::err_msg("angular_size: distance must be ≠ 0"));
    }
    push(vm, Value::from_float(2.0 * (size / (2.0 * dist)).atan()));
    Ok(vm)
}

/// `hill_sphere` — `( a e m M -- rH )`: `rH ≈ a·(1−e)·∛(m / (3M))`. a [any length],
/// m & M [same mass unit] → rH [same length as a].
fn a_hill_sphere(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let big_m = pop_f(vm, "hill_sphere")?;
    let m = pop_f(vm, "hill_sphere")?;
    let e = pop_f(vm, "hill_sphere")?;
    let a = pop_f(vm, "hill_sphere")?;
    if big_m <= 0.0 {
        return Err(easy_error::err_msg("hill_sphere: primary mass must be > 0"));
    }
    push(vm, Value::from_float(a * (1.0 - e) * (m / (3.0 * big_m)).cbrt()));
    Ok(vm)
}

/// `roche_limit` — `( R rhoM rhom -- d )`: fluid `d = 2.44·R·∛(ρM / ρm)`.
/// R [any length] → d [same]; densities in any shared unit.
fn a_roche_limit(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let rho_m = pop_f(vm, "roche_limit")?;
    let rho_big = pop_f(vm, "roche_limit")?;
    let r = pop_f(vm, "roche_limit")?;
    if rho_m <= 0.0 {
        return Err(easy_error::err_msg("roche_limit: satellite density must be > 0"));
    }
    push(vm, Value::from_float(2.44 * r * (rho_big / rho_m).cbrt()));
    Ok(vm)
}

/// `tidal_accel` — `( M r d -- a )`: SI tidal acceleration `a = 2·G·M·r / d³`.
/// M [kg], r & d [m] → a [m/s²].
fn a_tidal_accel(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let d = pop_f(vm, "tidal_accel")?;
    let r = pop_f(vm, "tidal_accel")?;
    let m = pop_f(vm, "tidal_accel")?;
    if d == 0.0 {
        return Err(easy_error::err_msg("tidal_accel: distance must be ≠ 0"));
    }
    const G: f64 = 6.674_30e-11;
    push(vm, Value::from_float(2.0 * G * m * r / d.powi(3)));
    Ok(vm)
}

// ── scientific math (R4-A completion) ────────────────────────────────────────
kconv!(m_trunc, |x| x.trunc());
kconv!(m_sign, |x| if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 });
kconv!(m_factorial, |x| {
    let n = x.round().max(0.0) as u64;
    // 171! already overflows f64 to +inf, so anything above that is `inf`
    // regardless — and computing it would spin up to ~1e19 iterations on the
    // synchronous `/calc` (UI) thread, an unkillable freeze. Short-circuit.
    if n > 170 {
        f64::INFINITY
    } else {
        (1..=n).fold(1.0f64, |a, i| a * i as f64)
    }
});

fn gcd_i64(a: i64, b: i64) -> i64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// `gcd` — `( a b -- g )` (integer semantics; inputs rounded).
fn m_gcd(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let b = pop_f(vm, "gcd")?.round() as i64;
    let a = pop_f(vm, "gcd")?.round() as i64;
    push(vm, Value::from_float(gcd_i64(a, b) as f64));
    Ok(vm)
}

/// `lcm` — `( a b -- l )`.
fn m_lcm(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let b = pop_f(vm, "lcm")?.round() as i64;
    let a = pop_f(vm, "lcm")?.round() as i64;
    let g = gcd_i64(a, b);
    let l = if g == 0 { 0.0 } else { (a / g * b).abs() as f64 };
    push(vm, Value::from_float(l));
    Ok(vm)
}

// ── list reducers (R4-A) ─────────────────────────────────────────────────────
// Consume a Bund list literal, e.g. `[ 1 2 3 ] calc.sum`.

/// Pop a list and coerce every element to `f64`.
fn pop_number_list(vm: &mut VM, tag: &str) -> std::result::Result<Vec<f64>, BundError> {
    let items = pull(vm, tag)
        .map_err(|e| easy_error::err_msg(e.to_string()))?
        .cast_list()
        .map_err(|e| easy_error::err_msg(format!("{tag}: expected a list, e.g. `[ 1 2 3 ] {tag}` ({e})")))?;
    items.iter().map(as_f64).collect()
}

/// `sum` — `( list -- Σ )`.
fn r_sum(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let xs = pop_number_list(vm, "calc.sum")?;
    push(vm, Value::from_float(xs.iter().sum()));
    Ok(vm)
}

/// `mean` — `( list -- x̄ )`.
fn r_mean(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let xs = pop_number_list(vm, "calc.mean")?;
    if xs.is_empty() {
        return Err(easy_error::err_msg("calc.mean: empty list"));
    }
    push(vm, Value::from_float(xs.iter().sum::<f64>() / xs.len() as f64));
    Ok(vm)
}

/// `min` — `( list -- m )`.
fn r_min(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let xs = pop_number_list(vm, "calc.min")?;
    match xs.iter().copied().reduce(f64::min) {
        Some(m) => push(vm, Value::from_float(m)),
        None => return Err(easy_error::err_msg("calc.min: empty list")),
    }
    Ok(vm)
}

/// `max` — `( list -- m )`.
fn r_max(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let xs = pop_number_list(vm, "calc.max")?;
    match xs.iter().copied().reduce(f64::max) {
        Some(m) => push(vm, Value::from_float(m)),
        None => return Err(easy_error::err_msg("calc.max: empty list")),
    }
    Ok(vm)
}

// ── climate, geography & economy (R4-C) ──────────────────────────────────────
const EARTH_RADIUS_KM: f64 = 6371.0;

/// `lapse_rate` — `( Δh_m -- ΔT )`: environmental Γ = 6.5 K/km.
fn c_lapse_rate(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let dh = pop_f(vm, "lapse_rate")?;
    push(vm, Value::from_float(-6.5 * dh / 1000.0));
    Ok(vm)
}

/// `dewpoint` — `( T RH -- Td )`: Magnus formula. T,Td [°C], RH [%].
fn c_dewpoint(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let rh = pop_f(vm, "dewpoint")?;
    let t = pop_f(vm, "dewpoint")?;
    if rh <= 0.0 {
        return Err(easy_error::err_msg("dewpoint: humidity must be > 0"));
    }
    let (a, b) = (17.625_f64, 243.04_f64);
    let alpha = (a * t) / (b + t) + (rh / 100.0).ln();
    push(vm, Value::from_float(b * alpha / (a - alpha)));
    Ok(vm)
}

/// `insolation_at_lat` — `( lat_deg decl_deg -- H )`: daily-mean insolation proxy
/// [W/m²]. `H = (1361/π)·(h0·sinφ·sinδ + cosφ·cosδ·sin h0)`.
fn c_insolation_at_lat(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let decl = pop_f(vm, "insolation_at_lat")?.to_radians();
    let lat = pop_f(vm, "insolation_at_lat")?.to_radians();
    // Sunset hour angle (clamped for polar day / night).
    let cos_h0 = (-lat.tan() * decl.tan()).clamp(-1.0, 1.0);
    let h0 = cos_h0.acos();
    let s0 = 1361.0 / std::f64::consts::PI;
    let h = s0 * (h0 * lat.sin() * decl.sin() + lat.cos() * decl.cos() * h0.sin());
    push(vm, Value::from_float(h.max(0.0)));
    Ok(vm)
}

/// `heat_index` — `( T RH -- HI )`: NWS Rothfusz regression. T,HI [°F], RH [%].
fn c_heat_index(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let rh = pop_f(vm, "heat_index")?;
    let t = pop_f(vm, "heat_index")?;
    let hi = -42.379 + 2.049_015_23 * t + 10.143_331_27 * rh - 0.224_755_41 * t * rh
        - 0.006_837_83 * t * t
        - 0.054_817_17 * rh * rh
        + 0.001_228_74 * t * t * rh
        + 0.000_852_82 * t * rh * rh
        - 0.000_001_99 * t * t * rh * rh;
    push(vm, Value::from_float(hi));
    Ok(vm)
}

/// `haversine` — `( lat1 lon1 lat2 lon2 -- km )`: great-circle distance.
fn c_haversine(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let lon2 = pop_f(vm, "haversine")?.to_radians();
    let lat2 = pop_f(vm, "haversine")?.to_radians();
    let lon1 = pop_f(vm, "haversine")?.to_radians();
    let lat1 = pop_f(vm, "haversine")?.to_radians();
    let (dlat, dlon) = (lat2 - lat1, lon2 - lon1);
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    push(vm, Value::from_float(EARTH_RADIUS_KM * c));
    Ok(vm)
}

/// `bearing` — `( lat1 lon1 lat2 lon2 -- deg )`: initial bearing, 0–360.
fn c_bearing(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let lon2 = pop_f(vm, "bearing")?.to_radians();
    let lat2 = pop_f(vm, "bearing")?.to_radians();
    let lon1 = pop_f(vm, "bearing")?.to_radians();
    let lat1 = pop_f(vm, "bearing")?.to_radians();
    let dlon = lon2 - lon1;
    let y = dlon.sin() * lat2.cos();
    let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * dlon.cos();
    let deg = (y.atan2(x).to_degrees() + 360.0) % 360.0;
    push(vm, Value::from_float(deg));
    Ok(vm)
}

/// `destination_point` — `( lat1 lon1 bearing_deg d_km -- lat2 lon2 )` (pushes two).
fn c_destination_point(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let d = pop_f(vm, "destination_point")?;
    let brng = pop_f(vm, "destination_point")?.to_radians();
    let lon1 = pop_f(vm, "destination_point")?.to_radians();
    let lat1 = pop_f(vm, "destination_point")?.to_radians();
    let ad = d / EARTH_RADIUS_KM; // angular distance
    let lat2 = (lat1.sin() * ad.cos() + lat1.cos() * ad.sin() * brng.cos()).asin();
    let lon2 = lon1
        + (brng.sin() * ad.sin() * lat1.cos()).atan2(ad.cos() - lat1.sin() * lat2.sin());
    push(vm, Value::from_float(lat2.to_degrees()));
    push(vm, Value::from_float(lon2.to_degrees()));
    Ok(vm)
}

/// `slope` — `( rise run -- grade )`: rise/run (unitless).
fn c_slope(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let run = pop_f(vm, "slope")?;
    let rise = pop_f(vm, "slope")?;
    if run == 0.0 {
        return Err(easy_error::err_msg("slope: run must be ≠ 0"));
    }
    push(vm, Value::from_float(rise / run));
    Ok(vm)
}

/// `compound` — `( P r n t -- A )`: `A = P·(1+r/n)^(n·t)`.
fn e_compound(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let t = pop_f(vm, "compound")?;
    let n = pop_f(vm, "compound")?;
    let r = pop_f(vm, "compound")?;
    let p = pop_f(vm, "compound")?;
    if n == 0.0 {
        return Err(easy_error::err_msg("compound: periods/yr must be ≠ 0"));
    }
    push(vm, Value::from_float(p * (1.0 + r / n).powf(n * t)));
    Ok(vm)
}

/// `cagr` — `( begin end yrs -- g )`: compound annual growth rate.
fn e_cagr(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let yrs = pop_f(vm, "cagr")?;
    let end = pop_f(vm, "cagr")?;
    let begin = pop_f(vm, "cagr")?;
    if begin == 0.0 || yrs == 0.0 {
        return Err(easy_error::err_msg("cagr: begin and years must be ≠ 0"));
    }
    push(vm, Value::from_float((end / begin).powf(1.0 / yrs) - 1.0));
    Ok(vm)
}

/// `inflation_adjust` — `( nominal i yrs -- real )`: `nominal / (1+i)^yrs`.
fn e_inflation_adjust(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let yrs = pop_f(vm, "inflation_adjust")?;
    let i = pop_f(vm, "inflation_adjust")?;
    let nominal = pop_f(vm, "inflation_adjust")?;
    push(vm, Value::from_float(nominal / (1.0 + i).powf(yrs)));
    Ok(vm)
}

/// `annuity` — `( PMT r n -- PV )`: present value of an ordinary annuity.
fn e_annuity(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let n = pop_f(vm, "annuity")?;
    let r = pop_f(vm, "annuity")?;
    let pmt = pop_f(vm, "annuity")?;
    if r == 0.0 {
        return Err(easy_error::err_msg("annuity: rate must be ≠ 0"));
    }
    push(vm, Value::from_float(pmt * (1.0 - (1.0 + r).powf(-n)) / r));
    Ok(vm)
}

/// `malthus` — `( N0 r t -- N )`: exponential growth `N0·e^(r·t)`.
fn e_malthus(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let t = pop_f(vm, "malthus")?;
    let r = pop_f(vm, "malthus")?;
    let n0 = pop_f(vm, "malthus")?;
    push(vm, Value::from_float(n0 * (r * t).exp()));
    Ok(vm)
}

/// `logistic` — `( N0 K r t -- N )`: carrying-capacity growth.
fn e_logistic(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let t = pop_f(vm, "logistic")?;
    let r = pop_f(vm, "logistic")?;
    let k = pop_f(vm, "logistic")?;
    let n0 = pop_f(vm, "logistic")?;
    if n0 == 0.0 {
        return Err(easy_error::err_msg("logistic: initial population must be ≠ 0"));
    }
    let n = k / (1.0 + ((k - n0) / n0) * (-r * t).exp());
    push(vm, Value::from_float(n));
    Ok(vm)
}

// ── World-book readers (R4-D) ────────────────────────────────────────────────
//
// `calc.world.*` pull this project's own World-book facts into `/calc`. Read-only
// over the materialized World book via `crate::world::calc_read`; a path that
// doesn't resolve pushes NODATA (never a fabricated value). Each read is recorded
// (path + rendered value) so `/calc` can echo the source and record `world:<path>`
// provenance on a derived `/fact`.

thread_local! {
    static WORLD_READS: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

fn record_world_read(path: &str, rendered: &str) {
    WORLD_READS.with(|r| r.borrow_mut().push((path.to_string(), rendered.to_string())));
}

/// Drain the World-fact paths read since the last call (clears the buffer).
pub(crate) fn take_world_reads() -> Vec<(String, String)> {
    WORLD_READS.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

/// The numeric value at a World path, or `None` when absent / non-numeric.
fn world_number(path: &str) -> Option<f64> {
    let store = crate::scripting::active_store()?;
    crate::world::calc_read::lookup(store, path)?.as_f64()
}

/// Resolve `path` to a number and push it (or push NODATA), recording the read.
fn push_world(vm: &mut VM, path: &str) {
    match world_number(path) {
        Some(x) => {
            record_world_read(path, &format_float(x));
            push(vm, Value::from_float(x));
        }
        None => {
            record_world_read(path, "NODATA");
            push(vm, Value::nodata());
        }
    }
}

/// Compact float rendering for the source echo (drops a trailing `.0`).
fn format_float(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

/// `calc.world.get` — `( path -- float | NODATA )`.
fn world_get(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let path = pull(vm, "calc.world.get")
        .map_err(|e| easy_error::err_msg(e.to_string()))?
        .cast_string()
        .map_err(|e| easy_error::err_msg(format!("calc.world.get: expected a path string ({e})")))?;
    push_world(vm, &path);
    Ok(vm)
}

/// `calc.world.check` — `( path computed -- delta )`: the signed difference
/// (declared − computed) between a World-book fact and a `/calc`-recomputed value,
/// e.g. `world.star_mass world.au kepler_period  "Astronomy/year_length_planet_days"  swap  calc.world.check`.
/// Surfaces the same divergence WORLD-4's `year_length_divergence_pct` reports.
fn world_check(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let computed = pop_f(vm, "calc.world.check")?;
    let path = pull(vm, "calc.world.check")
        .map_err(|e| easy_error::err_msg(e.to_string()))?
        .cast_string()
        .map_err(|e| easy_error::err_msg(format!("calc.world.check: expected a path string ({e})")))?;
    match world_number(&path) {
        Some(declared) => {
            let delta = declared - computed;
            record_world_read(&path, &format!("{} (Δ {:+})", format_float(declared), delta));
            push(vm, Value::from_float(delta));
        }
        None => {
            record_world_read(&path, "NODATA");
            push(vm, Value::nodata());
        }
    }
    Ok(vm)
}

/// `calc.world.has` — `( path -- bool )`: does the path resolve to anything?
fn world_has(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let path = pull(vm, "calc.world.has")
        .map_err(|e| easy_error::err_msg(e.to_string()))?
        .cast_string()
        .map_err(|e| easy_error::err_msg(format!("calc.world.has: expected a path string ({e})")))?;
    let ok = crate::scripting::active_store()
        .and_then(|s| crate::world::calc_read::lookup(s, &path))
        .is_some();
    push(vm, Value::from_bool(ok));
    Ok(vm)
}

/// `calc.world.dict` — `( chapter -- dict | NODATA )`: a whole layer as a dict.
fn world_dict(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let ch = pull(vm, "calc.world.dict")
        .map_err(|e| easy_error::err_msg(e.to_string()))?
        .cast_string()
        .map_err(|e| easy_error::err_msg(format!("calc.world.dict: expected a chapter ({e})")))?;
    let value = crate::scripting::active_store()
        .and_then(|s| crate::world::calc_read::chapter(s, &ch))
        .map(|j| json_to_value(&j))
        .unwrap_or_else(Value::nodata);
    record_world_read(&ch, "{dict}");
    push(vm, value);
    Ok(vm)
}

/// Recursively convert a JSON value to a Bund `Value`.
fn json_to_value(j: &Json) -> Value {
    match j {
        Json::Null => Value::nodata(),
        Json::Bool(b) => Value::from_bool(*b),
        Json::Number(n) => match n.as_i64() {
            Some(i) => Value::from_int(i),
            None => Value::from_float(n.as_f64().unwrap_or(0.0)),
        },
        Json::String(s) => Value::from_string(s),
        Json::Array(a) => Value::from_list(a.iter().map(json_to_value).collect()),
        Json::Object(o) => {
            let map: HashMap<String, Value> =
                o.iter().map(|(k, v)| (k.clone(), json_to_value(v))).collect();
            Value::from_dict(map)
        }
    }
}

/// A convenience World constant: push the number at a fixed path (or NODATA).
macro_rules! kworld {
    ($fn:ident, $path:expr) => {
        fn $fn(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            push_world(vm, $path);
            Ok(vm)
        }
    };
}

// Fixed-path conveniences for the materialized astronomy figures the formula
// words consume most (paths mirror `materialize_astronomy`'s JSON keys).
kworld!(w_year, "Astronomy/year_length_planet_days");
kworld!(w_declared_year, "Astronomy/declared_year_length_days");
kworld!(w_tilt, "Astronomy/axial_tilt_deg");
kworld!(w_star_mass, "Astronomy/stellar_mass_solar");
kworld!(w_orbit_days, "Astronomy/orbital_period_days_earth");
kworld!(w_divergence, "Astronomy/year_length_divergence_pct");

/// `(calc.<name>, fn)` table — the short alias is the suffix after `calc.`.
const WORDS: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
    ("calc.pi", k_pi),
    ("calc.tau", k_tau),
    ("calc.e", k_e),
    ("calc.c", k_c),
    ("calc.grav", k_grav),
    ("calc.gee", k_gee),
    ("calc.au", k_au),
    ("calc.ly", k_ly),
    ("calc.pc", k_pc),
    ("calc.year", k_year),
    ("calc.day", k_day),
    ("calc.hour", k_hour),
    ("calc.minute", k_minute),
    ("calc.km2mi", c_km2mi),
    ("calc.mi2km", c_mi2km),
    ("calc.m2ft", c_m2ft),
    ("calc.ft2m", c_ft2m),
    ("calc.m2mi", c_m2mi),
    ("calc.mi2m", c_mi2m),
    ("calc.c2f", c_c2f),
    ("calc.f2c", c_f2c),
    ("calc.c2k", c_c2k),
    ("calc.k2c", c_k2c),
    ("calc.kg2lb", c_kg2lb),
    ("calc.lb2kg", c_lb2kg),
    ("calc.kmh2mph", c_kmh2mph),
    ("calc.mph2kmh", c_mph2kmh),
    ("calc.au2km", c_au2km),
    ("calc.km2au", c_km2au),
    ("calc.ly2km", c_ly2km),
    ("calc.km2ly", c_km2ly),
    ("calc.ly2au", c_ly2au),
    ("calc.au2ly", c_au2ly),
    ("calc.pc2ly", c_pc2ly),
    ("calc.ly2pc", c_ly2pc),
    ("calc.deg2rad", c_deg2rad),
    ("calc.rad2deg", c_rad2deg),
    // Scientific math substrate (R4-A).
    ("calc.sqrt", m_sqrt),
    ("calc.cbrt", m_cbrt),
    ("calc.exp", m_exp),
    ("calc.ln", m_ln),
    ("calc.log10", m_log10),
    ("calc.log2", m_log2),
    ("calc.sin", m_sin),
    ("calc.cos", m_cos),
    ("calc.tan", m_tan),
    ("calc.asin", m_asin),
    ("calc.acos", m_acos),
    ("calc.atan", m_atan),
    ("calc.abs", m_abs),
    ("calc.floor", m_floor),
    ("calc.ceil", m_ceil),
    ("calc.round", m_round),
    ("calc.pow", m_pow),
    ("calc.atan2", m_atan2),
    ("calc.hypot", m_hypot),
    // Astronomy & planetology (R4-B).
    ("calc.kepler_period", a_kepler_period),
    ("calc.surface_gravity", a_surface_gravity),
    ("calc.escape_velocity", a_escape_velocity),
    ("calc.insolation", a_insolation),
    ("calc.synodic_period", a_synodic_period),
    ("calc.angular_size", a_angular_size),
    ("calc.hill_sphere", a_hill_sphere),
    ("calc.roche_limit", a_roche_limit),
    ("calc.tidal_accel", a_tidal_accel),
    // Scientific math completion (R4-A).
    ("calc.trunc", m_trunc),
    ("calc.sign", m_sign),
    ("calc.factorial", m_factorial),
    ("calc.gcd", m_gcd),
    ("calc.lcm", m_lcm),
    ("calc.sum", r_sum),
    ("calc.mean", r_mean),
    ("calc.min", r_min),
    ("calc.max", r_max),
    // Climate, geography & economy (R4-C).
    ("calc.lapse_rate", c_lapse_rate),
    ("calc.dewpoint", c_dewpoint),
    ("calc.insolation_at_lat", c_insolation_at_lat),
    ("calc.heat_index", c_heat_index),
    ("calc.haversine", c_haversine),
    ("calc.bearing", c_bearing),
    ("calc.destination_point", c_destination_point),
    ("calc.slope", c_slope),
    ("calc.compound", e_compound),
    ("calc.cagr", e_cagr),
    ("calc.inflation_adjust", e_inflation_adjust),
    ("calc.annuity", e_annuity),
    ("calc.malthus", e_malthus),
    ("calc.logistic", e_logistic),
    // World-book readers (R4-D).
    ("calc.world.get", world_get),
    ("calc.world.has", world_has),
    ("calc.world.dict", world_dict),
    ("calc.world.check", world_check),
    ("calc.world.year", w_year),
    ("calc.world.declared_year", w_declared_year),
    ("calc.world.tilt", w_tilt),
    ("calc.world.star_mass", w_star_mass),
    ("calc.world.orbit_days", w_orbit_days),
    ("calc.world.divergence", w_divergence),
];

pub fn register(vm: &mut VM) -> Result<()> {
    for (name, f) in WORDS {
        vm.register_inline(name.to_string(), *f).map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    // Best-effort short aliases (`calc.mi2km` → `mi2km`); collisions are ignored,
    // leaving the namespaced form usable.
    for (name, _) in WORDS {
        if let Some(short) = name.strip_prefix("calc.") {
            let _ = vm.register_alias(short.to_string(), name.to_string());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::scripting;

    fn top_float(code: &str) -> f64 {
        let out = scripting::eval(code).expect("eval");
        out.top.expect("a result").cast_float().expect("float")
    }

    #[test]
    fn conversions_via_short_alias() {
        assert!((top_float("100 mi2km") - 160.9344).abs() < 1e-6);
        assert!((top_float("0 c2f") - 32.0).abs() < 1e-9);
        assert!((top_float("100 c2f") - 212.0).abs() < 1e-9);
    }

    #[test]
    fn constants_and_namespaced_form() {
        assert!((top_float("calc.pi") - std::f64::consts::PI).abs() < 1e-12);
        // light-year in metres
        assert!((top_float("calc.ly") - 9.460_730_472_580_8e15).abs() < 1.0);
    }

    #[test]
    fn math_substrate() {
        assert!((top_float("9 calc.sqrt") - 3.0).abs() < 1e-9);
        assert!((top_float("2 10 calc.pow") - 1024.0).abs() < 1e-9);
        assert!((top_float("3 4 calc.hypot") - 5.0).abs() < 1e-9);
    }

    #[test]
    fn astronomy_known_values() {
        // Kepler: a=4 AU, M=1 M☉ → T = √(64) = 8 yr.
        assert!((top_float("4 1 calc.kepler_period") - 8.0).abs() < 1e-9);
        // Earth: M=1, R=1 → g₀ = 9.80665 m/s².
        assert!((top_float("1 1 calc.surface_gravity") - 9.806_65).abs() < 1e-6);
        // Escape: M=1, R=1 → 11186 m/s.
        assert!((top_float("1 1 calc.escape_velocity") - 11_186.0).abs() < 1e-6);
        // Insolation: L=1 L☉, d=2 AU → 1361 / 4 = 340.25 W/m².
        assert!((top_float("1 2 calc.insolation") - 340.25).abs() < 1e-6);
        // Synodic: T1=1, T2=1.5 → 1/(1 − 2/3) = 3.
        assert!((top_float("1 1.5 calc.synodic_period") - 3.0).abs() < 1e-9);
        // Roche (fluid): R=1, ρM=1, ρm=1 → 2.44.
        assert!((top_float("1 1 1 calc.roche_limit") - 2.44).abs() < 1e-9);
    }

    #[test]
    fn climate_geo_economy() {
        // lapse rate: 1000 m → −6.5 °C.
        assert!((top_float("1000 calc.lapse_rate") + 6.5).abs() < 1e-9);
        // haversine London→New York ≈ 5570 km.
        let d = top_float("51.5 -0.13 40.71 -74.0 calc.haversine");
        assert!((d - 5570.0).abs() < 20.0, "got {d}");
        // slope 3/4.
        assert!((top_float("3 4 calc.slope") - 0.75).abs() < 1e-9);
        // compound: 1000 at 5%/yr, annual, 10 yr → 1628.894…
        assert!((top_float("1000 0.05 1 10 calc.compound") - 1628.894_626_777).abs() < 1e-6);
        // CAGR 100→200 over 10 yr → 2^0.1 − 1.
        assert!((top_float("100 200 10 calc.cagr") - (2f64.powf(0.1) - 1.0)).abs() < 1e-9);
        // Malthus: N0=100, r=0, t=5 → 100.
        assert!((top_float("100 0 5 calc.malthus") - 100.0).abs() < 1e-9);
        // Logistic at t=0 → N0.
        assert!((top_float("10 100 0.1 0 calc.logistic") - 10.0).abs() < 1e-9);
        // gcd / factorial.
        assert!((top_float("12 18 calc.gcd") - 6.0).abs() < 1e-9);
        assert!((top_float("5 calc.factorial") - 120.0).abs() < 1e-9);
    }

    #[test]
    fn list_reducers() {
        assert!((top_float("[ 1 2 3 4 ] calc.sum") - 10.0).abs() < 1e-9);
        assert!((top_float("[ 1 2 3 4 ] calc.mean") - 2.5).abs() < 1e-9);
        assert!((top_float("[ 5 2 8 1 ] calc.min") - 1.0).abs() < 1e-9);
        assert!((top_float("[ 5 2 8 1 ] calc.max") - 8.0).abs() < 1e-9);
        // Floats + a longer chain: mean of [1.5 2.5] = 2.0.
        assert!((top_float("[ 1.5 2.5 ] calc.mean") - 2.0).abs() < 1e-9);
    }

    #[test]
    fn world_check_nodata_without_project() {
        // No active store → the path can't resolve → NODATA (never a number).
        let out = crate::scripting::eval("\"Astronomy/year_length_planet_days\" 400 calc.world.check")
            .expect("eval");
        assert!(out.top.unwrap().cast_float().is_err(), "should be NODATA, not a number");
    }

    #[test]
    fn destination_point_pushes_two() {
        // From the equator/prime meridian, bearing 90° (east), ~111.32 km ≈ 1° lon.
        let out = crate::scripting::eval("0 0 90 111.195 calc.destination_point").expect("eval");
        // top is lon2 ≈ 1.0, and a lat2 ≈ 0 remains beneath it.
        let lon2 = out.top.unwrap().cast_float().unwrap();
        assert!((lon2 - 1.0).abs() < 0.01, "lon2={lon2}");
    }
}
