#import "../design.typ": *

#chapter(number: 4, title: "The Sky")

Every world begins with a sky. Not because the sky is where a reader looks
first — most never look up at all — but because everything a reader *does*
notice hangs from it. The length of a year, the turn of the seasons, the tides
that time a harbour, the very difference between a temperate coast and a frozen
one: all of it descends from a star, a planet, and the tilt of that planet as it
swings around the star. In Inkhaven the world is grown one layer at a time, and
the first layer — the one every later layer leans on — is *astronomy*. This is
the root. Get the sky right and the land, the weather, and the rivers have a
firm thing to grow from. Get it careless and every layer above it inherits the
carelessness.

#insight[
  The sky is the root of the world. Axial tilt drives the seasons; the seasons
  drive the climate; the climate carves the rivers and decides where people can
  live. When you set the astronomy, you are not describing a backdrop — you are
  setting the initial conditions from which the whole physical world falls out.
  Everything downstream inherits whatever you decide up here.
]

#section("What the astronomy block holds")

The whole sky lives in one block of your `world.hjson`. It reads almost like a
form — a star, the planet that orbits it, the shape of that orbit, any moons,
and the calendar your people keep. Here is the shape of it:

```
astronomy: {
  star:   { class: "G2V", luminosity_solar: 1.0, age_gyr: 4.6 }
  planet: {
    mass_earth: 1.0, radius_earth: 1.0,
    axial_tilt_deg: 23.4,
    day_length_hours: 24.0,
    rotation_direction: "prograde"
  }
  orbit:  { semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365.25 }
  moons: [
    { name: "Luna", mass_lunar: 1.0, period_days: 27.3 }
  ]
  calendar: {
    months: 12, month_length_days: 30, weekdays: 7,
    month_names: ["Frostmoon", "Thawmoon", "..."],
    new_year_aligns_to: "spring_equinox"
  }
}
```

You do not have to fill every field to something exotic. Copy Earth's numbers,
as above, and you have a familiar sky you can trust while you learn — a real
starting point, not a placeholder. Change one number at a time and watch what
moves. Swap the Sun for a cooler, redder star and the whole energy budget shifts:

