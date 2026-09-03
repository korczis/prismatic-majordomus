/* Renders Mermaid diagrams that arrived either as a <pre class="mermaid"> written by a
   template, or as a ```mermaid fence in canonical Markdown — which Zola highlights into
   <pre><code class="language-mermaid">. Converting the second form here is what lets a
   canonical Markdown file keep rendering on GitHub and still draw a diagram on the site. */
(function () {
  'use strict';
  if (typeof window.mermaid === 'undefined') return;

  Array.prototype.forEach.call(
    document.querySelectorAll('pre code[data-lang="mermaid"]'),
    function (code) {
      var pre = code.closest('pre');
      if (!pre) return;
      var host = document.createElement('pre');
      host.className = 'mermaid';
      host.setAttribute('data-mermaid', '');
      host.textContent = code.textContent;
      pre.parentNode.replaceChild(host, pre);
    }
  );

  var nodes = document.querySelectorAll('pre.mermaid[data-mermaid]');
  if (!nodes.length) return;

  function theme() {
    return document.documentElement.classList.contains('dark') ? 'dark' : 'neutral';
  }
  var sources = [];
  Array.prototype.forEach.call(nodes, function (n) { sources.push(n.textContent); });

  function render() {
    Array.prototype.forEach.call(nodes, function (n, i) {
      n.removeAttribute('data-processed');
      n.textContent = sources[i];
    });
    window.mermaid.initialize({
      startOnLoad: false,
      /* Diagram source is repository-controlled, but "strict" still refuses raw HTML and
         click handlers inside diagrams. There is no reason to allow either. */
      securityLevel: 'strict',
      theme: theme(),
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
      flowchart: { useMaxWidth: true, htmlLabels: false }
    });
    window.mermaid.run({ nodes: nodes }).catch(function () { /* leave the source visible */ });
  }

  render();
  document.addEventListener('majordomus:theme', render);
})();
