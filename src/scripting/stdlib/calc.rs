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
    // World-book readers (R4-D).
    ("calc.world.get", world_get),
    ("calc.world.has", world_has),
    ("calc.world.dict", world_dict),
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
}