#hjson[```
astronomy: {
  star: { class: "K", luminosity_solar: 0.6 }
}
```]

#term("Axial tilt")[
  The angle between a planet's spin axis and the line straight up from its orbit
  — Earth's is about 23.4°. It is the single most consequential number in the
  sky, because it is *why there are seasons at all*: as the planet circles its
  star, the tilt leans first one hemisphere and then the other toward the light,
  so the same place gets more sun in summer and less in winter.
]

#section("What the sky computes")

Compiling the astronomy layer does more than tidy your numbers back to you. From
the orbit and the planet's day, Inkhaven works out how long a year actually is
*in that world's own days* — the `year_length_days` of the orbit divided by the
`day_length_hours` of the planet — because a story is dated in days, not in some
abstract fraction of an orbit. A world with an eighteen-hour day and a long slow
orbit can run to five hundred days a year, and every calendar you build must
answer to that number.

From the orbit and the tilt it finds the *four season markers* — the two
equinoxes and the two solstices — the four moments that pin the year in place.

#term("Solstice / Equinox")[
  The four turning points of the year. At an *equinox* the tilt leans neither
  hemisphere toward the star, so day and night are equal the world over — these
  are the balance points of spring and autumn. At a *solstice* one hemisphere
  leans most toward the star (its longest, brightest day) while the other leans
  away (its shortest) — the peaks of summer and the depths of winter.
]

From the tilt it also decides how *strong* the seasons are. This is the quiet
power of that one field: a world with almost no tilt has a mild, monotonous
year — perpetual near-equinox, little difference between the solstices — while a
sharply tilted world has ferocious summers and brutal winters, because far more
of the star's light lands on the leaning hemisphere. Push the tilt well past
Earth's to feel it:

#hjson[```
astronomy: {
  planet: { axial_tilt_deg: 35 }
}
```]

That quantity has a name.

#term("Insolation")[
  The amount of a star's light and heat falling on a given patch of ground over
  a given time — literally, incoming solar radiation. It varies with latitude
  (the poles catch the light at a glancing angle, the tropics head-on) and with
  the season (the tilt swings each hemisphere toward and away from the star).
  Insolation is the raw energy budget the climate layer will spend; the sky is
  where it is first computed.
]

The compiler works out insolation for each latitude, which is precisely the
number the *climate* layer will pick up in the next chapter and turn into
temperature and rain. The sky hands the land its energy; the land does the rest.

#subsection("Moons, synodic periods, and tides")

If you gave your world moons, the sky computes their *synodic periods* and the
tides they raise. A world with three moons has a busy, layered sky and complex
tides; a world with none has still seas and dark nights. Hang a second, smaller
moon on a faster orbit and the tides gain a second beat:

#hjson[```
astronomy: {
  moons: [
    { name: "Pale", mass_lunar: 1.0, period_days: 27.3 }
    { name: "Ember", mass_lunar: 0.4, period_days: 9.1 }
  ]
}
```]

#term("Synodic period")[
  The time a moon takes to return to the same phase as seen from the ground — new
  moon to new moon. It differs from the moon's raw orbital period because the
  planet is itself moving around the star meanwhile. The synodic period is the
  one your characters would actually keep time by, so it is the one worth
  knowing: it is the rhythm of a lunar month.
]

#subsection("The calendar-consistency check")

Last, the sky checks your *calendar* against the orbit it just computed. You
declared a year — some number of months of some number of days. The compiler
knows the true year length in planet-days. If the two disagree, it says so. A
calendar is a human artefact laid over an astronomical fact, and the check keeps
the artefact honest.

#pitfall[
  The commonest sky mistake is declaring a calendar that quietly contradicts the
  orbit — twelve months of thirty days (a 360-day year) sitting on top of an
  orbit that really runs 365.25 days, with nothing to reconcile the missing five
  and a quarter. The consistency check flags exactly this. Either adjust your
  months, or decide deliberately that your people's calendar *drifts* against the
  seasons — a genuine, story-rich choice — but do not leave the contradiction
  there by accident.
]

#section("Compiling the sky, and the calendar bridge")

To grow just this layer and read what it found, compile it alone:

```
realworld compile --layer astronomy
```

You will see the year in planet-days, the four season markers, the per-latitude
insolation, the moons' synodic periods, and the verdict of the calendar check.
This is the whole sky, computed and laid out, before any land exists to stand
under it.

#note[
  The sky is also the source of your story's *calendar*. Run `realworld calendar` and Inkhaven derives a story-Timeline calendar from the astronomy —
  months, weekdays, the alignment of the new year to a season marker — as a set
  of lines you can adopt into `timeline.calendar`. The world proposes the
  calendar; you adopt it. Nothing reaches your Timeline until you choose to let
  it.
]

That bridge matters: it means the date at the top of a scene and the season your
character sees out the window are computed from the *same* sky. They cannot drift
apart, because they share a root.

#question[
  How alien is your sky? One sun or two? A single familiar moon, or three that
  cross the night at different speeds? A brisk year, or a long slow one where a
  child might see only a handful of summers before growing up? You do not have to
  answer exotically — but answer *deliberately*, because every choice here is a
  choice the whole world below will have to live with.
]

#tryit[
  Open your `world.hjson` and change one number: `axial_tilt_deg`. Set it near
  `0` and recompile with `realworld compile --layer astronomy` — watch the
  seasons flatten toward a single endless mild one. Now set it to `40` and
  recompile — watch the summers and winters pull violently apart. You have just
  felt, in one field, the lever that drives the entire climate of your world.
]

#recap((
  [The *astronomy* layer is the *root* of the physical world — the first layer,
   and the one every later layer depends on.],
  [The `astronomy` block holds a *star*, a *planet* (crucially its
   `axial_tilt_deg`), an *orbit*, any *moons*, and a *calendar*.],
  [Compiling it yields the year in planet-days, the four *season markers*,
   per-latitude *insolation*, moon *synodic periods* and tides, and a
   *calendar-consistency check*.],
  [*Axial tilt* is the master lever: it sets how strong the seasons are, and so —
   through climate — shapes everything downstream.],
  [`realworld calendar` derives a story-Timeline calendar from the same sky, so
   your dates and your seasons can never drift apart — and you adopt it by
   choice.],
))
