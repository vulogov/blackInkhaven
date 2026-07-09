#import "../design.typ": *

#chapter(number: 4, title: "The page, and how it is built")

Colours and fonts change how the site looks. _Templates_ change how each page is put
together — what goes at the top, what sits in the sidebar, what appears at the foot of
every chapter. This is a step deeper than styling, and you may never need it. But when
you want a logo in the corner or a line in every footer, this is where it lives.

#section("What a template is")

Every page on your site has the same frame: the same header, the same sidebar, the
same footer, wrapped around _different_ chapter text. Inkhaven does not store that
frame separately for each page. It stores it once, with a blank where the chapter goes,
and fills the blank in for each chapter as it builds the site. That reusable frame is a
template.

#term("Template")[
  A page with blanks in it. Inkhaven keeps one template for the whole site and, for
  each chapter, fills the blanks with that chapter's title, text, and navigation. Write
  the frame once; get a consistent page every time.
]

The main template is `functional/page.html` in your ejected design folder. It is
mostly ordinary HTML — the same kind of text your finished pages are made of — with
blanks marked in a small language called Jinja.

#term("Jinja")[
  A simple language for writing the blanks in a template. Two marks are almost all of
  it: `{{ something }}` means "put the value of _something_ here," and
  `{% … %}` wraps an instruction like "repeat this for every chapter." Everything
  outside those marks is plain HTML that passes through unchanged.
]

#section("What you can put in the blanks")

When Inkhaven fills a template, it hands it a set of named values to draw from. You
refer to them inside `{{ }}`. The ones you will use most:

#gloss("`book.title`")[Your book's title.]
#gloss("`page.title`")[The title of the chapter being built.]
#gloss("`page.content`")[The chapter's text, already turned into HTML. This is the big blank.]
#gloss("`nav`")[The list of chapters, for building the sidebar.]
#gloss("`site`")[Your own values from a settings file — title, author, anything you like. This is Chapter 5.]
#gloss("`labels`")[The small interface words ("Contents", "Search"), already translated to your book's language.]

So a line in the template like this —

#config("functional/page.html", [```html
<title>{{ page.title }} · {{ book.title }}</title>
```])

— becomes, on the chapter page for "Beginnings" of a book called "The Drowned Atlas":

#config("the built page", [```html
<title>Beginnings · The Drowned Atlas</title>
```])

Inkhaven simply replaced each blank with its value.

#section("A repeat, for the sidebar")

The sidebar lists _every_ chapter, but you do not write each one — you write the
pattern once and Jinja repeats it. In `page.html` you will find something close to
this:

#config("functional/page.html", [```html
<ul>
  {% for item in nav %}
  <li><a href="{{ item.href }}">{{ item.title }}</a></li>
  {% endfor %}
</ul>
```])

#term("A `for` loop")[
  The instruction `{% for item in nav %} … {% endfor %}` means "for each chapter in the
  list, produce this once." Inside, `item.title` and `item.href` are that chapter's
  title and its page's address. One pattern, however many chapters you have.
]

You rarely need to touch this — it already builds a correct sidebar. It is shown so
that the file is not a mystery when you open it.

#section("An edit you might actually make")

The footer at the bottom of every page is the friendliest thing to change, and it
lives in `theme/footer.html` — a tiny template of its own. To add a line of your own
to every page's foot, open it and add plain HTML:

#config("theme/footer.html", [```html
<footer class="site-footer">
  <p class="footer-built">{{ labels.built_with }} Inkhaven</p>
  <p>First edition · Printed nowhere · Read everywhere.</p>
</footer>
```])

Export with your design, and that line now sits at the foot of every chapter. You
wrote it once; the template put it on every page.

#note[
  `theme/header.html` is the matching piece at the top of the sidebar — the book's
  title and subtitle. Edit it the same way. Because both are in `theme`, they count as
  _look_, not machinery: safe to change freely.
]

#pitfall[
  If you delete one of the `{{ … }}` blanks the site depends on — the big
  `{{ page.content }}` one especially — your pages will come out missing their text.
  When you edit a template, _add_ around the blanks; do not remove the ones that were
  there. If a page comes out wrong, eject a fresh copy and compare.
]

#recap((
  [A *template* is the page frame, written once with blanks that Inkhaven fills for each chapter.],
  [*Jinja* marks the blanks: `{{ value }}` inserts a value, `{% for … %}` repeats a pattern.],
  [Useful values include `book.title`, `page.title`, `page.content`, `nav`, `site`, and `labels`.],
  [The footer (`theme/footer.html`) and header (`theme/header.html`) are the easiest templates to make your own.],
))
