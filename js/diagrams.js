// Mermaid initialisation. Source blocks are <pre class="mermaid">, projected from canonical
// ```mermaid fences or from site/data/generated/diagrams.json. Rendering is a progressive
// enhancement: a failed diagram leaves its source visible and never breaks the page.
(function () {
  if (!window.mermaid) { return; }
  var blocks = document.querySelectorAll('pre.mermaid');
  if (!blocks.length) { return; }
  blocks.forEach(function (b) { b.setAttribute('data-source', b.textContent); });
  // One read of the page's own custom properties. `getPropertyValue` answers the empty
  // string for a name no stylesheet declares, which is how the previous palette here went
  // unnoticed: every lookup missed and every fallback was used.
  function tok(name) {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  // The named tokens that actually resolve, plus whatever is passed through verbatim.
  function vars(names, extra) {
    var out = {}, key;
    for (key in extra) { if (Object.prototype.hasOwnProperty.call(extra, key)) out[key] = extra[key]; }
    for (key in names) {
      if (!Object.prototype.hasOwnProperty.call(names, key)) continue;
      var value = tok(names[key]);
      if (value) out[key] = value;
    }
    return out;
  }

  function render() {
    var dark = document.documentElement.classList.contains(document.documentElement.dataset.themeClass);
    blocks.forEach(function (b) { b.removeAttribute('data-processed'); b.textContent = b.getAttribute('data-source'); });
    window.mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      theme: dark ? 'dark' : 'neutral',
      fontFamily: tok('--font-sans'),
      // Read from the page, not written here. These names are declared once, in
      // share/design/tokens.yaml, and reach this script through the stylesheet the page
      // already loaded; the block that used to sit here was a private colour table that
      // moved whenever nobody remembered it existed. A token that does not resolve is
      // omitted rather than guessed, and Mermaid's own theme answers for it.
      themeVariables: vars({
        primaryColor: '--mj-raised',
        primaryTextColor: '--mj-fg',
        primaryBorderColor: '--mj-line',
        lineColor: '--mj-muted',
        secondaryColor: '--mj-sunken',
        tertiaryColor: '--mj-sunken',
        background: '--mj-bg'
      }, { fontSize: '14px' })
    });
    window.mermaid.run({ nodes: blocks }).catch(function () { /* leave the source visible */ });
  }
  render();
  var toggle = document.getElementById('theme-toggle');
  if (toggle) { toggle.addEventListener('click', function () { setTimeout(render, 0); }); }
})();
