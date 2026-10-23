<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the `delivery` module of the canonical Majordomus capability registry; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.8 -->
# Module `delivery` — Delivery

Whether each product feature exists, computed and never recorded: on the trunk, deployed, publicly verified, its required tests current, that evidence published and linked from the interface — each a verdict of pass, fail or unknown with the reason and the remediation, and unknown never a pass. A feature short of any of them is not delivered, and its development stage (implemented on a branch, on master, tested, deployed) is a different type from delivery.

Stability: experimental. Capabilities: 3.

## `delivery.feature` — Does this feature exist

One product feature against the delivery invariant: every dimension with its verdict, the sentence that decided it and what would change it, whether it exists, and — when it does not — how far development has carried it and which dimensions block it. An id the layer does not declare is not found rather than answered as not delivered.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_delivery_feature` |
| HTTP | `GET /api/v1/delivery/feature` |
| CLI | `majordomus delivery show` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::delivery |
| tags | delivery, features, verification |

| input | type | required | description |
|---|---|---|---|
| `id` | string | yes | The feature's id, as `product.features` gives it. |

Output: `FeatureDelivery`.

## `delivery.report` — Does each feature exist

Every product feature of the layer against the delivery invariant, ordered by id: the six dimensions with their verdicts, reasons and remediations; the development stage when it is not delivered; the paths it is implemented by and the revisions that last touched them at HEAD and on the trunk; and the publication — the public site's identity, read once, and whether scripts/pages verified it. Reads git and the network; the site address can be replaced with MAJORDOMUS_DELIVERY_SITE_URL.

| | |
|---|---|
| kind | query |
| stability | experimental |
| MCP tool | `majordomus_delivery` |
| HTTP | `GET /api/v1/delivery` |
| CLI | `majordomus delivery report` |
| cache | — |
| benchmark | waived (external_dependency) |
| provenance | builtin majordomus_cli::capability::builtin::delivery |
| tags | delivery, features, verification |

Input: none.

Output: `DeliveryReport`.

## `delivery.trends` — How CI runs trend

The series of CI runs the pages publication recorded in this checkout at site/data/trends.json, oldest first: for each run its wall time, the time its jobs waited for a runner, its jobs, and the coverage it measured (lines, functions and regions as covered over total with the percentage they give); and the median run and queue time of every job over the series, the slowest first. A read of one file: nothing is fetched, derived or written, and no committed file depends on it. A figure the file does not give is absent, never zero, and a run that measured no coverage has none. `recorded: false` with the reason when the file is absent, is not JSON, or is not the trends/v1 series.

| | |
|---|---|
| kind | query |
| stability | experimental |
| HTTP | `GET /api/v1/delivery/trends` |
| cache | — |
| benchmark | required |
| provenance | builtin majordomus_cli::capability::builtin::delivery |
| tags | delivery, ci, trends |

Input: none.

Output: `TrendsReport`.

