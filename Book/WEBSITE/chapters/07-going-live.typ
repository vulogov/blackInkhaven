#import "../design.typ": *

#chapter(number: 7, title: "Putting it on the internet")

You have a folder that is a complete website. The last step is to move that folder to
a computer that is always on and connected, so anyone with the address can visit. That
computer is called a host, and because your site is self-contained, hosting it is about
as simple as web hosting ever gets.

#section("What hosting means")

#term("Web host")[
  A computer, run by someone else and always online, that holds your website's files
  and hands them to anyone who asks. You copy your folder to it once; from then on the
  host serves your pages to visitors. Many hosts cost nothing for a small site like
  a book.
]

#term("Static hosting")[
  Hosting for sites that are just files — HTML, CSS, images — with no program running
  behind them. That is precisely what Inkhaven builds, so any static host will do, and
  static hosts are the cheapest, fastest, and most durable kind. When a host mentions
  "static sites," that is you.
]

Because the site asks nothing of the outside world, there is no database to set up, no
software to install on the host, nothing to keep patched. You are moving a folder of
plain files. That is the whole job.

#section("The shape of it")

However you host, the steps rhyme:

+ *Build the site.* `inkhaven export html -o site` — the `site` folder is what you
  publish.
+ *Give the folder to the host.* Some hosts have you drag the folder onto a web page;
  some watch a folder on a file-sharing service; some pull it from a code repository.
  All of them, in the end, take your folder.
+ *The host gives you an address.* A web address where your book now lives. Share it.

#term("The front page")[
  When a visitor arrives at your address with nothing further, the host looks for a
  file called `index.html` and shows that. Inkhaven always writes one — it is your
  book's front page — so visitors land in the right place automatically. This is why
  the file is named as it is; do not rename it.
]

#section("Checking before you ship")

You never have to guess how the site will look online, because it looks the same
offline. Open `site/index.html` on your own machine and read it end to end. Click every
chapter in the sidebar. Follow the arrows. What you see is exactly what a visitor will
see — that is the gift of a self-contained site.

#tryit[
  Before publishing, move the whole `site` folder somewhere else on your computer — the
  desktop, a memory stick — and open `index.html` from there. It should work
  identically, with every picture and every link intact. If it does (and it will), you
  have proven the site carries everything it needs, and it will behave the same on any
  host.
]

#pitfall[
  Publish the _contents_ of the `site` folder as the root of your web space, so that
  `index.html` sits at the top. A common slip is to publish the folder _inside_ another
  folder, so visitors must find `.../site/index.html` instead of just your address.
  If your front page does not appear on its own, check whether `index.html` ended up
  one level too deep.
]

#section("Keeping it current")

A book is rarely finished all at once. When you revise — fix a line, add a chapter,
change the cover subtitle — you simply export again and replace the folder on the host.
There is no separate website to edit and keep in step with the manuscript: the
manuscript _is_ the website's source, every time. Change the book, re-export, re-upload.
The site is never out of date for longer than it takes to run one command.

#insight[
  This is the quiet promise of the whole feature. Your book and your website are not
  two things you must maintain in parallel — they are one thing, seen two ways. You
  keep writing the book; the website is only ever a fresh printing of it, one command
  away.
]

#recap((
  [A *web host* is an always-online computer that serves your files; a *static host* is the right, cheap, durable kind for an Inkhaven site.],
  [Publishing is moving the `site` folder to the host so `index.html` sits at the top; the host gives you an address.],
  [What you see opening `index.html` on your own machine is exactly what visitors see — check it there first.],
  [To update, export again and replace the folder; the manuscript is always the site's single source.],
))
