// The page an old anchor moved to: the id itself, or the nearest id it is namespaced under
// (schema-Plan-example belongs to schema-Plan). null when nothing in the map claims it.
window.mjApiAnchorTarget = function (map, id) {
  var key = id;
  while (key) {
    if (Object.prototype.hasOwnProperty.call(map, key)) return map[key];
    var cut = key.lastIndexOf('-');
    if (cut <= 0) return null;
    key = key.slice(0, cut);
  }
  return null;
};

// An address into the HTTP API reference from when it was one page — /docs/api/#op-…,
// #tag-…, #schema-…, or a heading id namespaced under a schema — lands on the index, which
// no longer carries those ids. The index publishes where each one moved, as data, and this
// sends the reader on. Without JavaScript nothing happens, and the index's own list of every
// tag and operation is the way on.
(function () {
  'use strict';
  var node = document.getElementById('api-anchors');
  if (!node) return;
  var hash = window.location.hash.slice(1);
  if (!hash || document.getElementById(hash)) return;
  var map;
  try { map = JSON.parse(node.textContent); } catch (e) { return; }
  var target = window.mjApiAnchorTarget(map, hash);
  if (target) window.location.replace(node.getAttribute('data-base') + target + '#' + hash);
})();
