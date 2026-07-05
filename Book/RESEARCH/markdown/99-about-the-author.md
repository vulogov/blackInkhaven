# About the Author

_Vladimir Ulogov._

Vladimir Ulogov has spent decades building infrastructure for distributed systems — the kind of software that watches other software. Early in his career he worked on monitoring and telemetry platforms; later years took him into federated observability, telemetry buses, and the architecture of systems that have to make sense of millions of data points without losing the thread.

Observability, in the end, is a discipline of **grounding** — of never reporting a number the system cannot back up, of knowing the provenance of every signal. It is not an accident that a tool he built for writers carries the same instinct into the library.

What makes him slightly unusual in his corner of the industry is a tendency to write his own tools — not in the sense of small utilities, but in the sense of programming languages. The Bund language (its compiler, its VM, its document store, its parser) lives in a long series of Rust crates on crates.io. `rust_dynamic`, `rust_multistack`, `rust_multistackvm`, `bundcore`, `zbus_universal_data_gateway` — each is a building block that exists because the off-the-shelf options didn't fit the shape of the work.

## Why the Research Assistant exists

Inkhaven is Vladimir's personal reflection on how a literary tool can help the people who write books. The Research Assistant is one answer to a specific frustration: that the tools writers use to **gather** facts and the tools they use to **write** live in different worlds, and the seam between them is where errors and lost citations breed.

A novelist keeps a browser open in one window and a manuscript in another, and the fact they looked up on Tuesday is gone by Thursday. A non-fiction author tracks citations in one program, prose in a second, and prays the two stay in sync. Neither tool remembers **where a fact came from** once it lands in the draft. The Research Assistant was built to close that gap: to make research a room inside the writing tool, where every fact you keep remembers its own provenance and can be composed straight back into the page.

## A work of love

Inkhaven is open source. The licence is permissive — you can read it, fork it, study it, modify it, and pass it on. Strictly speaking, the licence also lets you sell it. The author would, gently but firmly, disagree with you doing that. Inkhaven was not designed as a *for sale* project. It is a work of love made for the authors who can least afford to pay for software, and turning it into a commercial product would betray the reason it exists. So please — don't.

It carries no analytics, no telemetry, no upsell. The binary will never phone home; the project will never have an "Enterprise" tier you have to escape from.

This was a deliberate choice. There are excellent commercial tools for research and writing. They cost money — which is fine for many writers, and a barrier for many more. Inkhaven exists for the second group: for the graduate student writing a dissertation on a battered laptop, for the novelist who shouldn't have to pick between rent and software, for the engineer drafting in the same terminal where they already write code, for anyone who would benefit from a tool that respects their work without asking for a credit-card number.

## A note on cooperation

Vladimir believes firmly in the human capacity for mutual help — that we make better work, and live better lives, when we share what we know and what we build. Open source is one of the most concrete expressions of cooperation our era has produced: code read, improved, and passed forward without payment, without permission, by people who will never meet.

If Inkhaven helps you finish your book — that is enough. If it gives you a chord pattern you adapt into your own tool — that's a gift back to the larger project of making software writers can love. As a society, we achieve the greatest things when we help each other rather than compete with each other.

## Where to find more

- `GitHub` — @vulogov — the source for Inkhaven, Bund, and the dozen-plus Rust crates that carry the infrastructure. Issues and PRs welcome.
- `LinkedIn` — /in/vladimirulogov — posts on observability, the occasional long-form essay.
- `YouTube` — @vulogov — talks and walkthroughs from the conference trail.
- `X / Twitter` — @vladimir_ulogov

_If a fact you grounded ever surprises you, or a command trips you up and you can't find the answer in this book — open an issue on GitHub. The author reads them._
