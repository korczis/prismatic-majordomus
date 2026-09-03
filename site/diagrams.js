// Mermaid initialisation. Source blocks are <pre class="mermaid">, projected from canonical
// ```mermaid fences or from site/data/generated/diagrams.json. Rendering is a progressive
// enhancement: a failed diagram leaves its source visible and never breaks the page.
(function () {
  if (!window.mermaid) { return; }
  var blocks = document.querySelectorAll('pre.mermaid');
  if (!blocks.length) { return; }
  blocks.forEach(function (b) { b.setAttribute('data-source', b.textContent); });
  function render() {
    var dark = document.documentElement.classList.contains('dark');
    blocks.forEach(function (b) { b.removeAttribute('data-processed'); b.textContent = b.getAttribute('data-source'); });
    window.mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      theme: dark ? 'dark' : 'neutral',
      fontFamily: 'ui-sans-serif, system-ui, sans-serif',
      themeVariables: dark
        ? { primaryColor: '#1f2937', primaryTextColor: '#e5e7eb', primaryBorderColor: '#4b5563', lineColor: '#9ca3af', secondaryColor: '#111827', tertiaryColor: '#111827', background: '#111827', fontSize: '14px' }
        : { primaryColor: '#f3f4f6', primaryTextColor: '#111827', primaryBorderColor: '#9ca3af', lineColor: '#4b5563', secondaryColor: '#f9fafb', tertiaryColor: '#f9fafb', background: '#ffffff', fontSize: '14px' }
    });
    window.mermaid.run({ nodes: blocks }).catch(function () { /* leave the source visible */ });
  }
  render();
  var toggle = document.getElementById('theme-toggle');
  if (toggle) { toggle.addEventListener('click', function () { setTimeout(render, 0); }); }
})();
