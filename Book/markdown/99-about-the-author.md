# About the author

![figure: author-portrait](images/author-portrait.png) — Vladimir Ulogov.

Vladimir Ulogov has spent two decades building infrastructure for observability — the systems that watch other systems. He is currently a Lead Architect at Amadeus, working on telemetry-bus designs for federated observability platforms. Before Amadeus he was at New Relic, where the question of "how do you keep track of a thousand machines without losing your mind" first turned into something resembling an answer.

What makes him slightly unusual in his corner of the industry is a tendency to write his own tools — not in the sense of small utilities, but in the sense of programming languages. The Bund language (its compiler, its VM, its document store, its parser) lives in some two dozen Rust crates published to crates.io. ZB-script preceded it, designed specifically for the federated-observability problem he was working on at the time. `rust_dynamic`, `rust_multistack`, `rust_multistackvm`, `bundcore`, `zbus_universal_data_gateway` — each is a building block that exists because the off-the-shelf options didn't fit the shape of the work.

## Inkhaven, in that light

Inkhaven follows the same instinct. It is a TUI editor for Typst books, not a fork of an existing editor with Typst support bolted on. The database (`bdslib`), the scripting layer (Bund), the multilingual stemmer integration, the in-process Typst engine, the AI surface gated by a sandbox policy — every piece chosen or built because the path of least resistance produces work the author actually wants to use.

The result is software written in the spirit of a craftsman's workshop: more lathes than templates, more sharpened tools than purchased ones. Whether you find that a virtue or a quirk depends on whether you'd rather your editor know what you wanted or do what you said.

## Outside the terminal

Vladimir lives in Ogden, Utah. He writes prose. He has been known to read more than is strictly healthy. The book in your hands exists because he wanted a place to put that habit — and because if a writer is going to spend years inside an editor, the editor might as well respect the writing.

He is also, by his own admission, an enthusiastic proponent of the idea that good software, like good prose, benefits from being short, specific, and willing to disappear when the reader stops paying attention to it.

## Where to find more

| Where | What |
|-------|------|
| GitHub | @vulogov — 128 repositories. Most of the Rust infrastructure Inkhaven sits on (and ZB-script, and Bund) lives here. Many tagged crates.io releases. |
| LinkedIn | /in/vladimirulogov — career notes, posts on federated observability, the occasional essay on telemetry types. |
| YouTube | @vulogov — talks and walkthroughs. |
| X / Twitter | @vladimir_ulogov |
| ResearchGate | research profile under Vladimir Ulogov — peer-reviewed work in adjacent fields. |

*If you build something on top of Inkhaven, or if a chord trips you up and you can't find the answer in this book — open an issue on GitHub. The author reads them.*

---

*End of the Book of Inkhaven.*
