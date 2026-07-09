#import "../design.typ": *

#chapter(number: 5, title: "Your own values, in one place")

Your name. The book's subtitle. A copyright line. A tagline. These small pieces of
text want to appear on the site, often in more than one spot, and you want to write
each of them _once_. That is what variables are for. There are two kinds in Inkhaven —
one for the site's frame, one for the book's own text — and this chapter covers both.

#section("A file of your values")

You keep your site-wide values in a small file in your project, written in a format
built for exactly this.

#term("HJSON")[
  A gentle version of a common data format called JSON, made comfortable to write by
  hand. You list `name: value` pairs; you may leave out most quotation marks; you may
  add `#` comments; and trailing commas are forgiven. It is the same format
  `inkhaven.hjson` uses. Think of it as a tidy list of labelled values.
]

Create a file called `html.hjson` in your project folder and put your values in it:

#config("html.hjson", [```hjson
{
  title:    "The Drowned Atlas"
  subtitle: "A field guide to a sunken world"
  author:   "Mara Vendt"
  footer:   "© 2026 Mara Vendt"
}
```])

Each line names a value. Nothing here is required and the names are your choice — these
four are simply common ones.

#section("How the values reach the page")

Here is the part worth understanding slowly. Inkhaven reads `html.hjson`, and hands
everything in it to your templates under the single name `site`. So a value written
`author: "Mara Vendt"` in the file becomes `site.author` in a template.

#term("Translating HJSON to Jinja")[
  The bridge between the two formats. A pair you write in `html.hjson` as
  `author: "Mara Vendt"` arrives in your templates as the Jinja value `site.author`.
  The file supplies the values; the templates spend them. `site` is simply the basket
  everything in the file lands in.
]

So to show your name in the sidebar header, open `theme/header.html` and refer to it:

#config("theme/header.html", [```html
<div class="brand">
  <span class="brand-title">{{ site.title }}</span>
  <span class="brand-sub">{{ site.subtitle }}</span>
</div>
```])

When Inkhaven builds the site, `{{ site.title }}` becomes `The Drowned Atlas` and
`{{ site.subtitle }}` becomes `A field guide to a sunken world` — the values you wrote
once in `html.hjson`. Change the subtitle in the file, export again, and it changes
everywhere the template used it.

#note[
  The bundled templates already reach for `site.title`, `site.subtitle`, `site.author`,
  and `site.footer` in sensible places, so simply _creating_ `html.hjson` with those
  names fills them in — even before you touch a template. Add your own names
  (`site.tagline`, `site.edition`, anything) and use them wherever you like.
]

#tryit[
  Make an `html.hjson` with a `title` and a `subtitle`, then export with a plain
  `inkhaven export html -o site` — no template editing at all. Open the site and see
  your title and subtitle in the sidebar. You have just fed values from a data file
  into a web page.
]

#section("The other kind: values inside your book")

The variables above live in the site's _frame_. There is a second kind that lives in
the _text of the book itself_ — for a value you repeat in your prose and may need to
change everywhere at once, like a product's name or a version number.

You define these under `docs: { variables: … }` in `inkhaven.hjson`:

#config("inkhaven.hjson", [```hjson
docs: {
  variables: {
    product: "Tidewalker"
    version: "3.0"
  }
}
```])

Then, _in your manuscript_, you write the name wrapped in double braces:

#config("a paragraph in your book", [```
The {{product}} handbook, version {{version}}, begins here.
```])

When you export — to the web, but also to PDF or any other format — Inkhaven replaces
`{{product}}` with `Tidewalker` and `{{version}}` with `3.0`. Rename the product in the
one settings line and every mention in the book updates at once.

#insight[
  The two kinds are worth keeping straight. `site` values, from `html.hjson`, are for
  the _website's furniture_ — the title bar, the footer — and reach only your
  templates. `docs.variables`, from `inkhaven.hjson`, are for the _book's own words_ and
  reach every export in every format. One dresses the site; the other edits the prose.
]

#recap((
  [*HJSON* is a comfortable `name: value` file format; put site-wide values in `html.hjson`.],
  [Inkhaven hands that file to templates as `site` — `author: "…"` becomes `{{ site.author }}`.],
  [Just creating `html.hjson` with `title`/`subtitle`/`author`/`footer` fills the default templates in.],
  [`docs.variables` in `inkhaven.hjson` is a separate kind: `{{name}}` written *in your prose* is replaced in every export.],
))
