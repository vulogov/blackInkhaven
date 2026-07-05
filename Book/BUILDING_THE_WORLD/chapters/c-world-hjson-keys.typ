#import "../design.typ": *

#appendix(letter: "C", title: "The world.hjson Keys")

Every value you can set in `world.hjson`, block by block. Only `name` and the
`astronomy` block are required; every other block is optional, and every layer
that lacks an explicit block is generated from the `seed`. Types are given as
`number`, `text`, `list`, or `block`; where a value has a natural default (the
Earth-like starter), it is noted.

#section("Top level")

#gloss("name")[`text` — the world's name. Required.]
#gloss("seed")[`number` — the single value that fixes every generated choice
  (coastlines, which valley grows the first city). Accepts a decimal integer or a
  `0x…` hex value. The same seed always grows the same world. Defaults to `0`.]
#gloss("primary_language")[`text` — the world's common tongue, used when labelling
  materialised output. Defaults to `"en"`.]

#section("astronomy — the sky (required)")

Holds four sub-blocks and a calendar. Everything downstream — climate, rivers,
settlement — depends on it.

#subsection("star")
#gloss("class")[`text` — spectral class, e.g. `"G2V"` (a sun like ours) or `"K"`,
  `"M"` for cooler, redder stars.]
#gloss("luminosity_solar")[`number` — brightness in units of our Sun. `1.0` is
  Sun-like; raise it for a hotter, wetter world, lower it for a colder one.]
#gloss("age_gyr")[`number` — the star's age in billions of years. Descriptive.]

#subsection("planet")
#gloss("mass_earth")[`number` — planet mass in Earth masses (`1.0` = Earth).]
#gloss("radius_earth")[`number` — planet radius in Earth radii; sets the world's
  physical size, and so the real distance a rider or ship must cross.]
#gloss("axial_tilt_deg")[`number` — the tilt of the axis in degrees. Earth is
  `23.4`. Tilt drives the *strength* of the seasons: near `0`, seasons all but
  vanish; large tilts give violent ones.]
#gloss("day_length_hours")[`number` — the length of one rotation, in hours.]
#gloss("rotation_direction")[`text` — `"prograde"` (like Earth) or `"retrograde"`,
  which flips the prevailing winds.]

#subsection("orbit")
#gloss("semi_major_axis_au")[`number` — orbital distance in astronomical units
  (`1.0` = Earth's distance from the Sun).]
#gloss("eccentricity")[`number` — how elliptical the orbit is (`0` is a circle;
  Earth is `0.017`).]
#gloss("year_length_days")[`number` — the declared length of the year in days. The
  sky computes the *true* length from the orbit and flags a year that contradicts
  it (see the calendar check).]

#subsection("moons")
A `list` of moons, each a `block`:
#gloss("name")[`text` — the moon's name.]
#gloss("mass_lunar")[`number` — mass in units of our Moon; larger moons raise
  bigger tides.]
#gloss("period_days")[`number` — orbital period in days, from which the synodic
  period (the visible month) is computed.]

#subsection("calendar")
#gloss("months")[`number` — months in the year.]
#gloss("month_length_days")[`number` — days in a month.]
#gloss("weekdays")[`number` — days in a week.]
#gloss("month_names")[`list` of `text` — optional names for the months; used when
  the calendar is adopted into the story Timeline.]
#gloss("new_year_aligns_to")[`text` — the season marker the new year begins on,
  e.g. `"winter_solstice"`, `"vernal_equinox"`.]

#section("geology — the land (optional)")

Omit this block and the land is generated from the `seed`. Include it only to
supply your own heightmap.

#gloss("dem")[`block` — bring-your-own-map. `dem.path` (`text`) is the heightmap
  image, relative to the project root; `dem.scale_km_per_pixel` (`number`) sets
  its real scale; `dem.sea_level_pixel_value` (`number`) marks the pixel level at
  or below which land is sea.]

#section("geography — named places (optional)")

Author-declared regions and landmarks. These feed the gazetteer and let the
fact-checker resolve places you name in prose.

#gloss("regions")[`list` of `block`, each: `name` (`text`), `biome` (`text`),
  `climate` (`text`), `description` (`text`).]
