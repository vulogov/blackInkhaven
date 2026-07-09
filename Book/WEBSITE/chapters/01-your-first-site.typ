#import "../design.typ": *

#chapter(number: 1, title: "Your first website")

Let us make a website right now, before we explain anything else. You will see how
little it takes, and the rest of the book will make sense against that first result.

#section("The terminal")

Inkhaven's export runs from the _terminal_.

#term("Terminal")[
  A window where you type commands to your computer as text, instead of clicking
  buttons. On macOS it is the app called _Terminal_; on Windows, _PowerShell_ or
  _Windows Terminal_; on Linux, your _console_. You type a line, press Return, and the
  computer does it.
]

If you have used Inkhaven from the terminal before — to start it, or to export a PDF —
this is the same window. Open it, and move into the folder that holds your project
(the folder with your book in it). Then type one line:

#run[```
inkhaven export html --output site
```]

Press Return. In a moment Inkhaven prints something like `wrote HTML site to site`,
and you are done. You have a website.

#term("`inkhaven`")[
  The program itself — the same one that runs the editor. Typed at the start of a
  terminal command, it means "Inkhaven, do the following." Everything after it tells
  Inkhaven _what_ to do; here, `export html`.
]

#section("What just happened")

The word `--output site` told Inkhaven where to put the website: in a new folder
called `site`, next to your project. Look inside it and you will find something like
this:

#run[```
site/
  index.html          the front page
  ch01-beginnings.html  one page per chapter
  ch02-the-journey.html
  theme.css           the design (colours, type, layout)
  assets/             copies of your pictures
```]

Every one of those is an ordinary file. The `.html` files are your pages; `theme.css`
holds the look; `assets` holds your images. That whole folder _is_ the website.

#term("`--output` (or `-o`)")[
  The part of the command that names the destination folder. `--output site` and the
  short form `-o site` mean the same thing. If the folder does not exist, Inkhaven
  creates it; if it does, Inkhaven writes the fresh site into it.
]

#section("Looking at it")

To see your site, open the file `index.html` in a web browser — double-click it, or
drag it onto your browser window. It opens like any web page: your book's title, a
list of chapters down the side, and your first chapter ready to read. Click a chapter
in the sidebar to jump to it; use the arrows at the foot of each page to move to the
next.

You are looking at a real website, sitting on your own computer. Nobody else can see
it yet — that is the final chapter — but everything is here.

#tryit[
  Export your book with the command above, open `site/index.html`, and click through
  a chapter or two. Notice that the pictures are there, the chapters are in order, and
  the whole thing reads cleanly. That is the default design, and it is a perfectly
  good place to stop — you can publish exactly this. The rest of the book is about
  the times you want more.
]

#pitfall[
  If the terminal answers with `command not found: inkhaven`, the program is not on
  your system's list of commands yet. That is an installation matter, not an export
  one — the same thing that lets you start the editor by typing `inkhaven`. Once the
  editor starts from the terminal, export will too.
]

#recap((
  [`inkhaven export html --output site` turns your book into a website in the `site` folder.],
  [The folder holds one `.html` page per chapter, a front page, the `theme.css` design, and an `assets` folder of pictures.],
  [Open `index.html` in any browser to read the site on your own computer.],
  [The default design is publishable as-is; everything after this is optional refinement.],
))
