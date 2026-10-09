(function () {
  var q = document.getElementById("q"), box = document.getElementById("results"), data = window.BURNDOC || [], sel = 0, hits = [];
  function esc(s) { return s.replace(/&/g, "&amp;").replace(/</g, "&lt;"); }
  function score(e, t) {
    var n = e[0].toLowerCase(), last = n.slice(n.lastIndexOf(".") + 1);
    if (n === t || last === t) return 0;
    if (last.indexOf(t) === 0 || n.indexOf(t) === 0) return 1;
    if (n.indexOf(t) >= 0) return 2;
    if (e[3].toLowerCase().indexOf(t) >= 0) return 3;
    return -1;
  }
  function draw() {
    if (!hits.length) { box.innerHTML = "<a>No results</a>"; box.hidden = false; return; }
    box.innerHTML = hits.map(function (e, i) {
      return "<a href=\"" + e[2] + "\"" + (i === sel ? " class=\"sel\"" : "") + "><span class=\"rk\">" + e[1] + "</span><code>" + esc(e[0]) + "</code><span class=\"rs\">" + esc(e[3]) + "</span></a>";
    }).join("");
    box.hidden = false;
  }
  q.addEventListener("input", function () {
    var t = q.value.trim().toLowerCase();
    if (!t) { box.hidden = true; return; }
    hits = data.map(function (e) { return [score(e, t), e]; }).filter(function (x) { return x[0] >= 0; })
      .sort(function (a, b) { return a[0] - b[0] || a[1][0].length - b[1][0].length; }).slice(0, 12).map(function (x) { return x[1]; });
    sel = 0;
    draw();
  });
  q.addEventListener("keydown", function (ev) {
    if (box.hidden) return;
    if (ev.key === "ArrowDown") { sel = Math.min(sel + 1, hits.length - 1); draw(); ev.preventDefault(); }
    else if (ev.key === "ArrowUp") { sel = Math.max(sel - 1, 0); draw(); ev.preventDefault(); }
    else if (ev.key === "Enter" && hits[sel]) { location.href = hits[sel][2]; }
    else if (ev.key === "Escape") { box.hidden = true; q.blur(); }
  });
  document.addEventListener("keydown", function (ev) {
    if (ev.key === "/" && document.activeElement !== q) { q.focus(); ev.preventDefault(); }
  });
  document.addEventListener("click", function (ev) { if (!box.contains(ev.target) && ev.target !== q) box.hidden = true; });
})();