#gloss("landmarks")[`list` of `block`, each: `name` (`text`), `kind` (`text`, e.g.
  `"city"`, `"port"`, `"mountain"`), `climate_zone` (`text`), `population`
  (`number`), `description` (`text`). A landmark with a `climate_zone` becomes a
  gazetteer entry the fact-checker knows by name.]

#section("hydrology — named waters (optional)")

Descriptive names laid over the procedural rivers, which still run.

#gloss("rainfall")[`text` — a note: `"arid"`, `"temperate"`, or `"wet"`.]
#gloss("rivers")[`list` of `block`, each `name` (`text`) + `description` (`text`),
  and optionally a declared course: `from` and `to` (each a `[x, y]` cell). When
  both are set, the world checks the course runs downhill to water and warns if
  it does not.]
#gloss("lakes")[`list` — same shape as `rivers`.]
#gloss("seas")[`list` — same shape as `rivers`.]

#section("economy — trade and technology (optional)")

#gloss("tech_level")[`text` — e.g. `"bronze"`, `"iron"`, `"medieval"`,
  `"industrial"`.]
#gloss("currency")[`text` — the coin of the realm.]
#gloss("trade_goods")[`list` of `text` — what moves along the roads.]
#gloss("resources")[`list` of `text` — what the land yields; these are added to
  the fact-checker's known minerals, so trading a declared resource is not
  flagged.]

#section("magic — declared exceptions to physics (optional)")

#gloss("enabled")[`text`/`number` — `true` turns the ledger on; `false` gates it
  entirely.]
#gloss("rules")[`list` of `block`. Each rule: `kind` (`text`, your own label, e.g.
  `"messenger_birds"`); `covers` (`list` of `text` — which fact-check categories
  it may suppress: `astronomy`, `climate`, `climate_anomaly`, `date_coherence`,
  `demographics`, `economy`, `travel_time`, `character_age`); `description`
  (`text`); and `applicable_to` (`block` with optional `roles`, `regions`,
  `seasons` lists — an unset facet means "any"). Kind-specific parameters
  (e.g. `speed_kph_override`) may be added as extra keys on the rule.]

#section("history — declared events (optional)")

Author events merged into the generated chronology; the world checks them and
adopts place-linked ones onto the story Timeline.

#gloss("events")[`list` of `block`, each: `year` (`number` — years before the
  present, negative is the past); `title` (`text`); `epoch` (`text`, optional —
  inferred from the year when omitted); `places` (`list` of `text`, optional —
  accepted Place names, for Timeline links); `description` (`text`, optional).
  The world warns if a year is after the present or before recorded history, or
  if a declared `epoch` does not contain the year.]

#section("nations — declared realms (optional)")

A `list` of nations; each pins a named realm, and the remaining settlements
cluster into generated realms around them.

#gloss("name")[`text` — the realm's name.]
#gloss("capital")[`[x, y]` — the capital's map cell; the nearest settlement becomes
  its seat. The world warns if it sits far from any settlement (in the
  wilderness).]
#gloss("relations")[`list` of `block`, each `with` (`text` — another nation's name)
  + `stance` (`text` — `"allied"`, `"rival"`, `"neutral"`). Override the seeded
  relations.]

#section("cultures — pinned cultures (optional)")

A `list` that overrides the generated culture of a nation, matched by name.

#gloss("nation")[`text` — the nation this culture belongs to.]
#gloss("ethos")[`text`, optional — overrides the generated ethos. The world warns
  if a seafaring ethos is pinned to a dry inland capital.]
#gloss("belief")[`text`, optional — the culture's belief system.]
#gloss("language")[`text`, optional — a conlang typology profile, e.g.
  `"SOV · agglutinative · tonal"`.]

#section("ecology — pinned life (optional)")

#gloss("regions")[`list` of `block`, each: `biome` (`text` — one of the twelve
  biome names, e.g. `"hot_desert"`); `flora` (`list` of `text`); `fauna` (`list`
  of `text`); `keystone` (`text`). Overrides the generated life for that biome.
  The world warns if cold-adapted life is pinned to a hot biome (or the reverse),
  or if the biome does not occur in the world.]
