// proof_corpus — a fake 6×9 trade book used to exercise the PDF-1
// imposition pipeline end to end (compile → impose).  Not a real
// manuscript; every leaf is one numbered page of lorem so the imposed
// signatures are easy to eyeball.

#set document(title: "The Lantern Room (imposition proof)", author: "V. Ulogov")
#set page(
  width: 6in,
  height: 9in,
  margin: (inside: 0.85in, outside: 0.65in, top: 0.8in, bottom: 0.8in),
  numbering: "1",
)
#set text(size: 11pt, lang: "en")
#set par(justify: true, leading: 0.72em)

#let total = 48
#for i in range(0, total) {
  heading(level: 1, numbering: none)[Leaf #(i + 1)]
  parbreak()
  lorem(58)
  if i + 1 < total { pagebreak() }
}
