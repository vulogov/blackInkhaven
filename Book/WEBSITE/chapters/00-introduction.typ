#import "../design.typ": *

#chapter(number: 0, title: "Before we begin")

You have written a book in Inkhaven. Now you want other people to read it — not as a
file they must download and open in a particular program, but as something they can
simply _visit_, the way they visit any page on the internet. That is what this short
book teaches: how to turn your manuscript into a website, how to make that website
look the way you want, and how to put it somewhere the world can reach it.

You do not need to be a web designer, and you do not need to be an Inkhaven expert.
Everything here is explained from the ground up. Where a word has a precise meaning —
and the web is full of such words — it is defined the first time it appears, in a box
like this one:

#term("Website")[
  A collection of pages, written in a language called HTML, that a web browser (Safari,
  Chrome, Firefox) can display. A website can live on the internet for anyone to
  visit, or sit in a folder on your own computer. Inkhaven builds the second kind, and
  you decide whether to put it online.
]

#term("HTML")[
  _HyperText Markup Language_ — the format web pages are written in. You will never
  have to write HTML by hand: Inkhaven writes it for you, from the same manuscript you
  already have. HTML is just text, so a website is really just a folder of text files
  and pictures.
]

#section("What you will end up with")

One command turns your book into a _folder_. Inside that folder is a small website:
one page for each chapter, a table of contents down the side, a clean reading design,
and copies of any pictures your book uses. You can open it on your own machine to
check it, and when you are happy, you copy that folder to a web host and it is live.

Crucially, the site Inkhaven produces is _self-contained_.

#term("Self-contained")[
  Everything the website needs to display is inside its own folder — the design, the
  pictures, the text. It does not reach out to anyone else's computer to load a font
  or a style. That means it works with no internet connection at all, it will still
  work years from now, and nothing outside it can change how it looks. You can email
  the folder to someone and it just works.
]

#section("How this book is arranged")

We begin with the single command that makes a site, and look at what it produces.
Then we cover the terminal commands in full, so you know every option you have. The
middle of the book is about making the site _yours_: its colours and type (styling),
its layout (templates), and the little pieces of text that appear throughout
(variables). Near the end we talk about choosing exactly what goes on the site, and
finally how to put it on the internet.

You can read straight through, or jump to the chapter you need. Each one ends with a
short recap of what it covered.

#insight[
  The whole feature rests on one idea: you already wrote the book. Publishing it to
  the web should not mean writing it again in a new form. So Inkhaven reads the very
  same manuscript you have been editing and renders it as a website — and every time
  you change the book and export again, the website simply catches up. There is no
  second copy to keep in sync.
]

#recap((
  [A *website* is a folder of HTML pages a browser can display; you decide whether to put it online.],
  [Inkhaven builds the site from your existing manuscript — no second copy, no HTML by hand.],
  [The site is *self-contained*: it needs nothing from the outside to display, works offline, and travels as a single folder.],
  [You need no web or Inkhaven expertise; every term is defined as it appears.],
))
