//! TDOC-4.2 — client-side site search. The exporter writes a `search-index.js`
//! (the page corpus as a JS assignment — loaded via `<script src>`, which works from
//! `file://`, unlike `fetch`) plus a small vanilla-JS `search.js`. Both are
//! self-contained; no network, no library.

/// One searchable page.
pub struct SearchDoc {
    pub title: String,
    pub url: String,
    pub text: String,
}

/// Strip HTML tags + decode the handful of entities we emit, collapsing runs of
/// whitespace, and cap the length so the index stays bounded.
pub fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    // `i` is always advanced by whole characters, so `html[i..]` stays on a char
    // boundary (content contains multibyte chars like `—` and `…`).
    let mut i = 0;
    while i < html.len() {
        let rest = &html[i..];
        // Skip <script>…</script> wholesale.
        if !in_tag && rest.starts_with("<script") {
            match rest.find("</script>") {
                Some(end) => {
                    i += end + "</script>".len();
                    continue;
                }
                None => break,
            }
        }
        let c = rest.chars().next().unwrap();
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
            out.push(' ');
        } else if !in_tag {
            out.push(c);
        }
        i += c.len_utf8();
    }
    let decoded = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");
    let collapsed: String = decoded.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.chars().take(4000).collect()
}

/// Build `search-index.js` — a JS assignment of the corpus.
pub fn build_index_js(docs: &[SearchDoc]) -> String {
    let mut s = String::from("window.__INKHAVEN_SEARCH__ = [\n");
    for d in docs {
        s.push_str(&format!(
            "{{\"t\":{},\"u\":{},\"b\":{}}},\n",
            js_str(&d.title),
            js_str(&d.url),
            js_str(&d.text)
        ));
    }
    s.push_str("];\n");
    s
}

fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// The search UI. Attaches to `.site-search`; renders hits into
/// `.site-search-results`. Substring match over title + body, title hits first.
pub const SEARCH_JS: &str = r#"(function(){
  var idx = window.__INKHAVEN_SEARCH__ || [];
  var input = document.querySelector('.site-search');
  var out = document.querySelector('.site-search-results');
  if(!input || !out) return;
  function render(q){
    out.innerHTML = '';
    q = q.trim().toLowerCase();
    if(q.length < 2){ out.removeAttribute('data-open'); return; }
    var hits = [];
    for(var i=0;i<idx.length;i++){
      var e = idx[i];
      var t = (e.t||'').toLowerCase(), b = (e.b||'').toLowerCase();
      var ti = t.indexOf(q), bi = b.indexOf(q);
      if(ti>=0 || bi>=0){ hits.push({e:e, score: ti>=0?0:1, pos: bi}); }
    }
    hits.sort(function(a,b){ return a.score - b.score; });
    if(!hits.length){ out.setAttribute('data-open',''); var li=document.createElement('li'); li.className='no-hit'; li.textContent='No matches'; out.appendChild(li); return; }
    hits.slice(0,20).forEach(function(h){
      var li=document.createElement('li');
      var a=document.createElement('a'); a.href=h.e.u; a.textContent=h.e.t; li.appendChild(a);
      if(h.score===1){
        var start=Math.max(0, h.pos-30);
        var span=document.createElement('span'); span.className='search-snip';
        span.textContent='… '+ (h.e.b||'').substr(start,90) +' …';
        li.appendChild(span);
      }
      out.appendChild(li);
    });
    out.setAttribute('data-open','');
  }
  input.addEventListener('input', function(){ render(input.value); });
  document.addEventListener('click', function(ev){
    if(ev.target!==input && !out.contains(ev.target)){ out.removeAttribute('data-open'); }
  });
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_removes_tags_scripts_and_entities() {
        let text = strip_html("<h1>Hi &amp; bye</h1><p>a<strong>b</strong></p><script>var x=1<2;</script>end");
        assert!(text.contains("Hi & bye"));
        assert!(text.contains("a b") || text.contains("ab"));
        assert!(!text.contains("var x"), "script stripped: {text}");
        assert!(text.contains("end"));
    }

    #[test]
    fn strip_handles_multibyte_without_panicking() {
        // Regression: byte-stepping used to slice inside `—`/`…` and panic.
        let text = strip_html("<h2>Avesha — Dictionary</h2><p>Filter the words…</p>");
        assert!(text.contains("Avesha — Dictionary"), "got: {text}");
        assert!(text.contains("words…"));
    }

    #[test]
    fn index_js_is_valid_assignment() {
        let js = build_index_js(&[SearchDoc {
            title: "Chapter \"One\"".into(),
            url: "ch01.html".into(),
            text: "the sea".into(),
        }]);
        assert!(js.starts_with("window.__INKHAVEN_SEARCH__ = ["));
        assert!(js.contains("\"u\":\"ch01.html\""));
        assert!(js.contains("Chapter \\\"One\\\"")); // quotes escaped for JS
    }
}
