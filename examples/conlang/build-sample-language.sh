#!/usr/bin/env bash
#
# Build a complete sample constructed language end to end (LANG-1 P7).
#
#   ./build-sample-language.sh <project-path> [language-name]
#
# Creates (or reuses) an inkhaven project at <project-path> and builds a full
# language inside it: phonology, lexicon, morphology, typology, a constructed
# script whose glyphs are DRAWN BY THE AI (`glyph-draft`), a compiled TrueType
# font, and three books — a dictionary, a reference grammar, and a learner
# tutorial — as Typst documents that embed the font (compiled to PDF if Typst
# is on PATH).
#
# Requirements:
#   - the `inkhaven` binary on PATH (or set $INKHAVEN)
#   - an AI provider key for the glyph drafting. Set $PROVIDER (default:
#     deepseek) and the matching key (e.g. DEEPSEEK_API_KEY). See `inkhaven
#     language glyph-draft --help`.
#   - optional: `typst` on PATH to compile the .typ books to PDF.
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <project-path> [language-name]" >&2
  exit 2
fi

INK="${INKHAVEN:-inkhaven}"
PROJ="$1"
LANG_NAME="${2:-Avesha}"
PROVIDER="${PROVIDER:-deepseek}"
slug="$(printf '%s' "$LANG_NAME" | tr '[:upper:]' '[:lower:]')"
ART="$PROJ/conlang-out"   # where the fonts + books land

echo "▸ Project:  $PROJ"
echo "▸ Language: $LANG_NAME"
echo "▸ AI provider for glyphs: $PROVIDER"

# ── 1. Project + language book ───────────────────────────────────────────────
if [[ ! -d "$PROJ/.inkhaven" ]]; then
  "$INK" init "$PROJ" >/dev/null
fi
cd "$PROJ"
mkdir -p "$ART"
"$INK" language init "$LANG_NAME" >/dev/null 2>&1 || true

phon_dir="books/language/$slug/04-phonology"
gram_dir="books/language/$slug/03-grammar"
samp_dir="books/language/$slug/05-sample-texts"

# ── 2. Phonology ─────────────────────────────────────────────────────────────
# Inventory, syllable templates, phonotactics, allophony, stress.
# (A phonotactic constraint's discriminator key is `kind:`.)
cat > "$phon_dir/02-phonology.typ" <<'HJSON'
{
  phonemes: [
    { ipa: "p", kind: "consonant" } { ipa: "t", kind: "consonant" }
    { ipa: "k", kind: "consonant" } { ipa: "s", kind: "consonant" }
    { ipa: "m", kind: "consonant" } { ipa: "n", kind: "consonant" }
    { ipa: "l", kind: "consonant" } { ipa: "r", kind: "consonant" }
    { ipa: "a", kind: "vowel" } { ipa: "i", kind: "vowel" } { ipa: "u", kind: "vowel" }
  ]
  classes: { C: ["p","t","k","s","m","n","l","r"], V: ["a","i","u"] }
  templates: { root: [ { pattern: ["C","V"] }, { pattern: ["C","V","C"] } ] }
  constraints: [ { kind: "no_geminate" }, { kind: "max_cluster_size", value: 2 } ]
  allophony: [ { rule: "t > s / _ i" }, { rule: "n > m / _ p" } ]
  stress: { primary: "penultimate" }
}
HJSON

# ── 3. Morphology + typology + expressions + a sample text ───────────────────
cat > "$gram_dir/02-morphology.typ" <<'HJSON'
{
  morphemes: [
    { id: "DAT", gloss: "DAT", form: "ti", position: "suffix", category: "case",   value: "dative" }
    { id: "PL",  gloss: "PL",  form: "u",  position: "suffix", category: "number", value: "plural" }
  ]
  derivations: [
    { name: "agent", gloss: "one who {}s", form: "ar", position: "suffix", from_pos: "verb", to_pos: "noun" }
  ]
}
HJSON
cat > "$gram_dir/03-typology.typ" <<'HJSON'
{ grammar: { word_order: "sov", alignment: "nominative_accusative", case: "yes", adposition: "postposition" } }
HJSON
cat > "$gram_dir/04-expressions.typ" <<'HJSON'
{ idioms: [ { form: "pata nami", literal: "stone sees", meaning: "to be stubborn" } ]
  metaphors: [ { source: "river", target: "time" } ] }
HJSON
cat > "$samp_dir/02-sample.typ" <<'HJSON'
kira suna nami. tani palu.
HJSON

"$INK" reindex --adopt >/dev/null 2>&1
echo "▸ Phonology"
"$INK" language stats "$LANG_NAME" | sed 's/^/    /'

