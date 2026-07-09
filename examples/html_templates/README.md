# Inkhaven HTML templates

These are the **default** templates `inkhaven export html` uses. You do **not** need
to copy them to publish a site — a bare `inkhaven export html -o site/` renders a
complete, styled book out of the box using exactly these files (they are embedded in
the binary).

Copy them when you want to **customise**. If you installed Inkhaven with `cargo
install` (no repo checkout), get the exact defaults with:

```
inkhaven export html --eject-templates my-templates/
```

Then point the exporter at your copy — either per-run or via config:

```
inkhaven export html -o site/ --templates my-templates/   # CLI flag (wins)
```
```hjson
docs: { html: { template_dir: "my-templates" } }          # or in inkhaven.hjson
```

A file present in your template directory overrides that one bundled default; every
other file keeps its default. So you can restyle without touching the machinery, and
you only keep the files you actually changed.

## Self-contained by design

The exported site has **zero external dependencies** — no CDN, no web fonts, no
external scripts or styles. `theme.css` and every image are written into the output
directory, and the type uses system font stacks. The whole folder opens from disk or
drops onto any static host and renders identically offline. Keep it that way: if you
add a font or script to a template, embed or bundle it rather than linking a CDN.

## The split: `functional/` vs `theme/`

- **`functional/`** — the *machinery*. Page skeleton, the navigation tree, the table
  of contents, search wiring. Change these to alter *structure or behaviour*.
  - `page.html` — the per-page skeleton: `<head>`, the sidebar/nav, the content slot,
    the prev/next pager.
- **`theme/`** — the *look*. Change these to restyle without touching the plumbing.
  - `theme.css` — all styling (light + dark, layout, type, callout colours). Based on
    the *Building the World* companion book's warm cream/sienna palette.
  - `header.html` — the sidebar brand block.
  - `footer.html` — the page footer.

## The template context

Every template is [minijinja](https://docs.rs/minijinja) and receives:

| Variable | What |
|----------|------|
| `book` | `{ title, language }` — the exported user book. |
| `page` | `{ title, content, root, prev, next, is_index }`. `content` is pre-rendered HTML (use `\| safe`). `root` is the relative path to the site root. |
| `nav` | the table of contents: a list of `{ title, href, current, sections: [{title, href}] }`. |
| `site` | your `html.hjson` (the `docs.html.variables_file`) parsed — put a `title`, `author`, `subtitle`, `footer`, or anything else here. |
| `vars` | `docs.variables` (the TDOC-3 single-sourcing map). |
| `labels` | UI chrome localised to the book's language (`contents`, `search`, `skip`, `built_with`, …). |
| `lang` | the two-letter `lang` attribute for `<html>`. |

## Site variables — `html.hjson`

Put site-wide values in your project's `html.hjson` (HJSON: comments, unquoted keys):

```hjson
{
  title:    "The Tide That Remembers"
  subtitle: "A field guide to a drowned world"
  author:   "Your Name"
  footer:   "© 2026"
}
```

They arrive in templates as `{{ site.title }}`, `{{ site.author }}`, etc.