# ── 4. Lexicon ───────────────────────────────────────────────────────────────
echo "▸ Lexicon"
add() { "$INK" language add-word "$LANG_NAME" "$1" --type "$2" --translation "$3" >/dev/null; }
add pata noun stone;  add talu noun river; add kira noun bird; add suna noun sun
add kanu noun hand;   add nami verb see;   add palu verb run;  add tani verb speak
add mira adjective bright; add lasu adjective cold
"$INK" language audit "$LANG_NAME" | sed 's/^/    /'

# ── 5. AI-generated script ───────────────────────────────────────────────────
# One glyph per phoneme, DRAWN BY THE AI from a short description, run through
# the suitability preflight, and bound to the sound + a PUA codepoint. Each
# call is advisory + preflight-gated; `--yes` binds a usable result.
echo "▸ Writing system (AI-drafted glyphs via $PROVIDER)"
draft() { # phoneme  index  description
  local ph="$1" idx="$2" desc="$3"
  local cp; cp="$(printf 'U+E0%02X' "$idx")"
  if "$INK" language glyph-draft "$LANG_NAME" \
        --describe "$desc" --phoneme "$ph" --codepoint "$cp" --name "$ph" \
        --provider "$PROVIDER" --out "$ART/glyph-$ph.svg" --yes >/dev/null 2>&1; then
    echo "    ✓ $ph  ($cp)  $desc"
  else
    echo "    ⚠ $ph  — AI draft unusable, skipped"
  fi
}
draft p 0  "a bold vertical post with a small foot at the bottom"
draft t 1  "a tall cross, one vertical stroke crossed near the top"
draft k 2  "an angular open hook facing right"
draft s 3  "a single smooth S-like wave, drawn as a filled ribbon"
draft m 4  "three short vertical teeth joined along the bottom"
draft n 5  "two vertical teeth joined along the bottom"
draft l 6  "an upright stroke with a foot turning right at the base"
draft r 7  "a vertical stroke with a small loop at the top right"
draft a 8  "a bold filled triangle pointing up"
draft i 9  "a single narrow vertical bar"
draft u 10 "a wide filled bowl, open at the top"

"$INK" language font-config "$LANG_NAME" | sed 's/^/    /'

# Compile the script to a TrueType font, fully in-process.
"$INK" language font-build --language "$LANG_NAME" --format ttf --out "$ART/$LANG_NAME" \
    | sed 's/^/    /'
# Type a word in the script (romanized → glyph codepoints). The glyphs are PUA
# and invisible in a terminal, so show the codepoints (on stderr).
echo "    transliterate 'kira':"
"$INK" language transliterate "$LANG_NAME" --text kira --json 2>/dev/null \
  | grep -E '"codepoints"' -A6 | grep -oE 'U\+[0-9A-F]+' | paste -sd' ' - | sed 's/^/      /'

# ── 6. The books: dictionary, grammar, tutorial ──────────────────────────────
echo "▸ Books"
"$INK" language dictionary   "$LANG_NAME" --format typ --font "$LANG_NAME" --out "$ART/$slug-dictionary.typ" 2>/dev/null | sed 's/^/    /'
"$INK" language grammar-book "$LANG_NAME" --format typ --font "$LANG_NAME" --out "$ART/$slug-grammar.typ"    2>/dev/null | sed 's/^/    /'
"$INK" language tutorial     "$LANG_NAME" --format typ --font "$LANG_NAME" --out "$ART/$slug-tutorial.typ"   2>/dev/null | sed 's/^/    /'
# Markdown editions too.
"$INK" language dictionary   "$LANG_NAME" --format md > "$ART/$slug-dictionary.md"
"$INK" language grammar-book "$LANG_NAME" --format md > "$ART/$slug-grammar.md"
"$INK" language tutorial     "$LANG_NAME" --format md > "$ART/$slug-tutorial.md"

# ── 7. Compile the Typst books to PDF, if Typst is available ─────────────────
if command -v typst >/dev/null 2>&1; then
  echo "▸ Compiling PDFs (Typst $(typst --version | awk '{print $2}'))"
  for b in dictionary grammar tutorial; do
    typst compile --font-path "$ART" "$ART/$slug-$b.typ" "$ART/$slug-$b.pdf" \
      && echo "    $ART/$slug-$b.pdf"
  done
else
  echo "▸ (install Typst to compile: typst compile --font-path '$ART' '$ART/$slug-dictionary.typ' …)"
fi

echo "✓ Done. Artifacts in $ART:"
ls -1 "$ART" | sed 's/^/    /'
