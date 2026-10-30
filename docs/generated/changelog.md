<!-- GENERATED FILE — DO NOT EDIT DIRECTLY
     Source: the layer's release records, the decisions dated in each release's window, and the commits in its range; regenerate with `majordomus generate`
     Generator: majordomus-cli 1.0.0-alpha.9 -->
# Changelog

Every entry below is derived: a section per release the layer records, its decisions the ADRs dated inside that release's window, its changes the conventional commits in its range, its artifacts the record's own evidence. Nothing here is written by hand, and `majordomus generate changelog --check` fails when it stops matching the tree.

Current version: **1.0.0-alpha.9**

## v1.0.0-alpha.9 — 2026-10-28

### Added

- **ui**: majordomus ui ensures the shared server and opens the dashboard (I2484) (`2273179f5`)
- **mesh**: a claim lives by its holder session's progress (`62e4e0ea4`)

### Fixed

- **follow**: a follower's shared read is keyed by its own server (I4241) (`e2fb35e04`)
- **train**: train 8 owes alpha.9, its ui coverage and its roadmap width (`b63ff0c87`)
- **ui**: ui takes its repository from serve's one discovery (I2484) (`a31215b95`)
- **launcher**: a MAJORDOMUS_BIN naming a launcher is refused, not looped (I3320) (`fe6bf1153`)
- **plan**: a commit that only plans an issue is not its delivery (I2749) (`71674847a`)
- **ci**: a rerun is judged by its own receipts (`36b4a0771`)
- **ui**: the ui command's summary quotes its route placeholder (I2484) (`81f2c1a70`)
- **ci**: only a trends/v1 series decides a run link (`59bcaa4f9`)
- **ui**: the ui case bounds its requests and UiArgs' example names it (I2484) (`656bb6a02`)
- **peers**: a client that re-attaches keeps one departed row, not one per reconnect (I4227) (`8c480144a`)
- **mesh**: a node's provisioning secret is read on mc's standard input (I3996) (`7a71401f8`)
- **ci**: install-check names a launcher that closed its pipe instead of dying on it (`3da5911d2`)
- **mesh**: the tailscale CLI is started through the busy retry (ETXTBSY) (`8ad00b161`)
- **ci**: the Rust cache carries the shared build directory (I4221) (`e9e7ba923`)
- **mesh**: the items with doc examples stay public, each with its own (`a380305aa`)
- **mesh**: claim liveness is a rule of its own, and every new surface is proven (`c2b1d4a94`)

### Tests

- **ui**: case 3383 lets the peers screen say it is catching up (I4244) (`6c79dcf68`)
- cases 3075 and 51 are handed this tree's executable (ADR 0224) (`4c7d86e19`)
- **plan**: a git that cannot answer is an error, not an empty answer (I2749) (`e610dfe28`)
- **deploy**: the image's build stage carries every file the crate embeds (`8402db60e`)
- **mesh**: case 3990 turns the tailnet off with multicast (I3990) (`38b6d05ec`)
- **mesh**: an overtaken release logs its outcome, and mesh.beat's refusals are tested (`2703e7929`)
- **mesh**: every beat path I3990 added is covered, the unreachable ones removed (`d0f68f0e7`)
- **mesh**: cover the progress rule's bounds, readers and surfaces (I3990) (`596e3b2ae`)

### Housekeeping

- **derive**: delivery observes the committed I4241 fix (`e2e4d3eac`)
- **release**: record v1.0.0-alpha.8 and the metadata it publishes (`f219fc9c8`)
- **derive**: train 8's derived data observes its committed merge (`91a9570d0`)
- **plan**: I3171 is done; the source-range test passes on master after train 7 (`e75d3c393`)
- **derive**: delivery observes the committed merge of train 7 (`e43a17ee7`)
- **derive**: delivery observes the committed fix (`70a15d8cc`)
- **derive**: delivery observes the committed merge of master (`c3a6920cc`)

### Published

- `macos-aarch64` — majordomus-v1.0.0-alpha.9-aarch64-apple-darwin.tar.gz (`29b669559190`)
- `macos-x86_64` — majordomus-v1.0.0-alpha.9-x86_64-apple-darwin.tar.gz (`774141b6da42`)
- `linux-x86_64-gnu` — majordomus-v1.0.0-alpha.9-x86_64-unknown-linux-gnu.tar.gz (`a73710f2e52b`)
- `linux-x86_64-musl` — majordomus-v1.0.0-alpha.9-x86_64-unknown-linux-musl.tar.gz (`492e842bee68`)
- `linux-aarch64-gnu` — majordomus-v1.0.0-alpha.9-aarch64-unknown-linux-gnu.tar.gz (`da9918ab3c1d`)
- `linux-aarch64-musl` — majordomus-v1.0.0-alpha.9-aarch64-unknown-linux-musl.tar.gz (`f78cbb2261e7`)


## v1.0.0-alpha.8 — 2026-10-25

### Added

- **build**: every local build path goes through the mesh cache, and a gate refuses a bypass (`f134eb5d0`)
- **mesh**: a node is provisioned as a cache provider, idempotently (`68990e194`)
- **mcp**: a board row names the address that reaches its worker (`ae584cbab`)
- **mesh**: a store is offered in a signed, expiring mesh record (`14cfb0ab4`)
- **mesh**: the sccache client is derived from the selected store (`bfe990b78`)
- **deploy**: a deployment's slug carries the site built at its own commit (I0912, ADR 0223) (`9c9b11578`)
- **mesh**: a dependency install is restored before and saved after (`7d4e23d3c`)
- **github**: a pull request carries its metadata, projected from the records (I3970) (`9f231f212`)
- **mesh**: a cache key is a derived triplet and the store is spoken natively (`2656a9905`)
- **integration**: the executor fast-forwards master to every green head (I3910) (`92946d3e0`)
- **devcontext**: the compiled context names Rust source ranges with their reason and cost (I3171) (`c453f536a`)
- **providers**: a provider declares how it reports usage, and the telemetry plan unblocks (I3287) (`8b28ca31f`)
- **mesh**: a hosted hub is trusted and closed to strangers (I3826) (`38afe12a3`)
- **mesh**: the mesh client speaks TLS and refuses cleartext to a public address (I3823) (`dd334ac00`)
- **deploy**: the provider's tree is assembled from the object, and master publishes it (I3821) (`85427255b`)
- **mesh**: the tailnet is a discovery provider, so a machine joining it is found unaided (I2410) (`35253fcf8`)
- **deploy**: a deployment names its provider and its provider files are derived (I3820) (`27d017ea1`)
- **reasoning**: an Anthropic API advisor is declared, so API credits can pay for review (I3663) (`f57147f13`)
- **models**: a vendor declares its funding sources with vendor evidence (I3660) (`28da69d37`)
- **release**: release.currency measures the landed work that waits past the last release (`6d9b6d445`)
- **core**: one portable core runs on the host and as wasm32, held by one executed fixture (I2784) (`03318debc`)
- **yaml**: a flatten is cached by its content under .ai/local/cache/flat (I0821) (`aafa0179f`)
- **gates**: gate state is something the runtime can answer (I1512) (`e46cb5d11`)
- **mesh**: a node key's succession is signed and confers trust only through its predecessor (`5cc6804b0`)
- **intent**: the CLI and the Cockpit show the three counts of unlinked work (`c8f72354a`)
- **gates**: done is one declared policy and a derived stage (`9d3b033b9`)
- **intent**: realize() classifies unlinked work as empty, before_intents or orphan (`c4ec0b183`)
- **mesh**: a node file holds the store it serves and the providers it was told about (`34949bd86`)
- **mesh**: a node reads what it offers from a local declaration and a bounded probe (`39d709d0a`)
- **mesh**: a build-cache offer is a typed model and selection is a pure function (`e5c94aece`)
- **telemetry**: the telemetry record names who reported each fact (I0301) (`4e6e7d765`)
- **mesh**: the tailnet is a discovery provider, so a machine joining it is found unaided (`febf05332`)

### Fixed

- **pages**: only an ISO 8601 time is a build's start time (`a44676d18`)
- **top**: a milestone takes a free order, and case 730 counts the advisors it declares (`b144038d8`)
- **ci**: install-check names a launcher that closed its pipe instead of dying on it (`f49a73994`)
- **top**: train 7's decision takes ADR 0226, and a probe starts a busy program again (`d55a5af86`)
- **plan**: the scope-conflict scan allocates nothing, and a failed case's reason is kept (I4223) (`a296782bc`)
- **mesh**: the cache provider's payloads live in the package the image compiles (`7da45a51f`)
- **integration**: keep the declarations page at fifty (`288ff37ab`)
- **top**: the pre-push gates train 7's own changes met, each at its source (`0a961cf30`)
- **integration**: a forge reply cut off on its way is asked again (`e6c43203e`)
- **site**: a run that /trends/ draws is decided by provenance (I4220) (`8337fde02`)
- **top**: the warm UI screens borrow the plan instead of copying it (I4200) (`43f1acecb`)
- **top**: the timed economics cases run alone (I4199) (`4b9aa6eb9`)
- **top**: the portable core builds in its own build directory (`2389d4b13`)
- **top**: a case that compiles never starves its neighbours (I4199) (`1bf59a9cf`)
- **top**: train 6's new case rules meet two members, each fixed at its source (`e6949e691`)
- **top**: train 6's site and structure jobs take the build job's executable (`96de70c88`)
- **top**: train 7's test reds, a doc example and a table header at their sources (`6eaf3613d`)
- **top**: train 7's gate-model reds, a member gate laned and a member case planned (`eb1baf7e4`)
- **top**: train 5's suite and Cockpit reds, fixed at their sources and derived once (`d078fb230`)
- **top**: train 5's UI bar is the bar its site renders (`9c17a4b25`)
- **top**: train 7's receipt reds, fixed at their sources and derived once (`b747ce88c`)
- **top**: train 6's pipefail and inventory reds, fixed at their sources (`00df03719`)
- **top**: train 6's quality red, Lane::as_str carries its example (`f11849aa0`)
- **test**: a fixture holds the skills the site generator reads (I3999) (`33c194867`)
- **build**: the shared build dir stays under its bound while builds run (I3959) (`b3ace7b45`)
- **top**: train 5's site red, a doc example in a schema description scrolls (`3c26d6a6e`)
- **mesh**: the build-cache ensure asks the context for its repository (`91d6681ef`)
- **top**: case 3002 holds the explicit build to the bound, not the launcher (`e34d028af`)
- **top**: case 3248 runs its fixture without the suite's MAJORDOMUS_BIN_COMMIT (`329906b54`)
- **top**: case 3032 reads the rust-quality gate's bin/majordomus-cli line (`fb8272978`)
- **top**: the blackboard's test peer names #2398's address field (`afc122670`)
- **top**: mesh.build_cache.env joins the mesh module's projection and query test lists (`e62b2789d`)
- **build**: the cache start lives in the sccache wrapper, not in lib/ (`61a50ef65`)
- **mesh**: the cache provider names no Prismatic path and its case hashes without shasum (`474370f78`)
- **top**: train 5's pre-push reds, I3939's scope names its case and rustfmt (`643f9e269`)
- **top**: train 5's case 94 red, case 3970 runs in plan-record-cases (`22cecde11`)
- **mcp**: the address docs name the _meta key, not a private constant (`f42a4bcd4`)
- **mesh**: the env module's intra-doc links name no redundant target (`691a70126`)
- **mesh**: a server bound to every interface advertises the addresses it has now (`22f02f456`)
- **serve**: an idle server leaves by itself; machine traffic is not use (`8fdebcaeb`)
- **build**: no entry path or gate builds; a stale executable is a same-generation release or exit 12 (`8c8a331fc`)
- **mesh**: a remote node's loopback runtimes are not dialed (`cb6494220`)
- **lifecycle**: a task started before the first commit is not diverged (I3995) (`39f371117`)
- **plan**: the intent's proofs are planned commands and every criterion is served (`8f672d83a`)
- **ci**: the provider checks out one commit, within five minutes (`d16cbee9f`)
- **hooks**: a push that only deletes refs plans nothing (`68cf7bebb`)
- **devcontext**: source ranges free their spans, keep one-line definitions, and rank below objects (`174b97284`)
- **top**: train 4's site and suite reds, fixed at their sources and derived once (`e173188a3`)
- **ci**: the provider push sends its slug in one request, and case 3821 holds the push to it (`6d37d0cb5`)
- **ci**: a push is planned from the commits it sends, not the working tree (`a56a5e079`)
- **devcontext**: every exported source-range item has an example; sorts are canonical (I3171) (`f48de8a90`)
- **pages**: a hung Pages build is replaced by a declared bound (I3902) (`9daede484`)
- **peers**: the peer board fits one answer, and initialize tells the caller about itself first (`ab4985cb2`)
- **deploy**: a path crate is in the build context and the dependency stage (I2784) (`70afed8c2`)
- **deploy**: a path crate is in the build context and the dependency stage (I2784) (`059e010bb`)
- **derive**: a merge of master derives in one go, both parents as history (`1a8d55357`)
- **health**: a deployed build without a checkout names its commit from its build identity (I3822) (`e2c9675d6`)
- **ci**: the provider push authenticates from its secrets through a credential helper (`16a28e3b7`)
- **advisors**: the codex advisor answers to the answer schema, and only its file is an answer (`d888a790b`)
- **plan**: I3829 names the provider rule at its new version (I3287) (`dc514006f`)
- **site**: the homepage counts what was proven and shows no backlog (I3916, I3917) (`3071f0f8c`)
- **top**: train 4's last reds, and the GitHub projection reads every issue (`516459918`)
- **ci**: the coverage gate reads multi-line and raw strings to the end of a test module (I3919) (`caee9bbaf`)
- **plan**: the new issues' scopes name only files that exist (`daaf4e367`)
- **top**: train 4's coverage, owed by its members, paid with tests and fixed at the source (`96556f1e8`)
- **top**: train 4 suite reds: marketing.toml within its budget, case 3480 not its own unit (`b364af4e9`)
- **top**: train 4's CI reds, fixed at their sources and derived once (`a6489f162`)
- **top**: train 3b's CI reds, fixed at their sources and derived once (`2034d4a3f`)
- **top**: train 3b's pre-push reds, fixed at their sources and derived once (`a01a4280c`)
- **ci**: the provider workflow skips with a notice until its secrets are set (I3821) (`47d0dfca8`)
- **ui**: the intent workspace folds its shared-scope pairs to the first 25 (I3840) (`d621aee64`)
- **release**: a landing waits from when it reached the trunk, and a bound alone is never overdue (`04a4cdc9e`)
- **editors**: a personal command that hides a projected skill is named, in editors.status and doctor (`3e687b162`)
- **ui**: a wake lock is asked for once, and one granted unwanted is given back (`6e402dcf1`)
- **adr**: a branch that renumbered its decision away claims nothing at the old number (`ace037903`)
- **ui**: the kiosk keeps the screen awake while it is shown (`85c4ccd26`)
- **runtime**: a rebuild never replaces the executable a live lease names (`2b9dc2d98`)
- **mesh**: a dialer reads no reply beyond its bound and no more candidates than the bound (`b862b8cda`)
- **mesh**: a hello older than the one that made a link cannot displace it (`2dbf9a0f9`)
- **mesh**: a host-scoped build-cache offer is usable only by its own node (`fac63421b`)
- **mesh**: build-cache model owes examples and one order (order-check, rust-quality) (`e68029076`)
- **doctor**: judge only the generated region of a region projection as a bootstrap (I2588) (`4cc8811be`)
- **monitor**: a cancelled run is news, a deployed Pages run needs help, the cursor is kept (`03222902a`)
- **mesh**: the tailnet's peers are a set, not a sort site (`9c47d6d94`)

### Performance

- **site-data**: refuse the manifest before deriving; case 95 runs three generations (I2115) (`1cb762146`)

### Documentation

- **serve**: the idle module shows its lifecycle and every method an example (`3f9483bd0`)
- **mesh**: the followed endpoints carry their examples (I4013) (`25db77504`)
- **adr**: no ordinary command builds is ADR 0224 (`9d38456bb`)
- **campaigns**: the Mission Control pack is in the manifest and the index (`3b19fb9e3`)
- **campaigns**: the one-click Mission Control prompt pack is recorded for every peer (`840efde32`)
- **adr**: no ordinary command builds — ADR 0222 supersedes ADR 0172, I3180 is cancelled (`6d4e35012`)
- **ai**: reconcile ChatGPT capture coverage and native-client evidence (`a64709651`)
- **plan**: I3287 is recorded done with its case as evidence (I3287) (`7801f7e3d`)
- **plan**: merging and referencing are enforced (I3970-I3979) (`e6deaf9e9`)
- **plan**: the telemetry record is done, and its writer is I3288 (I0301) (`7e7b4c218`)
- **plan**: the GitHub projection reads every issue (I3920) (`080a40af4`)
- the tool's size and its registry figures are said truthfully (I3918) (`0cb26dda0`)
- **plan**: the homepage tells a stranger the truth (I3916-I3919, I3449 to p0) (`784a3727a`)
- **plan**: landing green work survives any session, restart and machine (I3910-I3915) (`b52038cfc`)
- **deploy**: every exported provider item carries an example (I3826) (`75c02225f`)
- **plan**: I3663's scope names the model catalogue it adds claude-opus-5-5 to (`7b64c2a08`)
- **adr**: a hosted server is a rendezvous only over TLS, its key a secret (ADR 0220, I3821-I3827) (`9b6bc4376`)
- **models**: every funding item of the catalogue carries an executable example (I3660) (`e9f596482`)
- **plan**: the funding plan is cut to what vendor evidence carries (`c64a74dcc`)
- **plan**: every model call is paid from a declared funding source (`e0b088281`)
- **mcp**: ReplyRefused::of carries an example of its own (`a9df9d908`)
- **plan**: the client launch configurations have a carrier (I2148) (`5e3cc2d99`)
- **mcp**: the MCP reference repeats no registry entry (I2732) (`5b2c2813c`)
- **adr**: the join case is 3010, 3004 is another's (ADR 0156) (`4caf2546c`)
- **adr**: a session joins the cluster before it works — current, announced and useful (ADR 0156) (`40eb57657`)
- **mesh**: the tailnet provider and its constructor carry an example (`d8f32cf14`)

### Tests

- **deploy**: the dependency stage admits exactly the declared path crates (`cff7ee629`)
- **top**: the changed items train 7's coverage run found unreached, reached (`3d5ea522d`)
- **top**: train 7's members owe their coverage, and the train pays it (`5b39028ba`)
- **top**: every closed-port test of the build cache asks one source of refusing ports (`4864d4057`)
- **top**: the closed-port probe test asks fresh ports, not one another test can take (`37dac4c82`)
- **top**: train 6's 9 uncovered lane and tier items, each proved (`7f722caaf`)
- **mesh**: close the train-6 block's last 39 uncovered build-cache items (`376493cd4`)
- **mesh**: read the machine's environment through a seam, not set_var (`8c84a5c87`)
- **mesh**: cover the train-6 block's build-cache env, ensure, key and address branches (`0e7ec629f`)
- **mesh**: cover the build-cache archive, ensure and offer error arms (`efaf0ae88`)
- **mesh**: cover the train-6 block's uncovered cache provision, S3, key, journal and monitor items (`4ca410a9c`)
- **top**: train 5's changed code is covered, and one run's chart is captioned once (`cca0100be`)
- **mesh**: case 3952 hashes with the repository's own SHA-256 helper (`9e417b517`)
- **derive**: case 3963 pins the sort in its stub executable (`e4c32fb3b`)
- **mesh**: the mesh module's declaration tests name mesh.build_cache.key (`cbc54b94c`)
- **ui**: the shared-scope route lists every pair and names what it needs (I3840) (`b83c6a0fd`)
- **release**: case 3470 asks release.currency over MCP, as a client does (`1d7940d5e`)
- **deploy**: every changed line of the slug assembler is covered (I3821) (`fdeee1ac3`)
- **release**: the reflog's malformed lines and the currency's benchmark cases are proved (`15a4aef13`)
- **mcp**: a bounded reply is covered to the last changed line (`cc0ca44f1`)
- **intent**: unlinked work is classed end to end (case 3057) (`817670473`)
- **mesh**: a lost append is said end to end (I2706, case 3052) (`40f3b6829`)
- **mesh**: the last uncovered branches of the build-cache model (coverage gate) (`4c059c962`)
- **mesh**: every branch of the build-cache model is covered by a test (coverage gate) (`742cdb13f`)
- **monitor**: the monitor cases wait for a server that answers, as long as the load needs (`042ebedee`)

### Housekeeping

- **derive**: train 7's derived data observes its merge of train 6 (`be4daac15`)
- **derive**: I4223's derived data observes its merge of master (`f6d61ebea`)
- **derive**: train 6's derived data observes its merge of master (`6b2a37bc9`)
- **derive**: delivery observes the committed merge of master (`a8334b035`)
- **derive**: delivery observes the pushed head, not the offload's snapshot (`d9207285b`)
- **plan**: the merged work is recorded (10 issues), with its evidence (`cc4aa2672`)
- **plan**: I0905 is done; its evidence is bound to the tests that already prove it (`e714307d6`)
- **derive**: the pages and registry observe the two case fixes (`b1ef7641b`)
- **derive**: delivery observes the committed block, not its throwaway twin (`b395eeb66`)
- **derive**: delivery observes the committed merge, not its throwaway twin (`439ea6019`)
- **plan**: plan the intent a-session-proceeds-through-the-mesh and file I4000-I4009 (`315af95a8`)
- **site**: delivery.json observes the merge commit, not the offload's input (`b223c73d2`)
- **economics**: the context selection is measured at f48de8a9090, and the default is unchanged (`55bab024b`)
- **site**: delivery.json observes the merge commit, not the offload's input (`d9d9313a7`)
- **generated**: re-derive after bringing master in (`3876124c6`)
- **economics**: the context selection is measured again at the compiler that names ranges (`461f0e2f0`)
- **generated**: re-derive after bringing master in (`80b3c3310`)
- **derive**: the derived data follows I3287's fix, derived once (I0301) (`2760d637c`)
- **generated**: re-derive after bringing master in (`20ee1133d`)
- **generated**: re-derive after bringing master in (`20e8f939c`)
- **generated**: re-derive after bringing master in (`a0eecfcf5`)
- **generated**: re-derive after bringing master in (`780b05368`)
- **generated**: re-derive after bringing master in (`86151175c`)
- **top**: train 4, its members applied on train 3b and the alpha.5 record, derived once (`35f852d9e`)
- **release**: record v1.0.0-alpha.5 and the metadata it publishes (`675af4c06`)
- **top**: train 3b, its members applied on train 3a and derived once, bump to 1.0.0-alpha.6 (`a57482183`)
- **plan**: I3823 serves the relay criterion of its intent (`c6eda5742`)
- **plan**: I3840 verified, its fold test recorded; the budget awaits CI (`d9c2330a8`)
- **plan**: I3060 closed, I3061 recorded (`8bf9978f8`)
- **generated**: re-derive after bringing master in (`266d26d48`)
- **generated**: re-derive after bringing master in (`d20e3f7e5`)
- **plan**: I2115's lane criteria move to I2350, verbatim (I2115, I2350) (`2937c955d`)
- **derive**: the derived data follows the committed merge of master 9a5a9b7752 (`3138c873a`)
- **derive**: the derived data follows the committed merge of master 9904000d2d (`3f6591416`)
- **derive**: the changelog, artifacts and site data read the committed merge (`6e480086d`)
- **derive**: regenerate after merging top integration/70-top-20261016 (`dcdf834ca`)
- **plan**: I2132 is done, both of its replays proven refused (`e5d2a83b0`)
- **derive**: the derived data follows the committed merge of master c0e3585dc8 (`ec174ee62`)
- **derive**: the derived data follows the merge of master (`f123c80b7`)
- **plan**: record I2588 as verified (I2588) (`80bc2035e`)

### Formatting

- **top**: rustfmt the provider browser table assertions train 7's fix widened (`15a60d4fc`)
- **peers**: the board that fits one answer is rustfmt-clean (`b379214d0`)

### Published

- `macos-aarch64` — majordomus-v1.0.0-alpha.8-aarch64-apple-darwin.tar.gz (`8a94f198f4c2`)
- `macos-x86_64` — majordomus-v1.0.0-alpha.8-x86_64-apple-darwin.tar.gz (`d4654a0e9e11`)
- `linux-x86_64-gnu` — majordomus-v1.0.0-alpha.8-x86_64-unknown-linux-gnu.tar.gz (`40036f83748a`)
- `linux-x86_64-musl` — majordomus-v1.0.0-alpha.8-x86_64-unknown-linux-musl.tar.gz (`1b24d28382ce`)
- `linux-aarch64-gnu` — majordomus-v1.0.0-alpha.8-aarch64-unknown-linux-gnu.tar.gz (`08cc36a1185f`)
- `linux-aarch64-musl` — majordomus-v1.0.0-alpha.8-aarch64-unknown-linux-musl.tar.gz (`464d467bad13`)


## v1.0.0-alpha.5 — 2026-10-22

### Added

- **site**: a memory edit names the pages each key reaches (I3092) (`37205d4a1`)
- **site**: a count inside a translated sentence is a slot (I3095) (`d296bf2ac`)
- **site**: a translated bar keeps its English headroom (I3091), and the lessons are decided (`0ada9533f`)
- **site**: translate the 8362 segments the alpha.1 build added to the Czech copy (`40d2a5875`)
- **handbook**: the handbook carries the outlook, derived from the tree (`6b762a886`)
- **site**: every article ends with the articles related to it, in its reader's language (`bf14f79f2`)

### Fixed

- **top**: #1789's reds, fixed at their sources and derived once (`76c578596`)
- **top**: train 3a's pre-push reds, fixed at their source and derived once (`8e06eb3be`)
- **handbook**: the outlook reads only the intent keys it shows (`f34dd4acb`)

### Documentation

- **site**: an article explains derivation, in English and in Czech (`a2d233d13`)
- **site**: an article explains the plan critique, in English and in Czech (`6de68f89d`)
- **site**: an article explains the cooperation model, in English and in Czech (`455304b1f`)

### Housekeeping

- **top**: train 3a, its members applied on the alpha.4 record and derived once (`c471d374c`)
- **release**: record v1.0.0-alpha.4 and the metadata it publishes (`7dd644289`)
- **release**: train 3a, site and docs members applied, bump to 1.0.0-alpha.5 (`3f32825b1`)
- **derive**: the derived data follows the committed merge of master 9a5a9b7752 (`d10ea0aff`)
- **plan**: I3094 is done by the alpha.1 top's pages.yml bounds, and master is derived once (`eef2f1cb5`)
- **site**: derive the master merge once (`5e604fffc`)

### Published

- `macos-aarch64` — majordomus-v1.0.0-alpha.5-aarch64-apple-darwin.tar.gz (`6204966024c0`)
- `macos-x86_64` — majordomus-v1.0.0-alpha.5-x86_64-apple-darwin.tar.gz (`e854ee236363`)
- `linux-x86_64-gnu` — majordomus-v1.0.0-alpha.5-x86_64-unknown-linux-gnu.tar.gz (`2106367929ab`)
- `linux-x86_64-musl` — majordomus-v1.0.0-alpha.5-x86_64-unknown-linux-musl.tar.gz (`6cdb68bffb32`)
- `linux-aarch64-gnu` — majordomus-v1.0.0-alpha.5-aarch64-unknown-linux-gnu.tar.gz (`ec943cc2672d`)
- `linux-aarch64-musl` — majordomus-v1.0.0-alpha.5-aarch64-unknown-linux-musl.tar.gz (`b1c8dfd1285f`)


## v1.0.0-alpha.4 — 2026-10-21

### Added

- **providers**: a provider declares its browser bridge, and preflight names the missing host (`b30ee30a1`)
- **ci**: the interaction probe runs in six shards, verified as one sweep (I3602) (`7666c365b`)
- **ci**: the site's browser probe runs in four shards that are the sweep (I2841) (`95357325d`)

### Fixed

- **top**: #1765's reds, fixed at their sources and derived once (`97391192d`)
- **top**: train 2's pre-push reds — canonical order, no eval, scopes that exist (`98707ace2`)
- **top**: the mesh doctor's remediation names `serve status`, not the lease path (`712729721`)
- **top**: cases 3247 and 3836 name why their globals look unused (`76158b704`)
- **top**: case 1018 counts the queue's tickets with a glob, not ls | grep (`374fa149e`)
- **site**: the plan page counts its findings instead of printing one row per pair (I3838) (`c278259ab`)
- **site-data**: the scenario cache engages; the tool's lib dir is compared physically (I0832) (`57c1a9d91`)
- **mcp**: the Claude in the browser sources are cited as text, not links (`6ea3de94b`)
- **derive**: waiting derives take the machine lock in the order they arrived (I3098) (`fe464a41d`)
- **test**: a case run removes its own scratch dir however it ends (I3834) (`34ca69681`)
- **ci**: a gated step leaves its receipt and a step bound follows its measured time (L1, L2, L7) (`6240bc2d7`)
- **ci**: the evidence job runs only when a planned lane ran tests (I3013) (`99727f47c`)
- **test**: case 3602 compares its route lists in C order (I2841) (`a9f29e563`)
- **ci**: the site probe runs in six shards, since the Czech copies doubled the routes (I2841) (`3ad4310f4`)

### Performance

- **derive**: reading the derive queue forks nothing (I3098) (`4b36890a9`)
- **site-data**: a scenario whose tool and use case are unchanged is not re-executed (I0832) (`73d26b129`)

### Documentation

- **plan**: the roadmap reads idd-workspace's release from the committed merge (`603164312`)
- **plan**: a node the mesh lists as present but cannot link says why (I3835) (`12c702cdc`)
- **mcp**: Claude in the browser reaches the mesh through Claude Code (`ebf58f233`)
- **rules**: a worker lands its own green head, and every bootstrap says so (I3824) (`8d394f3c2`)

### Tests

- **provider**: a bridge without its host is not looked for (`f21025614`)
- **site**: case 3848 reads its own plan, and says what its ceiling counts (I3838) (`0545642b7`)

### Housekeeping

- **top**: train 2 carries the v1.0.0-alpha.2 record, derived once (`b5140729d`)
- **release**: record v1.0.0-alpha.2 and the metadata it publishes (`81063e55d`)
- **release**: train 2, members applied, bump to 1.0.0-alpha.4, derived once (`69fd62f1d`)
- **derive**: delivery observes a commit origin holds (I3834) (`42a1ce389`)
- **release**: bump to 1.0.0-alpha.3, train 1 derived once (`784960b08`)
- **plan**: I2413 is recorded done; a worktree already cooperates through the primary (`42291f9ef`)
- **plan**: I1985 and I2282 are recorded done, each on a test that ran green on master (`35a559f3e`)
- **plan**: I3013, the evidence job runs only when a planned lane ran tests (`2937f3ebf`)

### Published

- `macos-aarch64` — majordomus-v1.0.0-alpha.4-aarch64-apple-darwin.tar.gz (`87b5f17a05d8`)
- `macos-x86_64` — majordomus-v1.0.0-alpha.4-x86_64-apple-darwin.tar.gz (`db150297c237`)
- `linux-x86_64-gnu` — majordomus-v1.0.0-alpha.4-x86_64-unknown-linux-gnu.tar.gz (`aadc60edec30`)
- `linux-x86_64-musl` — majordomus-v1.0.0-alpha.4-x86_64-unknown-linux-musl.tar.gz (`f4022f2b79ab`)
- `linux-aarch64-gnu` — majordomus-v1.0.0-alpha.4-aarch64-unknown-linux-gnu.tar.gz (`1aafbed99a30`)
- `linux-aarch64-musl` — majordomus-v1.0.0-alpha.4-aarch64-unknown-linux-musl.tar.gz (`49152c6ced56`)


## v1.0.0-alpha.2 — 2026-10-21

### Fixed

- **ui**: the intent workspace borrows the plan instead of copying it (I3018) (`9129c6697`)

### Documentation

- **plan**: record I3079, the Deployments table's link that is too small a target (`ea81bca38`)
- **plan**: the roadmap reads idd-workspace's release from the committed merge (`46ffa4e6e`)
- **plan**: the derived plan page of I3009 (`ce424b866`)
- **plan**: the landed IDD work is recorded, and idd-workspace is done (`3c9797a27`)

### Housekeeping

- **release**: bump to 1.0.0-alpha.2, derived once (`905159df7`)
- **top**: the W0 records top, derived once (`573df967b`)
- **release**: record v1.0.0-alpha.1 and the metadata it publishes (`91c0b5d56`)
- **plan**: record five delivered development-runtime issues as done (`1c0a10454`)
- **plan**: four merged issues are recorded done by their own proof (`48092d192`)
- **plan**: record 18 merged issues as done, each with its own evidence (`c2b6d98cd`)

### Published

- `macos-aarch64` — majordomus-v1.0.0-alpha.2-aarch64-apple-darwin.tar.gz (`baae30271835`)
- `macos-x86_64` — majordomus-v1.0.0-alpha.2-x86_64-apple-darwin.tar.gz (`5236640245e0`)
- `linux-x86_64-gnu` — majordomus-v1.0.0-alpha.2-x86_64-unknown-linux-gnu.tar.gz (`12dc25ea6388`)
- `linux-x86_64-musl` — majordomus-v1.0.0-alpha.2-x86_64-unknown-linux-musl.tar.gz (`376d53a9ca37`)
- `linux-aarch64-gnu` — majordomus-v1.0.0-alpha.2-aarch64-unknown-linux-gnu.tar.gz (`3b246c41cf04`)
- `linux-aarch64-musl` — majordomus-v1.0.0-alpha.2-aarch64-unknown-linux-musl.tar.gz (`b4373e9151e5`)


## v1.0.0-alpha.1 — 2026-10-20

### Added

- **site**: the Czech copy translates the segments the merge with master brought (`370fdf4cc`)
- **site**: the Czech copy is complete and its rule is enforced (`ce0b7e58c`)
- **site**: every page in Czech, derived from the English build and a translation memory (`0ce1424df`)
- **site**: every article reads in every language, and a translation keeps up with its source (`4624a38df`)

### Fixed

- **top**: suite budgets re-derived from the release head's run (`9a5a9b775`)
- **top**: a milestone title GitHub can hold, and Pages bounds for two languages (`88ce9857e`)
- **top**: the site-languages step leaves the receipt the ci verdict asks for (`bd676bbe3`)
- **top**: the UI audit warms each surface and retries a timed-out load once (I3090) (`82ac259d0`)
- **top**: the bar's own "How it works" is Princip in Czech too (`35f5241ad`)
- **top**: the UI audit samples the served docs' Czech copy, and its breadcrumb fits at 320 (`9bf2b7258`)
- **top**: the Czech bar fits at 768 px, and a graph's reset restores the page's own words (`ca83931eb`)
- **top**: the UI audit samples a language copy under a mount as well as at the root (`5b5358d91`)
- **top**: two more cases the Czech copy reached, cases 97 and 1071 (`aa936b537`)
- **top**: what the Czech copy of the site asked of the checks that read every page (`4fc1ffe7f`)
- **top**: the reds the re-cut's own checks found before its push (`f2420807a`)
- **site**: every language's copy is in the sitemap, and an alias leads to its target's copy (`b3bf6ed7e`)
- **capture**: a provider hook declares its timeout, and the session end closes inside it (I3060) (`c6556694a`)
- **ui**: the UI and the Cockpit wear the site's bar with the language switch (`b12a5fcb5`)
- **test**: case 3590 compares the second pass by diff, not shasum (`c24cdc1a2`)
- **ui**: every Knowledge tile reads whole at a phone's width (I3025) (`b9b6dbfa5`)
- **ui**: the Knowledge manual link names a page the site has (I3024) (`19d5dda61`)
- **plan**: the promoted records pass the pre-push gates and name only what they touch (`596cd5ebb`)
- **plan**: the promoted records wait on what 0.30.0 landed and on what their owners answered (`51916f41d`)
- **plan**: the promoted records name only paths that exist and break two order ties (`d3229ef16`)

### Documentation

- **plan**: the owner's two 10-19 briefs become a proof program and a gap-only cognitive fabric (`bd8ca33c1`)
- **plan**: promote what the owner's ChatGPT project decided, each record naming its source message (`5ef3c3251`)

### Pipeline

- **validate**: three browser probes outgrew their bounds; each bound grows with its measurement (`6e2aadba1`)

### Housekeeping

- **derive**: the I3060 plan page (`07f15d4c0`)
- **release**: record v0.30.2 and the metadata it publishes (`94372c0fa`)
- **derive**: the projections follow the merge with master (`b0e833128`)
- **plan**: revive the derive, CI critical-path and IDD drafting records that never landed (`321f5dde6`)

### Published

- `macos-aarch64` — majordomus-v1.0.0-alpha.1-aarch64-apple-darwin.tar.gz (`afa2fe2f0e19`)
- `macos-x86_64` — majordomus-v1.0.0-alpha.1-x86_64-apple-darwin.tar.gz (`84d7c08db6f1`)
- `linux-x86_64-gnu` — majordomus-v1.0.0-alpha.1-x86_64-unknown-linux-gnu.tar.gz (`45b14249c8fa`)
- `linux-x86_64-musl` — majordomus-v1.0.0-alpha.1-x86_64-unknown-linux-musl.tar.gz (`298c3832116c`)
- `linux-aarch64-gnu` — majordomus-v1.0.0-alpha.1-aarch64-unknown-linux-gnu.tar.gz (`46302b271585`)
- `linux-aarch64-musl` — majordomus-v1.0.0-alpha.1-aarch64-unknown-linux-musl.tar.gz (`38ff14c89ad3`)


## v0.30.2 — 2026-10-19

### Fixed

- **top**: I2917's nothing-to-move arm of the intent's primary action has a test (`dd4c0a87c`)
- **ui**: each intent has one primary action (I2917) (`d93b93ad1`)
- **ui**: the intent workspace's panels stand in the order a person asks (I2916) (`069970820`)
- **ui**: the intent is the workspace's title and every panel asks a plain question (I3001) (`d65aee8a7`)
- **pages**: a claim newer than the ledger is not yet recorded, not a refusal (I2853) (`29acb77dd`)
- **ui**: every record text on the workspace is drawn as prose (I3000) (`d09232416`)

### Housekeeping

- **release**: record v0.30.1 and the metadata it publishes (`e6268966b`)

### Published

- `macos-aarch64` — majordomus-v0.30.2-aarch64-apple-darwin.tar.gz (`b44b8da00054`)
- `macos-x86_64` — majordomus-v0.30.2-x86_64-apple-darwin.tar.gz (`60735a8fdad4`)
- `linux-x86_64-gnu` — majordomus-v0.30.2-x86_64-unknown-linux-gnu.tar.gz (`eed5fce0af6b`)
- `linux-x86_64-musl` — majordomus-v0.30.2-x86_64-unknown-linux-musl.tar.gz (`2b404e30a78f`)
- `linux-aarch64-gnu` — majordomus-v0.30.2-aarch64-unknown-linux-gnu.tar.gz (`6292503a7ab1`)
- `linux-aarch64-musl` — majordomus-v0.30.2-aarch64-unknown-linux-musl.tar.gz (`05777434fd01`)


## v0.30.1 — 2026-10-19

### Fixed

- **top**: case 3003 reads the Knowledge screen I3022 and I3023 draw (`9904000d2`)
- **top**: #1412's real reds: the footer commit, the record cases, coverage, a pipe flake (`70d127e14`)
- **top**: I3007's objective names the template links without writing a placeholder (`7a20e31fd`)
- **top**: case 3363 sorts with LC_ALL=C, so its order is the machine's no more (`92f790d39`)
- **ui**: the Knowledge screen reads in review order, in plain labels (I3023) (`076312f8a`)
- **ui**: an empty knowledge queue is one sentence, not two empty tables (I3022) (`820602967`)
- **site**: a commit that a record or the site's data names is in the history (I3007) (`c0b799497`)
- **offload**: a remote derive builds only inside its own run directory (I2882) (`a4c19d60f`)
- **ui**: the Knowledge review reads as steps, each command as code (I3021) (`6f54b4153`)
- **site**: the site says a test proves a claim only as far as the evidence does (I2965) (`c90d10c01`)
- **doctor**: no work whose answer cannot change a finding (`ed72c1325`)
- **ui**: the workspace's headline numbers are the chosen intent's (I2907) (`a762f7456`)
- **doctor**: the work no remote holds is counted in one walk (`ba44bed3f`)
- **docs**: the MCP surface, claims and README name the eight writing capabilities (I2962) (`1b9b93d1b`)
- **release**: a latest.json naming any pre-release is refused, not only the one published (I2829) (`9f8baafee`)
- **docs**: the README's exit codes are the commands' declared contracts (I2963) (`ae2a2a322`)
- **site**: the policy and roadmap pages name the .ai/repo layout in force (I2967) (`886cd9a18`)
- **install**: the smoke server binds its own loopback address (`0707822cd`)

### Performance

- **server**: every checkout is asked at once, sixteen at a time (`baadbbfb0`)

### Documentation

- **server**: the public module doc names no crate-private item (`317ae1312`)

### Housekeeping

- **release**: record v0.30.0 and the metadata it publishes (`a0e7dd961`)

### Published

- `macos-aarch64` — majordomus-v0.30.1-aarch64-apple-darwin.tar.gz (`0dfaa6a41224`)
- `macos-x86_64` — majordomus-v0.30.1-x86_64-apple-darwin.tar.gz (`f8b8424efb08`)
- `linux-x86_64-gnu` — majordomus-v0.30.1-x86_64-unknown-linux-gnu.tar.gz (`83b29af38b2a`)
- `linux-x86_64-musl` — majordomus-v0.30.1-x86_64-unknown-linux-musl.tar.gz (`9022bbd67e6f`)
- `linux-aarch64-gnu` — majordomus-v0.30.1-aarch64-unknown-linux-gnu.tar.gz (`e79b36d624e9`)
- `linux-aarch64-musl` — majordomus-v0.30.1-aarch64-unknown-linux-musl.tar.gz (`a50464e71535`)


## v0.30.0 — 2026-10-18

### Added

- **site**: the homepage counts the delivery the history proves, each count a link (I2851) (`ca277ce74`)
- **ui**: the intent workspace shows why its intent stands where it does, and its moves (I2789) (`49f348d2e`)
- **ui**: the intent workspace reads itself again when a move ends on its server (I2783) (`afaecfe34`)
- **ui**: an issue is started, verified and finished from the intent workspace (I2781) (`6811d1eeb`)
- **site**: /plan/ names its observed commit and each milestone's release (I2743) (`5485fee6b`)
- **plan**: merged work the plan has not recorded is drift, gated past a window (I2741) (`09c334079`)
- **ui**: the roadmap reads delivery, and its now is where work is (I2744) (`c3f6618cf`)
- **worktree**: a checkout says how far it is from the remote trunk, and a stale one fails doctor (`f4635ab71`)
- **plan**: an issue's delivery is inferred from merges, releases and the forge (I2740) (`ee02079b1`)
- **site**: the roadmap draws every milestone in the graph's order (I2743) (`4d75789ea`)
- **plan**: a milestone's version is inferred from the release that carried it (I2742) (`f8fe91332`)

### Fixed

- **site**: a navigation link's fragment names an element, not a file (`2dfff9698`)
- **ui**: an unobserved panel's reason wraps at phone width (I2943) (`81d1fb302`)
- **release**: a version's part count is decided once, so no parse branch is unreachable (`0dea534e7`)
- **ui**: the id offset in the workspace's prose is derived, not defaulted (I2900) (`c969471ec`)
- **ui**: the standing panel's subheadings wear a class the kit defines (I2900) (`1808c4ccb`)
- **ui**: every issue id on the IDD workspace is a link; a missing library is not asked for (I2900) (`3782bedcf`)
- **top**: the IDD sub-headings use the kit's panel title (`01b1aae0a`)
- **ui**: every class a screen gives an element is one the stylesheet defines (I2852) (`500f0d48b`)
- **pages**: a published page that does not answer at all is asked again like a 503 (I2813) (`3f36b5bd2`)
- **ui**: the Knowledge screen states nothing its checkout could not observe (I3020) (`4767d1be4`)
- **integration**: a forge setting that could not be read is unknown (`269bf5d67`)
- **ui**: a mesh this process is not on is said as not observed, never as zero or no (I2943) (`a636d9448`)
- **install**: a dry run initialises nothing, and the install guide says only what is true (I2942) (`5b4f8ba6a`)
- **ui**: the published PR Drain carries its pull requests and counts them by lane (I2850) (`82ace9a10`)
- **ui**: the IDD lists wrap a long path, and the shared-scope heading follows its panel's (I2789) (`9bcbc988b`)
- **top**: 71b2's cases skip properly, its gate model parses, case 18 mutes every suite step (`4ada9a40e`)
- **release**: release next raises from the trunk the repository names (I2849) (`6dceb97e7`)
- **pages**: every snapshot read step can ask the forge (`3aa828e5b`)
- **lease**: a server's release is ordered by semantic-version precedence (`f7183e329`)
- **init**: a fresh init leaves doctor clean (I2941) (`4f3b3490d`)
- **install**: the installer refuses an unpinned pre-release, in its source (I2829) (`261c45caf`)
- **release**: an alpha is published as a pre-release end to end (I2829) (`2d4131a4a`)
- **pages**: a published page that answers 503 is asked again before it is called missing (I2813) (`9d016bb19`)
- **process**: a tool written a moment ago is run once its writer lets go (I2827) (`8a82388e0`)
- **release**: a pre-release version is parsed, ordered and bumped to (`5b2bfa723`)
- **scope**: a task scoped . claims the whole repository (`2dedd2d78`)
- **ci**: plan-record-cases runs the plan surfaces' cases 3100-3104 (`1135b5236`)
- **build**: the build dir sweeps sort in one locale (`f6d28ada9`)
- **knowledge**: a candidate cites its own commit, and a superseded decision yields none (I1985) (`2d3ee66da`)
- **build**: build dirs are swept before every build, under cargo's lock (I2713) (`36de9d126`)
- **scope**: a task scoped . claims the whole repository (`c7def1192`)
- **release**: a pre-release is never published or resolved as the stable release (`13670b1f0`)
- **mcp**: a client always gets a server, and a bare session starts by joining the cluster (ADR 0172) (`80eb4a539`)
- **plan**: a milestone DONE with no issue is released where its evidence was recorded (I2742) (`dcabb286c`)
- **plan**: release-record-cases runs 3102 and 49; release records ordered by a set (I2742) (`c67c4407e`)
- **plan**: the delivery and drift code is covered and ordered canonically (I2741) (`043955cc7`)
- **build**: case 08 declares the idle incremental-session sweep as its one non-temp rm -rf (`51246dd95`)
- **plan**: released_in reads the release records, not the tags (I2742) (`e82e6edb9`)
- **pages**: a publication that did not arrive is a finding, never a green step (I2661) (`194e207f3`)
- **build**: a build prunes the idle incremental sessions of its build directory (I2713) (`309665056`)
- **reap**: disk and load are bounded before the cliff, not at it (`7be59b023`)

### Documentation

- **release**: Version's docs name the distribution module without linking it (`1a74d3627`)
- **plan**: the owner's /ui/idd review is milestone idd-reads-at-a-glance (I2914) (`4b15b432b`)
- **adr**: the alpha declaration states its install pin and its network limits (`22ed9fd9a`)
- **adr**: the hub re-admit step is the supported command (`4d0da4d80`)
- **adr**: a macOS mesh hub upgrade can drop it from the tailnet, as an alpha limit (`c2746d4af`)
- **plan**: I2842, the live site is verified on a schedule (after the alpha) (`70b5804e8`)
- **plan**: the site-probe split is I2841, in this lane's id block (`b6be53111`)
- **plan**: the MCP cancellation issue is I2840, and I2829 claims no case it does not add (`33b682111`)
- **plan**: I2829, the site's browser probe is split (after the alpha) (`0549a8cb1`)
- **plan**: renumber the MCP cancellation issue to I2826 (`644a6d053`)
- **adr**: the alpha declaration points at no record master will not hold (`e2d69a022`)
- **adr**: v1.0.0-alpha.1 declares what it can do and its limits (ADR 0175) (`d605fbeb4`)
- **plan**: a released mesh claim reaches every runtime that holds it (I2825) (`91ed1dac1`)
- **plan**: the MCP cancellation issue is recorded, a cancelled call stops (`907697ecb`)
- **worktree**: the trunk-distance surface carries its examples, and its MCP tool is invoked (`3d16d1c08`)

### Tests

- **ui**: every branch of the workspace's id linking and the graph library's absence is held (I2900) (`a119ab213`)
- **ui**: the PR Drain's lane cards are read from the one map the queue answers (I2850) (`38f2e9b7f`)
- **release**: a pre-release is ordered, printed and refused the way the release paths rely on (`aa68933a7`)
- **ui**: each session-ledger line is read on its own (I3020) (`6afb12403`)
- **ui**: a drift message in the standing panel reads whole with its issue id a link (I2900) (`d0d2d01f7`)
- **ui**: case 3220 walks its loop in the ui job, not wherever a browser exists (I2812) (`627eae882`)
- **ui**: case 3220's live step shows a move only through the channel it holds (I2812) (`bc782b40f`)
- **server**: a pre-release server is older than its final; unorderable means not semver (`1bb0dcc04`)
- **process**: the busy retry is proved on every platform, not only Linux (`f7385e854`)
- **ui**: the standing panel's realising rows and drift are rendered from an answer (I2789) (`fab8869e2`)
- **mcp**: the oversized body is one byte over the bound, so its 413 is always read (`2306da302`)
- **release**: case 3224 measures its fixture whatever MJ_ROOT the caller exports (`877ff431c`)
- **pages**: the 503 retry's case is 3226, not the 3221 #1312 carries (I2813) (`fe777b442`)
- **ui**: the IDD loop is walked end to end in a real browser (I2812) (`f199ded8a`)
- **plan**: plan.flight is one answer over HTTP and MCP (`a92f60917`)
- **worktree**: the trunk distance is covered to the last changed line (`9223ef689`)
- **plan**: every path of release_records and release_key is covered (I2742) (`a1be751f0`)
- **backlog**: the integrator case copies what the gate now reads (`d8936063c`)

### Pipeline

- **ui**: the audit's bound follows the surface it walks (`2d0c9df40`)
- the site job's bound keeps headroom, and plan-record-cases runs the IDD loop (`68062918e`)

### Housekeeping

- **derive**: the 0.30.0 top derived after its coverage merge (`ad1dfe7ce`)
- **derive**: delivery.json observes the pushed 0.30.0 top (`bd3219d01`)
- **derive**: I2900 over the 3220 deferral (I2900) (`a77d2a93b`)
- **derive**: I2900 over 0.29.1 (I2900) (`2e80ec3f7`)
- **derive**: I2900 over 71b2's head with master 707a04c (I2900) (`03aa72b7b`)
- **plan**: I2942 is done, with the evidence that settles it (`2b383cf28`)
- **plan**: I2943 is done, with the evidence that settles it (`21b661513`)
- **plan**: I2826 names its own case, so the plan status names no missing file (`4960fa3c7`)
- **release**: bump the version to 0.29.1 (`f17855bf9`)
- **plan**: the VERIFY issues whose work is on master are settled (`f9c4d9b94`)
- **plan**: I2941 is done, with the evidence that settles it (`65220a9e5`)
- **release**: record v0.29.0 and the metadata it publishes (`707a04c7c`)
- **plan**: I2829 is done, with the evidence that settles it (`c554c08bc`)
- **plan**: four issues whose code the integration top carries are recorded done (I2706) (`90bf65b4a`)
- **quality**: the baseline drops the three examples Version now carries (`c50657244`)
- **derive**: the merged #1312 tree (`7cf2236f2`)
- **derive**: #1299 over the release candidate 0a19b7fe3ec (`50905c443`)
- **derive**: the batch derived over the 0.29.0 release candidate (`fd2023800`)
- **plan**: eight issues whose code master already carries are recorded done (I2414) (`8d0b36d99`)
- **evidence**: the trunk's recorded run reaches the tracked ledger, and the baseline holds it (`99667818d`)
- **plan**: close I2601, I2745, I2746 and two milestones whose evidence holds (`58d522c1a`)
- **release**: bump the version to 0.29.0 (`040dcb14d`)
- **plan**: delivery.json observes the merge of #1304's CI fix (I2748) (`c34a3c008`)
- **plan**: I2743 evidence recorded (I2743) (`33cd3f9a0`)
- **plan**: eight merged issues are recorded done, the rest of the drift is planned (I2748) (`822cd914f`)
- **plan**: record I2744's evidence (I2744) (`b4247f312`)
- **plan**: I2742 evidence recorded and verified (I2742) (`8642d7437`)

### Published

- `macos-aarch64` — majordomus-v0.30.0-aarch64-apple-darwin.tar.gz (`e15586cfe0da`)
- `macos-x86_64` — majordomus-v0.30.0-x86_64-apple-darwin.tar.gz (`ecea13aa20dc`)
- `linux-x86_64-gnu` — majordomus-v0.30.0-x86_64-unknown-linux-gnu.tar.gz (`7fcc015c688b`)
- `linux-x86_64-musl` — majordomus-v0.30.0-x86_64-unknown-linux-musl.tar.gz (`dff34a9a3015`)
- `linux-aarch64-gnu` — majordomus-v0.30.0-aarch64-unknown-linux-gnu.tar.gz (`0fea0290e04c`)
- `linux-aarch64-musl` — majordomus-v0.30.0-aarch64-unknown-linux-musl.tar.gz (`51df29dea8b4`)


## v0.29.0 — 2026-10-17

### Added

- **features**: every capability module and every kind is named by a stable feature (`359f8223f`)
- **ui**: the intent workspace shows an intent and what serves it (I2780) (`f1cb08bb7`)
- **http**: a served surface names the checkout serving it (I1031) (`f178d2bfe`)
- **ui**: the UI and the Cockpit wear the site's navbar (I2770) (`3a8ca8549`)

### Fixed

- **ui**: a monospaced path or id wraps at phone width instead of widening the page (`4a4573690`)
- **top**: the create under the lock is covered, and a named skill never reads as unnamed (`af5c8efe7`)
- **top**: the UI wears the bar the site renders, and two milestones no longer share an order (`40f558749`)
- **top**: batch 71a passes shellcheck and covers the create planned under the lock (`81744db86`)
- **top**: the navbar heading has an anchor zola and GitHub agree on (`d0de74268`)
- **top**: batch 71a covers its changed code and meets the structure gates (`bc9132ba0`)
- **worktree**: StartPoint's reference and fallback carry their examples (`fe9b99405`)
- **pages**: the published UI reads the evidence CI recorded (I2786) (`302adff89`)
- **ui**: the intent workspace passes rustfmt and clippy (I2780) (`8def88737`)
- **test**: case 3110 stops when it cannot enter its temp dir (I1031) (`159db73a4`)
- **bench**: remove a cold sample's repository by the mktemp path it lives under (`65e961cd8`)
- **test**: case 3066 reads the mesh state until it answers (`3d90e0920`)
- **worktree**: a new worktree starts from the remote trunk (`6131bff0f`)
- **start**: a scope entry is one path, and what context refuses is refused at start (`71474ebac`)
- **mesh**: a task is claimed on the mesh when it starts (I2288) (`486de96de`)
- **bench**: a timing verdict names its runner, and one cold sample is not a regression (I2068) (`507e38c80`)

### Documentation

- **plan**: the shared build directory's crate artifacts are bounded (I2787) (`cc5362986`)
- **adr**: renumber the adoption-footprint ADR to 0169 (I1101) (`b0064423d`)
- **rules**: the cooperation rule names its enforcement points by their pull requests (`a5f1e0e4a`)
- **consolidation**: what an adoption leaves behind, measured (I1101) (`e1997844a`)
- **adr**: the owner's cooperation model is one rule, read by the site and the UI (ADR 0167) (`7adfcc3d1`)

### Housekeeping

- **release**: derive the 0.29.0 bump over batch 71a (`0a19b7fe3`)
- **release**: bump the version to 0.29.0 (`6e1e527df`)
- **release**: record v0.28.0 and the metadata it publishes (`0c8016a43`)
- **plan**: I1101 evidence recorded, issue in VERIFY (I1101) (`e3b0e747a`)
- **derive**: the plan page of I2760 (`e81e622bd`)

### Published

- `macos-aarch64` — majordomus-v0.29.0-aarch64-apple-darwin.tar.gz (`c0f257a83447`)
- `macos-x86_64` — majordomus-v0.29.0-x86_64-apple-darwin.tar.gz (`1dfe8ba83f0b`)
- `linux-x86_64-gnu` — majordomus-v0.29.0-x86_64-unknown-linux-gnu.tar.gz (`6bf87c46d4f7`)
- `linux-x86_64-musl` — majordomus-v0.29.0-x86_64-unknown-linux-musl.tar.gz (`837304bbecc3`)
- `linux-aarch64-gnu` — majordomus-v0.29.0-aarch64-unknown-linux-gnu.tar.gz (`869ee0afe991`)
- `linux-aarch64-musl` — majordomus-v0.29.0-aarch64-unknown-linux-musl.tar.gz (`05734268f0e9`)


## v0.28.0 — 2026-10-17

### Added

- **context**: a worker is given a bounded slice of knowledge, not the knowledge base (`c598adf8b`)
- **knowledge**: the graph is validated, and doctor and watch ask the same validator (I0812) (`c5aec68c9`)
- **editors**: every domain of the registry is a skill, shown wherever skills are (ADR 0165) (`4e74a42a6`)
- **session**: a branch episode carries the trunk's outbox into the tracked sections (I1984) (`6edc0e6a6`)
- **session**: a trunk episode writes its records to the outbox (I1984) (`a5c3ee4c4`)
- **rules**: a standing approval is written where the permission layer reads it (`3774d1ed0`)

### Fixed

- **ci**: the rust job's case gates drive the executable the job built (`441c96554`)
- **top**: the last three changed items of top 70 are covered (`bd7f0af86`)
- **top**: top 70 covers its changed code, its rust lane fits, its palette waits (`a259f94df`)
- **top**: the essay links limitations relatively and plan-record-cases runs every plan reader (`735266fe5`)
- **site**: the plan diagram is drawn past Mermaid's default size bound (`611a9a85b`)
- **test**: the stale-executable readiness test waits out a busy copy (`e559ee54e`)
- **top**: the members of top 70 meet on the CI wiring, the port and the site audit (`078ae6911`)
- **site**: the skills table on the skills page takes keyboard focus (`d4cbf8924`)
- **top**: the members of top 70 meet where they first meet (`e97f034bd`)
- **rules**: the vendored candidates-reviewed rule dates no incident it does not attribute (`8f8825642`)
- **site**: the landing page states no present it does not read (I2745) (`bad02f37e`)
- **reap**: the reaper asks cargo where the shared build directory is (`7ff6be98e`)
- **docs**: every published tree draws its diagrams (I2746) (`337b41ee7`)
- **release**: the record pull request's dispatched run is not its verdict (I2282) (`8ed40f6eb`)
- **site**: a diagram's node text is readable in both themes (`3024cbcc0`)
- **test**: case 3066 names the step that ended it (`00fc6a841`)
- **mesh**: a review is redacted before it is signed (`3c3372d06`)
- **serve**: the idle timeout is declared beside the other server timings (`7111f846b`)
- **derive**: a fixture without history gets a verdict on its derived data (`3b5833692`)
- **logging**: every tracing field passes the one redactor (`dd4798ce5`)
- **mesh**: an EventBody field nobody listed fails the mesh check (`e2d7042a4`)
- **doctor**: doctor says it checks governance only and names where the runtime is read (`1ed658183`)
- **ci**: a plan-record change plans only the gates that read a plan record (`4df3ca5e5`)
- **site**: a leak finding names the file it is in, never the text (I1023) (`bdc75e2cf`)
- **site**: the essay links the limitations page by its address, not a content path (`ea7f82b50`)
- **graph**: the committed graph is whole or its generation refuses (`a2e22b484`)
- **serve**: a checkout keeps one address, its server asks for a port derived from it (`b3f35a34a`)
- **mesh**: an append the disk did not take is not acknowledged as durable (`06e28bb4d`)
- **scope**: a task's scope admits what its derive wrote (`ed8afb075`)
- **doctor**: a machine that derives has the pinned Rust toolchain (`52b0659d8`)
- **test**: a case does not inherit the interface the operator binds servers on (`aa6688ddc`)
- **ci**: the hook environment is dropped as the fixture fix drops it, keeping git -c settings (`4b05ff868`)
- **ci**: nothing a hook runs inherits the repository git hands the hook (`3c415b146`)
- **ci**: a push runs what CI would find first, and a case never waits on stdin (`8f7fb5bc5`)

### Performance

- **economics**: a bootstrap median counts its draws instead of sorting each resample (`43b8e9bac`)
- **build**: dependencies are optimised in the debug build every derive and hook runs (`fa8f00f6c`)
- **changelog**: every release's ancestry is listed by one git process (`7eee00861`)

### Documentation

- **cli**: linked_port carries an example of the port it derives (`52a884282`)
- **plan**: the plan's surfaces are inferred from delivery (ADR 0166) (`b80c18350`)
- **knowledge**: the words follow the trunk outbox (I1984 slice 3) (`b1a0bff60`)
- **plan**: a resumed mesh.events walk misses a partitioned writer's event (I2733) (`a64fdc1a4`)
- **adr**: a node key is rotated by a succession its predecessor signs (`a9ff7e1c3`)
- **adr**: a merge GitHub makes is guarded by checks, not by strict (I1409-c) (`89a44047b`)
- **runtime**: every failure class names its proof (`a53fd3b80`)
- **adr**: a durable record names the session that wrote it (ADR 0164, I0702) (`e59b86b66`)
- **adr**: the realization report names what is not orphan work (`6ee6b77c5`)
- **adr**: a server serves only a share of its own version (ADR 0163) (`cfff43810`)
- **adr**: a trunk episode stages its records and the pre-push judges the pushed branches (`a53f08f11`)
- **site**: a Czech essay says why the product is a layer and not another agent (`17fa2c4d0`)
- **adr**: state the disk and derive time without a typed percentage (ADR 0157) (`a28c54f27`)
- **adr**: the build-cache case is 3013, 3011 is another session's (ADR 0157) (`057e3d4f0`)
- **adr**: record what the first provider measured in ADR 0157 and I2673/I2674 (`7a749356c`)
- **adr**: a mesh node may offer a shared build cache, never depended on (ADR 0157) (`cdb52778d`)
- **adr**: the prompt-pack ADR is 0124, 0121 is the fleet ADR on master (`809c603e3`)
- **adr**: an uploaded prompt pack is a provenance-tracked input artifact (`a488c7405`)

### Tests

- **perf**: a repository of N records costs N parses, not N squared (I0836) (`c081c0b1a`)
- **sessions**: a second worker's record lands on the first worker's task (I0701) (`84d174c23`)
- **derive**: a committed derivation survives a merge that touches none of its inputs (`91b5fdf8e`)
- **http**: a rebinding host, the API page and the site's JSON are proven, and I2151 is done (`d5f1289e9`)

### Housekeeping

- **plan**: I2745 carries its evidence and reads verify (I2745) (`54cf30f57`)
- **plan**: I2746 records its diagram-scope evidence and is verified (I2746) (`3a8ffe184`)
- **release**: bump the version to 0.28.0 (`4a6abf2a0`)
- **derive**: the derived data follows the merge of master d116266 (`bad1720c3`)
- **derive**: the derived data follows the merge of master (`275f35597`)
- **derive**: the derived data follows the merge of master d116266 (`d65fd8cca`)
- **release**: record v0.27.0 and the metadata it publishes (`a2aa25f36`)
- **plan**: I2123 is done, a caller beyond loopback changes nothing it cannot authenticate (`dc3c36550`)
- **plan**: I2124 is recorded done with the tests that prove it (`43933145b`)
- **plan**: I1409 records fixture_without_history_evaluates from case 3055 (`9910ad089`)
- **plan**: I2128 is recorded done with the tests that prove it (`2caa75bee`)
- **plan**: failure-is-visible records two of its three evidence tokens (`4056f7302`)
- **plan**: I2135 is done; a committed tofu declaration fails the mesh check (`70e203fd2`)
- **derive**: the changelog follows the merge's whole history (`311e092db`)
- **plan**: an observation becomes a grouped, routed candidate is recorded done (`88d1e8075`)
- **plan**: I2134 is done, and its evidence names the tests that prove it (`db2d2d31b`)
- **derive**: the projections follow master c0e3585dc8 (`618f825f8`)
- **derive**: the projections follow master 97c4521f91 (`fa31bd31a`)
- **derive**: the projections of the Czech essay over master a6b1e1032 (`74cd022cd`)
- **sessions**: the record of the episode that opened this change, and its projections (`b406a1e53`)
- **derive**: regenerate after merging master and renumbering the build-cache case (`9dd87294f`)
- **offload**: the iMac is a third derive host (`a3a86d364`)
- **plan**: I2414 names the files and the measurements its implementation found (`4a6f9976e`)
- **plan**: the mesh works out of the box, and its six missing pieces are issues (`e531f1fe9`)

### Formatting

- **test**: cargo fmt over the derived-port assertion in cli_docs (`f37efd935`)

### Published

- `macos-aarch64` — majordomus-v0.28.0-aarch64-apple-darwin.tar.gz (`48e91654391e`)
- `macos-x86_64` — majordomus-v0.28.0-x86_64-apple-darwin.tar.gz (`f3e1f0e686a2`)
- `linux-x86_64-gnu` — majordomus-v0.28.0-x86_64-unknown-linux-gnu.tar.gz (`52e8c51339c0`)
- `linux-x86_64-musl` — majordomus-v0.28.0-x86_64-unknown-linux-musl.tar.gz (`87e69f3184eb`)
- `linux-aarch64-gnu` — majordomus-v0.28.0-aarch64-unknown-linux-gnu.tar.gz (`b9b600487e2d`)
- `linux-aarch64-musl` — majordomus-v0.28.0-aarch64-unknown-linux-musl.tar.gz (`103134918ff2`)


## v0.27.0 — 2026-10-16

### Added

- **mesh**: an edit no claim covers is refused (I2735) (`ed994d5d0`)
- **mesh**: a task built off the mesh is refused (`23b7f645e`)
- **mesh**: a task's scope is claimed on the mesh while it works (`8bb8a2238`)
- **capture**: a session joins the mesh at its start and leaves it at its end (`e81e054a5`)
- **capability**: the sandboxed engine a scripted capability runs in (I1800, ADR 0069) (`7ac2fde4a`)
- **knowledge**: a lookup answers in an order it can explain, and asking writes nothing (`f36a4c554`)
- **worktree**: work a session left uncommitted in a worktree is named (`dfbbc29db`)
- **ci**: a gap the audit calls planned names an issue that exists (`9f8c41b68`)
- **executions**: a run started from the command line is seen by the server (`2be92697c`)
- **ui**: two more kiosk scenes, and the snapshot carries the scenes and p5 (ADR 0160, I2697) (`75db37541`)
- **ui**: the kiosk draws what the repository proves, with p5 where it is built (ADR 0160) (`1ed8a4c87`)
- **site**: a diagram is interactive and linkable, a table draws as a chart (`bbb94fc2d`)
- **site**: the landing page speaks to a buyer, and holds on every device (`239df858c`)
- **providers**: Gemini and Antigravity read the repository through the projection (`60e97fb0c`)
- **site**: hand-written copy promises only what a claim permits (`2e26a3d36`)

### Fixed

- **mesh**: the edit guard passes the structure gates (`93063e327`)
- **test**: case 3067 names its commands on one covers header (`c6b42cf6d`)
- **top**: case 1096 counts a shared reading as a reading (`3c12372e3`)
- **mesh**: the task record declares the claim it carries (`70e60d531`)
- **plan**: the milestone is the bytes its first branches carry (`3acf2cf68`)
- **top**: the grand top's new code is covered and the command screen is within its budget (`ee944b18b`)
- **ci**: an integration top runs to the end and shows every red (`2c56a6fb8`)
- **test**: case 532 reads a table row up to its own </tr> (`8080ea219`)
- **top**: the crate boundary is told about rhai (`a0dcdeaaa`)
- **top**: the launcher's help reader takes no pipe, and I2718 names no home directory (`51faa7a5c`)
- **doctor**: no linked worktree reads the same whether or not the executable is at hand (`4bf88bed4`)
- **site**: the grand top carries its members' authored site data (`692d067f0`)
- **ci**: the release path never waits behind pull-request validates (`ba11bd6bf`)
- **site**: a surface card's title fits a 320px screen (`5f4964a3b`)
- **ui**: the Observatory asks again when it is seen (`11567a0b0`)
- **serve**: ensure starts its server with the share it was given (`4dada2c87`)
- **release**: a release names its archive from the matrix, without a debug build (`c2fa59702`)
- **handbook**: a copied link names the artifact the handbook is published at (`f56d54cbf`)
- **executions**: the reported store shows itself in examples, and the case waits bounded (`2dec71329`)
- **evidence**: the launcher hands evidence subcommands to the executable (I2601) (`c4a8f9c56`)
- **init**: a personal layer is seeded with a policy update can write (`f83ff2c7a`)
- **docs**: no heading of the handbook skips a level (`962472c96`)
- **events**: each execution's cursor is set as its replay is collected (`e9ee93dd5`)
- **site**: a missing copy file is a finding of promises, not the end of homepage-check (`367e4f86b`)
- **mesh**: a borrowed server publishes no handover of another checkout (`ad249172c`)
- **http**: keep a live-event cursor per execution (`b7b675cf7`)
- **mesh**: a linked worktree cooperates through its repository's running server (`e8235eece`)
- **mesh**: a board intent past the journal bound is projected once (`d4ce8f995`)
- **mesh**: a retired server leaves the mesh (`f0a739905`)

### Performance

- **ui**: a screen reads the answer its source shares instead of copying it (`43204ad13`)

### Documentation

- **adr**: a capability may be scripted, and the automation milestone is on master (ADR 0069) (`fbb9aaf47`)
- **plan**: four of the day's defects are planned so their carriers can close them (`b1e0d8d12`)
- **adr**: what the Observatory is told arrives on /events, the one live channel (ADR 0158) (`2f759351d`)
- **convergence**: the baseline of the 10-14 pack is measured against master e8505a431d (`5b0e28630`)
- **adr**: a mesh node follows the published release (ADR 0161, safe-self-update) (`5addc860b`)

### Tests

- **mesh**: the edit guard owes its examples and its coverage (`2a90f49ab`)
- **coverage**: the grand top's new Rust code is covered by a test (`2396be3f4`)

### Pipeline

- **pages**: the UI snapshot job builds p5, so the published kiosk draws its scenes (I2697) (`f4cda1016`)

### Housekeeping

- **derive**: the derived data follows the merged case header fix (`0a6e6c127`)
- **derive**: the derived data follows the merged task record schema (`92d335db2`)
- **derive**: the derived data follows the merged milestone (`417eab2ce`)
- **derive**: the grand top's derived documents follow its canonical inputs (`a3ba14a7e`)
- **plan**: I2127 and I2163 are recorded done with the tests that prove them (`540b3ca5c`)
- **derive**: the derived data follows the merged tree (`1b82abaf6`)
- **plan**: the MCP transport bounds are recorded done with master's own evidence (`0c0a7211f`)
- **derive**: the projections of the landing page on master c0e3585dc83 (`8340a04bf`)
- **derive**: the derived data follows master c0e3585dc8 into I1403's record (`fa049026e`)
- **derive**: the projections follow master e8505a431d (`8e180b845`)
- **derive**: the derived data follows master e8505a431d (`7e1f2ac80`)
- **derive**: the derived data follows master e8505a431d into I0806's record (`289cbbe4d`)
- **plan**: I1403 is done — the GitHub projection has no backlog, and the ratchet says so (`4f3c9bddd`)
- **plan**: I0806 is done — master already holds the session rules (`266d5af14`)
- **campaigns**: the convergence, trust and product proof pack of 10-14 is tracked (`af070b47f`)
- **release**: record v0.26.0 and the metadata it publishes (`f937ac387`)
- **derive**: the derived data follows the merge of master 97c4521f91 (`a20d963d9`)
- **derive**: the derived data follows the merge of master (`a71bccb80`)
- **derive**: the derived data follows the merge of master ff38297096 (`927f2f7e4`)
- **derive**: the derived data follows master ff38297096 and the I1600 record (`ca4401900`)
- **derive**: the projections follow master ff38297096 (`bf8179aa8`)
- **derive**: the projections of the landing page on the master of its landing (`bce111701`)
- **plan**: I2601 is in VERIFY, and the derived plan pages follow (I2601) (`6f4a83658`)
- **derive**: the projections of the landing page on the master of its landing (`5a679909e`)
- **derive**: the derived data follows master e506c041a4 (`783e7cf70`)
- **derive**: the derived data follows master e506c041a4 (`aa0395953`)
- **plan**: the public-proof milestone is done, on the evidence of I1600 (`9ee0a66a7`)
- **plan**: I1600 is done, the challenge master already replays (`de14385dd`)

### Formatting

- **mesh**: rustfmt the edit guard; the derived data follows the merged stack (`04986858e`)

### Published

- `macos-aarch64` — majordomus-v0.27.0-aarch64-apple-darwin.tar.gz (`283ce45eb568`)
- `macos-x86_64` — majordomus-v0.27.0-x86_64-apple-darwin.tar.gz (`178bb10c8fab`)
- `linux-x86_64-gnu` — majordomus-v0.27.0-x86_64-unknown-linux-gnu.tar.gz (`dd5cd97cfc51`)
- `linux-x86_64-musl` — majordomus-v0.27.0-x86_64-unknown-linux-musl.tar.gz (`1679e7eaa6b0`)
- `linux-aarch64-gnu` — majordomus-v0.27.0-aarch64-unknown-linux-gnu.tar.gz (`64a0114471e0`)
- `linux-aarch64-musl` — majordomus-v0.27.0-aarch64-unknown-linux-musl.tar.gz (`c3d8d14d28c6`)


## v0.26.0 — 2026-10-14

### Added

- **adopters**: an adopter gets only what fits it, and a failed branch run ends itself (`a47c70939`)
- **ui**: the Observatory's gates are closed with evidence, and new events interrupt its rotation (`d7a4361bc`)

### Fixed

- **test**: case 130 compares surfaces at a budget the seeded governance fits (`d4d23c378`)
- **release**: a tag no longer freezes the queue, and two adopter records hold (`96cff105c`)
- **adopters**: the catalogue drops doctor's adopter reasoning line; case 3018 asks for no pager (`d9dea083b`)
- **runtime**: a closed live channel stops keeping its server, and eight issues are proven (`3e848ca4d`)
- **mesh**: a node key is owner-only from the moment it is written (`37c72bbed`)

### Housekeeping

- **release**: 0.26.0, the contract the knowledge reference answer changed (`83d4afbff`)
- **release**: record v0.25.0 and the metadata it publishes (`0f0c35eb7`)
- **observatory**: the gates' production evidence is the published site at 2ca25bb9cf (`63ae8e0c2`)
- **plan**: the Observatory's milestones are done with their evidence (`c3da677d4`)
- **observatory**: the published replay is the mesh as recorded at 11:39Z (`753449f90`)
- **plan**: nine issues stop requiring what they deferred, and I2137 is done (`304469c84`)

### Published

- `macos-aarch64` — majordomus-v0.26.0-aarch64-apple-darwin.tar.gz (`b503fa119f32`)
- `macos-x86_64` — majordomus-v0.26.0-x86_64-apple-darwin.tar.gz (`5660a987bb0a`)
- `linux-x86_64-gnu` — majordomus-v0.26.0-x86_64-unknown-linux-gnu.tar.gz (`54f911fa0b69`)
- `linux-x86_64-musl` — majordomus-v0.26.0-x86_64-unknown-linux-musl.tar.gz (`6de4ec37191e`)
- `linux-aarch64-gnu` — majordomus-v0.26.0-aarch64-unknown-linux-gnu.tar.gz (`89633a013bf8`)
- `linux-aarch64-musl` — majordomus-v0.26.0-aarch64-unknown-linux-musl.tar.gz (`1fa2dfd6b2bc`)


## v0.25.0 — 2026-10-14

### Added

- **ui**: the Observatory follows the journal from a cursor, and every new tool is called (`bb844110b`)
- **ui**: the published Observatory replays a recording of the real mesh, and says it is a replay (`f58b8a1c6`)
- **editors**: Claude Code reaches every workflow skill by its name, and evolve says what it learned (`9709eea84`)

### Fixed

- **plan**: I1900 is ready again until the intent it serves has a critique (`f044ae642`)
- **plan**: an open issue scopes the test directory, not a case it has not written yet (`70b4e540c`)

### Documentation

- **handbook**: the Czech handbook says what the MJ way is, and states no figure by hand (`e217df9d6`)

### Housekeeping

- **plan**: the deployment milestone is the GHCR image, and eight Fly-only issues are cancelled (`3b787430c`)
- **release**: 0.25.0, for the Observatory's three new capabilities (`2b9f58e93`)
- **plan**: 26 issues master already delivers are done with their evidence, and I0924 is cancelled (`f29cb6ac2`)
- **release**: record v0.24.1 and the metadata it publishes (`b8f6f3ec3`)

### Published

- `macos-aarch64` — majordomus-v0.25.0-aarch64-apple-darwin.tar.gz (`315a5f7824e6`)
- `macos-x86_64` — majordomus-v0.25.0-x86_64-apple-darwin.tar.gz (`c4de17255b78`)
- `linux-x86_64-gnu` — majordomus-v0.25.0-x86_64-unknown-linux-gnu.tar.gz (`edb3a128a163`)
- `linux-x86_64-musl` — majordomus-v0.25.0-x86_64-unknown-linux-musl.tar.gz (`34416d74a83d`)
- `linux-aarch64-gnu` — majordomus-v0.25.0-aarch64-unknown-linux-gnu.tar.gz (`1c34018c1250`)
- `linux-aarch64-musl` — majordomus-v0.25.0-aarch64-unknown-linux-musl.tar.gz (`9185d67e2b07`)


## v0.24.1 — 2026-10-14

### Added

- **product**: the Observatory is a feature with a claim a reader can check (`3d8da99be`)
- **ui**: the Observatory draws the repository and its mesh from what the sources answered (`6e06a86c6`)
- **skills**: evolve is a skill that composes the knowledge commands, and the version is 0.23.1 (`a9d7b2115`)
- **ui**: the UI shows what the knowledge base holds (I2652) (`6b46a80f5`)
- **rules**: an expedited landing is the owner's word and never reaches a tag (`a383e44b3`)
- **pages**: the Cockpit is published as a snapshot (I2495, ADR 0147) (`46bdc20b6`)
- **session**: session records are capped by policy and archived on request (`ebe15d2e4`)

### Fixed

- **ui**: the Observatory's projection carries no link into another surface (`ce94eb767`)
- **ui**: every name of the Observatory answers on the published site, and the homepage shows it (`e37877639`)
- **test**: case 3011 finds the snapshot's observatory page by its path inside the snapshot (`9d31aa3a7`)
- **ui**: the Observatory takes every colour from the tokens and numbers no event it cannot identify (`371100487`)
- **test**: case 401 reads the build the bounded launcher asks cargo for (`65d86ff18`)
- **ui**: /ui/screensaver is the kiosk under its earlier name (`689fc2555`)
- **test**: the evolve skill is named by the two tests that prove every active skill (`bd82ee4e9`)
- **ui**: the first screen of the published UI leads only with decided figures (`0a926184a`)
- **build**: a build never waits in silence behind another build's lock (`e7a37c636`)
- **ci**: a record a head adds is pending, not backlog (`308857ded`)
- **pages**: the surface name is braced in the pattern pages-check builds (`d77991a7c`)
- **pages**: a snapshot is asked of the publication that declared it (`5fa227b68`)

### Documentation

- **plan**: the delivery latency of 2026-10-13 is recorded as a post-mortem and four issues (`51021b88c`)
- **rules**: between a tag and its record nothing else lands (`48cd7a07e`)
- **handbook**: the Czech handbook says how session records are capped and archived (`f4c90e2ee`)

### Housekeeping

- **derive**: the derived data follows master ee84e49b18 (`44f3d84d2`)
- **derive**: the derived data follows master a6b1e10328 (`e7c83bbb4`)
- **derive**: the derived data follows master 2ee40c5aa1 (`47aaa49d4`)
- **derive**: the projections of the session cap and 0.24.0 on the master of its landing (`507b1724d`)
- **derive**: the projections of the session cap and 0.24.0 on the master of its landing (`e363d523d`)
- **derive**: the projections of the session cap and 0.24.0 on the master of its landing (`0b28ece25`)
- **derive**: the projections of the session cap and 0.24.0 on master beb60f5f96 (`8783d8371`)
- **plan**: four delivered UI issues record their evidence and completion (`8a29993b6`)
- **derive**: the projections of the session cap and 0.24.0 on master 5bfee30eb1 (`d3b69169c`)
- **release**: record v0.23.0 and the metadata it publishes (`2d933d314`)
- **release**: 0.24.0, with the projections of the session cap on master 3442ce68c5 (`100c41bbb`)
- **derive**: the projections of the session cap on master e506c041a4 (`cbe421c38`)
- **derive**: the derived data follows the merge of master e506c041a4 (`9c56e7124`)

### Published

- `macos-aarch64` — majordomus-v0.24.1-aarch64-apple-darwin.tar.gz (`4e6371d0760f`)
- `macos-x86_64` — majordomus-v0.24.1-x86_64-apple-darwin.tar.gz (`36322c978c1c`)
- `linux-x86_64-gnu` — majordomus-v0.24.1-x86_64-unknown-linux-gnu.tar.gz (`5e6ff82feadd`)
- `linux-x86_64-musl` — majordomus-v0.24.1-x86_64-unknown-linux-musl.tar.gz (`263ac4ef71ce`)
- `linux-aarch64-gnu` — majordomus-v0.24.1-aarch64-unknown-linux-gnu.tar.gz (`7ab71d5c08e2`)
- `linux-aarch64-musl` — majordomus-v0.24.1-aarch64-unknown-linux-musl.tar.gz (`45291456055f`)


## v0.23.0 — 2026-10-13

### Decisions

- **ADR-0146** A capability is invoked by name through a generated skill _(accepted)_

### Added

- **editors**: a capability is invoked by name from Claude Code, through a generated skill (`37067ec23`)
- **site**: the published UI is linked where the publication holds it (`8c9082664`)
- **site**: the landing answers five questions, and the navigation is rendered once (`6c58b9974`)
- **ci**: the GitHub projection is applied by the landing itself (`cf90345b7`)
- **providers**: preserve owner-authorized SSH permissions (`eb76b216d`)

### Fixed

- **ci**: the projection workflow's guards are if-statements (`8d852d79f`)
- **test**: case 1097's message no longer reads as an early exit (`93c9c2189`)
- **test**: case 486 asserts what the site's check says today (`ba49c586c`)
- **rules**: a rule cites an external page without linking it (`b7ef193ab`)
- **http**: a stopped server refuses the next connection (`a178e1b2b`)
- **docs**: the handbook build writes nothing into the tree it reads (`3b3ba8e77`)
- **site**: Escape gives the focus back, and the hero's label wraps on a phone (`052b4ae49`)
- **hooks**: a push that changes the crate builds its rustdoc with warnings denied first (`19a2f94f7`)
- **docs**: the doc of a public function links no private constant (`115d67a39`)
- **tmp**: a temporary root is swept only when its process is gone, never when it cannot be asked (`57ce0ccf6`)
- **site**: a bar link to a snapshot's root is decided by the publication model (`b90699421`)
- **rules**: one malformed rule file is one finding, not fifty-seven (`a28ab7032`)
- **github**: a listing that failed is not an empty remote (`941ec686f`)
- **docs**: the six commands that joined the doc-command baseline leave it (#720) (`bbbc44a4f`)
- **doctor**: a JSON parser that cannot run is not read as the file's failure (`83fdc2932`)
- **ci**: a lane's own output is not a dirty tree (`f0911e679`)
- **live**: a failed reload consumes no write either (I1992) (`32d5b488a`)
- **live**: a rebuild that fails consumes no change (I1992) (`260da7bf2`)

### Performance

- **ui**: a screen over its budget fails the build (I2489) (`664ebe4cf`)

### Documentation

- **plan**: I1992, a rebuild that fails consumes no change (`2adf44450`)

### Housekeeping

- **derive**: the derived data follows master 62d6ca2e45 (`49f201108`)
- **derive**: the derived data follows master 73d3c8cc31 (`71b3060cc`)
- **derive**: the derived data follows master 9e50ce2af3 (`9d3ef5c78`)
- **derive**: the derived data follows the merge of master 9e50ce2af3 (`9e2ed1041`)
- **derive**: the derived data follows the merge of master b34259715e (`7c8b77de2`)
- **release**: record v0.22.0 and the metadata it publishes (`c3ebd75b8`)
- **derive**: the derived data follows the merge of master d1653776cc (`e28de1d22`)
- **merge**: the handbook's cache files are master's bytes (`917f35a97`)
- **derive**: the derived data follows the merge of master 23c5c31be3 (`bc6d8b8aa`)
- **merge**: master 3d92c25c12 into the branch, derived files follow (`b09367dda`)
- **derive**: the derived data follows the v0.18.0 record (`18cb398ce`)
- **derive**: the derived data follows the v0.18.0 record (`9f513a514`)
- **derive**: the derived data follows the v0.18.0 record (`6c3325eb9`)
- **plan**: close the eight issues master already holds, and re-measure four (`a8a7830a4`)

### Published

- `macos-aarch64` — majordomus-v0.23.0-aarch64-apple-darwin.tar.gz (`237ded653955`)
- `macos-x86_64` — majordomus-v0.23.0-x86_64-apple-darwin.tar.gz (`b8dfb703363a`)
- `linux-x86_64-gnu` — majordomus-v0.23.0-x86_64-unknown-linux-gnu.tar.gz (`b09b4716c76e`)
- `linux-x86_64-musl` — majordomus-v0.23.0-x86_64-unknown-linux-musl.tar.gz (`f36a18a751fd`)
- `linux-aarch64-gnu` — majordomus-v0.23.0-aarch64-unknown-linux-gnu.tar.gz (`0e090873567c`)
- `linux-aarch64-musl` — majordomus-v0.23.0-aarch64-unknown-linux-musl.tar.gz (`7e251296b6b3`)


## v0.22.0 — 2026-10-13

### Decisions

- **ADR-0149** A handbook edition is a declared locale tree, built from the tree it describes _(proposed)_
- **ADR-0145** A personal layer changes nothing shared: it is covered by git's own exclude file, checked where it is claimed, and handed to the team by one command _(proposed)_
- **ADR-0144** An intent is proposed by a capability, and its stage stays derived _(proposed)_
- **ADR-0142** No screen waits for a source: a page shows the latest reading with its age, and the reading catches up behind it _(proposed)_
- **ADR-0141** The UI is published as a snapshot read at one commit, and a publication never pretends to be a process _(proposed)_
- **ADR-0140** Editors and agents read projections of the layer, and cargo embeds none of it _(proposed)_

### Added

- **docs**: the handbook is built from the tree it describes (ADR 0149) (`fc7917367`)
- **install**: personal mode is a switch — init --personal, verified, and --shared (ADR 0145) (`3bf63db8e`)
- **intent**: an intent is proposed by a capability, and its stage stays derived (`ee7f3373c`)
- **ui**: every entity the UI names is a link, and every screen has a case (I2481, I2482) (`995496f6f`)
- **ui**: the snapshot is bounded, declared, documented and wired into Pages (I2483, ADR 0141) (`41cc4dddd`)
- **ui**: the UI links what it names and says none (I2481, work in progress) (`64dca8303`)
- **ui**: the UI is read once and written as a static snapshot (`6f55316bc`)
- **editors**: Zed and Antigravity read projections of the layer (ADR 0140) (`17e99c068`)
- **mesh**: a review request carries a body, is withdrawn by its asker, and ends with it (`76dc4b399`)

### Fixed

- **test**: the deployments reading is asked for by served forge, and the test says so (`e6d0371cf`)
- **test**: the served doctest names served.forge, and the handbook page is a whole document (`c03a5ff76`)
- **served**: the module's documentation names the three capabilities it has (`ebf01655f`)
- **docs**: the handbook wears the declared design, and its sources are chapters only (`ab498d5c0`)
- **ci**: every red of the tag candidate's validate run is paid at its cause (`3e8225a43`)
- **test**: case 2979 looks for a session record without ls | grep (`54263f8ba`)
- **ui**: an absent deployment count reads none, and the stub forges answer the deployments read (`897bc430a`)
- **ci**: master's seven reds after the backlog landings are paid in one commit (`5631447ff`)
- **site**: a Markdown table is aligned by a class, so one column cannot stop a publication (`3c48889f8`)
- **quality**: the readings carry their examples (`c40dbb86a`)
- **ci**: top 2 pins a sort to the C locale and classifies web report ui-snapshot (`6cb8009ad`)
- **ci**: the landing top's members meet — what the first combined run found (`102a08dd4`)
- **site**: every badge states the one offer, and no surface lists release records (`5dab8737b`)
- **site**: pages-check finds the repository for release-live, and case 180 holds it (`d38d3bc56`)
- **site**: the site offers the release that was published, on every surface at once (`1c15652c0`)
- **editors**: the projection meets the structure gates it added a module to (`19b6416d4`)
- **editors**: rustdoc reads no path placeholder as an HTML tag (`7cf2dfb56`)
- **http**: the live stream misses nothing a client can be told about (`000e2b047`)
- **build**: each worktree builds its own crate in the shared build directory (`a29738743`)
- **scope**: a path that holds a space is one path (I2464) (`dea8e07a7`)
- **mesh**: a review request is withdrawn and expires with its runtime, not its session (`3daef42ca`)
- **mesh**: a clone on a machine its trust list does not name opens no mesh socket (`9e1d9cc37`)

### Performance

- **ui**: no screen waits for a source (I2486, ADR 0142) (`344cb306c`)
- **ci**: a release record and its projections plan a light validate (`22de3c496`)
- **capability**: the slow screens read once (`99ea09509`)
- **bench**: the bench measures the UI and the live view (`2fcb023e5`)
- **cockpit**: assets are cached by their digest (`66bc00bd2`)
- **integration**: the queue reads its relations once (`a865e3207`)
- **ci**: the projection renders every record in one process, adr check resolves rules once (`89db623dc`)
- **test**: derivation cases generate site data without executing the scenarios (`26898026f`)
- **gates**: digest each file once per completion report (`9df08e7bd`)
- **doctor**: one python3 for every schema, one awk for every case header (`0221b4779`)
- **site**: page literals without a subshell each; modules checked in one jq (`141acab6d`)
- **site**: one jq per section for the page literals, one awk for case coverage (`202efbac8`)
- **ci**: the pipeline's data files are planned by their readers (`84de132df`)
- **ci**: shellcheck runs in batches over a bounded pool (`39b0587db`)
- **site**: invert the OpenAPI reference walks and drop jarr's per-line forks (`e5e4bcf36`)

### Documentation

- **adoption**: the arrangement is called personal mode, and its way to a shared layer is measured (`4b66ef5f5`)
- **adoption**: using the tool without leaving anything in git, measured (`98bee6156`)
- **ui**: the UI is the accepted primary surface, and its plan says what landed (`0e458b2c6`)

### Tests

- **editors**: `generate editors` is proven over a real repository, with `just` where coverage runs (`c46ff6c72`)

### Housekeeping

- **derive**: the derived data follows master 9d31c4d456 and the readings (`8d35e7165`)
- **deploy**: Fly.io is not a target; the provider block and fly.toml leave the contract (`9af3fa65a`)
- **derive**: the derived data follows the UI-performance batch with the slow screens (`c66f49e40`)
- **release**: record v0.21.0 and the metadata it publishes (`7bc7e662c`)
- **derive**: the derived data follows the UI-performance batch on v0.21.0 (`39d682a26`)
- **derive**: the derived data follows the v0.21.0 top and the per-tree build (`1906935a4`)
- **derive**: the derived data follows the remembered queue (`64352fc6a`)

### Published

- `macos-aarch64` — majordomus-v0.22.0-aarch64-apple-darwin.tar.gz (`bd4f69a863cc`)
- `macos-x86_64` — majordomus-v0.22.0-x86_64-apple-darwin.tar.gz (`aa02c341feb0`)
- `linux-x86_64-gnu` — majordomus-v0.22.0-x86_64-unknown-linux-gnu.tar.gz (`879d2aa20108`)
- `linux-x86_64-musl` — majordomus-v0.22.0-x86_64-unknown-linux-musl.tar.gz (`7ab2dd2fbe34`)
- `linux-aarch64-gnu` — majordomus-v0.22.0-aarch64-unknown-linux-gnu.tar.gz (`7051bc83be12`)
- `linux-aarch64-musl` — majordomus-v0.22.0-aarch64-unknown-linux-musl.tar.gz (`2c289bd004a0`)


## v0.21.0 — 2026-10-12

### Decisions

- **ADR-0139** Worktrees share what cargo compiles on the way, and keep their own finished artifacts _(proposed)_
- **ADR-0138** A train of pull requests is a typed object that can be seen _(proposed)_
- **ADR-0136** Every session briefs itself on an interval, from one capability a provider mod the policy switches keeps in front of it _(proposed)_
- **ADR-0135** `majordomus ui` is the primary surface, and the Cockpit is the technical layer beneath it _(accepted)_
- **ADR-0134** The next version is allocated at landing, by the integration executor, from the trunk's version and the change's measured impact under a typed policy floor _(proposed)_
- **ADR-0133** Everything that happens in the repository is followed in one stream, folded and addressed _(proposed)_
- **ADR-0131** A release publishes its metadata to the site before its record lands, and the site never goes back a release _(proposed)_
- **ADR-0129** A train of stacked pull requests lands in one run of the required check, each link on the verdict its head already has _(proposed)_
- **ADR-0126** A caller beyond loopback reads, and changes nothing it cannot authenticate _(proposed)_
- **ADR-0118** A session leaves what it observed, and the boundary keeps one routed candidate per defect _(proposed)_

### Added

- **build**: worktrees share what cargo compiles on the way (ADR 0139) (`c56b5b87c`)
- **ui**: a rule opens its proof, and the rules it depends on and that depend on it (I2478) (`ae556b894`)
- **ui**: an intent opens what serves it, its criteria and their evidence (I2476) (`5ba4b928a`)
- **ui**: the issues are browsed by status, milestone, priority and text (I2475) (`bc8ee345d`)
- **ui**: the drain draws its merge throughput over the fortnight its trail ends on (I2474) (`6a69100eb`)
- **ui**: a pull request shows its trail, and the drain the lane's recent activity (I2473) (`41008d555`)
- **integration**: how long CI takes is measured, and the CI/CD screen draws it (I2472) (`306cf3b05`)
- **ui**: the roadmap opens a milestone, its critical path and its neighbours (I2471) (`6c8c17a44`)
- **commands**: the effect class is declared per subcommand and flag (`378f93cc2`)
- **ui**: the pull requests drain in the UI, narrowed, inspected and followed (I2470) (`2df014b2d`)
- **integration**: a train of stacked pull requests is typed, served and drawn (`b60351428`)
- **monitor**: a machine is named, a withdrawn review is news, a merge is told once (`73855209f`)
- **offload**: a derive offloaded to a declared host waits its turn in a FIFO queue (`c1958f5d1`)
- **ui**: the UI has every screen, and the mesh is an inspectable graph (I2462) (`59a993753`)
- **integration**: a head stacked on a landed pull request merges on the verdict it has (`361fecf07`)
- **session**: every session briefs itself on an interval (I2399, ADR 0136) (`dd5a6336c`)
- **ui**: majordomus ui is the product, served at /ui (I2461, ADR 0135) (`9bcebb9f8`)
- **integration**: a head that contains master still owes its allocated version (`606d9c61a`)
- **site**: one search, opened from every page, over what the build published (`988aec549`)
- **integration**: the executor allocates the version where it refreshes (`99c3eafd5`)
- **site**: a number in prose is a number the repository states (`0824947bf`)
- **site**: the landing page is a click-through story (`76fb37abe`)
- **release**: the version a landing allocates is decided once, under a typed policy floor (`0e38ab5bf`)
- **monitor**: everything that happens is followed in one stream (`bf08180a4`)
- **mesh**: mesh.state and mesh.peers answer a summary that fits one tool answer (`1eb8d5fa8`)
- **release**: a release publishes its metadata before its record lands (`45efcfead`)
- **gates**: the closing question is answered by measurement (`303f8384f`)
- **integration**: a head stacked on a landed pull request merges on the verdict it has (`e97caa007`)
- **integration**: the executor reopens a record pull request no run was started for (I2296) (`44e2af08b`)
- **knowledge**: a session leaves what it observed, and one candidate per defect is routed (`4afbde69e`)
- **mesh**: the host firewall is a derived requirement with a verdict (`88fe975fc`)

### Fixed

- **test**: case 991's no-root probe defers its root, as case 1074's library now roots every script (`e95cbeed1`)
- **test**: the published pair is compared on the figures it cites, not only the ones it types (`217e1b501`)
- **ci**: case 1022 is renumbered 1086 and the mesh's test fixtures live in the mesh (`94ca56fe3`)
- **ci**: the interaction probe is bounded at 3300 s, the site it walks having grown (`d7b1c82d6`)
- **offload**: a refused apply no longer spoils the next offload of the same tree (`66600867a`)
- **deploy**: a stage line is asserted with contains, not a manual any (`f48fcbca9`)
- **cli**: two error paths no test can reach are folded into the answer (`57c27f784`)
- **ui**: a slow panel cannot take the browser's connections (`1a552beb5`)
- **integration**: a version conflict that cannot be resolved aborts the refresh merge (`7517f7c4a`)
- **deploy**: an image that builds, caches, starts, and reuses the release binary (`d62f7e258`)
- **test**: integration tests ignore the session's MAJORDOMUS_* environment (`f38533dba`)
- **reap-orphans**: the shared build directory is removed through $target (`3312a5a3a`)
- **deploy**: preserve generated artifact header contract (`045f576f5`)
- **release**: the record is staged without naming an ignored path (`8517b0a12`)
- **ci**: what #963's structure job and the ui audit found is paid (`24ab7f820`)
- **evidence**: a file moved since the run changed at its old path too (`0e97eae55`)
- **ci**: the coverage job's bound follows the crate it measures (`326a13a68`)
- **ui**: a busy panel keeps its contrast, and a long fact value wraps (`05fa7e561`)
- **ci**: the rest of what #957's run found below the top is paid (`c6c04bfd8`)
- **train**: what #957's run found below the top is paid in one commit (`79bb4e3c1`)
- **ui**: an unnumbered pull request is shown as unknown, not as #0 (`34d029bef`)
- **ui**: a milestone named by a slug opens, and the roadmap's tests ask the screen as it is (I2471) (`08333dc61`)
- **integration**: the trains command's doc renders under rustdoc (`aa1b399b1`)
- **ui**: a mesh session shows the claims the summary counts (`bbc547e6c`)
- **ui**: every screen renders in-process and passes the audit and design gates (I2461, I2462) (`d5580ae82`)
- **lib**: a script that sources the library leaves no temporary files (`bcc6347d5`)
- **mesh**: the mesh's own cases and lab ask for the full view (`1f9c8a0ec`)
- **ci**: the slowest-case budget is re-derived from 2026-10-11's runs (`841ed86e8`)
- **ui**: the UI is a described service and keeps its internals to itself (I2461, I2462) (`572494c22`)
- **ci**: the slowest-case budget follows the 4127 s measured under a saturated pool (`4fb1dbd16`)
- **release**: only the change that adds a release's record escapes the floor (`32999fa85`)
- **monitor**: the withdrawal rule takes no context it does not read (`7d1d467ad`)
- **release**: the record of the trunk's release lands at its number (`388353292`)
- **release**: the record is staged without naming an ignored path (`1ba11643c`)
- **integration**: carries_master owes a minor version, and its two branches are tested (`9993844bd`)
- **integration**: the tests leave no temporary directory beside the system's (`770b464bc`)
- **release**: the review of slice A, on the mesh: renames, empty changes, the floor's mode (`8e833d463`)
- **release**: a landing measures the movement from where the head forked (`c8c6a9205`)
- **ci**: the verdict asks every selected gate for a receipt (`f220f5e02`)
- **integration**: carries_master owes a minor version, and its two branches are tested (`6dde8d38e`)
- **gates**: every way a closing reading can fail is one a test reaches (`225f6ab38`)
- **ci**: a projected record edited by a head is pending, not refused (`a8c7c77ad`)
- **release**: the record is staged without naming an ignored path (`ca488f87a`)
- **plan**: I2123 names the files it touches (`4868514a4`)
- **mesh**: the nftables commands are ones nft accepts, and a command is ok by its exit (`47fc2d6a6`)
- **mesh**: a lab node commits its declaration, and the trust root's paths are tested (`11472da40`)
- **http**: a slow or silent client cannot hold the shared server (`ee9053efe`)
- **ci**: a bench check and a status probe are not verdicts on the runner (`56a337e1a`)
- **mesh**: no port is derived open to every source, and the firewall's code is tested (`e30630c99`)
- **mesh**: a sync message stays under the link bound whatever the marks weigh (`060311a90`)
- **mcp**: every MCP transport has a bound, and a session outlives its longest request (`9e018a302`)
- **ci**: the suite is dealt from measured seconds, and case 51 runs beside the others (`37b825efe`)
- **ci**: a registry hiccup and a slow sample are not verdicts (`71c3284e1`)
- **lease**: a newer client replaces an older server, and an older client says it is older (`383309ae0`)
- **integration**: unmirrored reads git's list without a branch nothing reaches (`4189fa949`)
- **release**: an installed release serves a foreign repository (I2162) (`5f654cae6`)
- **http**: a server busy with slow calls still answers the lease probe (`3be5cbbda`)
- **serve**: a server someone is using is not idle, whatever the transport (`e80c336ba`)
- **serve**: a superseded server hands its sessions on and ends (`5bb27f421`)
- **serve**: the declared lease timings are the ones an election uses (`59dbfd1c0`)
- **mesh**: a key withdrawn from the trust list loses its links without a restart (`9d09a4a5f`)
- **mesh**: every mesh input has a bound, and a registration learns only its own trusted peers (`a0553dd19`)
- **mesh**: paging mesh.events returns every event exactly once (`f820f5ab0`)
- **http**: no answer, log line or handover carries a credential the redactor knows (`adb513dce`)
- **mesh**: a hello names the runtime it is meant for, and no other runtime accepts it (`aa28332ca`)
- **health**: ready means ready, and the mesh is a dimension of health (`721e9a917`)
- **http**: the API page runs only what it ships, and a rebinding host reads nothing (`21ea43629`)
- **http**: the writer list and the doc examples follow executions.start (`bde38800c`)
- **mesh**: an accepted event is on disk before it counts, and a torn journal line is reported (`bb63c9439`)
- **mesh**: a working tree may narrow the mesh's trust and never widen it (`4816d7c8a`)
- **serve**: a lease is removed only by the process that holds it, and serve stop stops in order (`632acc00d`)
- **mcp**: a tool call that may have run is never sent a second time (`7f956339a`)
- **mesh**: an event reloaded from disk meets the trust policy in force (`477f417d9`)
- **mesh**: the MacBook hubs are named by their tailnet address (`a9cf1f295`)
- **http**: a caller beyond loopback reads, and changes nothing it cannot authenticate (`8d6e0c76e`)
- **integration**: a missing pull ref is one unknown head, not a failed refresh (`fe13037d1`)

### Performance

- **docker**: materialize generated cached multistage image (`4069a90b7`)
- **deploy**: derive BuildKit dependency and local compiler cache stages (`4a221398f`)
- **release**: the changelog asks git when each decision was added side by side (`65429c638`)
- **dashboard**: the overview asks its sources at once (I2460) (`51b8def04`)

### Documentation

- **rustdoc**: the batch documents with -D warnings (`58b31d489`)
- **ui**: the pull-request detail is documented and accepted (I2462) (`8d34e1f7a`)
- **campaigns**: the living-site V2 pack is measured against its premise (`6d5254e96`)
- **mesh**: every public item of the summary view carries an example (`642a4563a`)
- **ci**: the critical path of 2026-10-10 is measured and recorded (`705890da2`)
- **plan**: I2282, the record pull request's dispatched verdict does not satisfy branch protection (`fdcdb3916`)

### Tests

- **coverage**: the lease's lock warning and the test network's refusal are covered (`d765ef1d5`)
- **monitor**: the monitor, the lease and their neighbours are covered to the last line (`dbbcb1228`)
- **ui**: the drain, timings and train views are covered to the last line (`c4686c286`)
- **release**: the allocation, the landing and the prs commands are covered to the last line (`a706d00ee`)
- **mesh**: every changed line of the mesh command and runtime is covered (`0c65eedf6`)
- **integration**: the drain, the relation and their neighbours are covered (`f4283b34b`)
- **runtime**: the monitor, the lease, the server and the MCP transports are covered (`0963cda5b`)
- **deploy**: enforce cache stages and canonical Rust toolchain (`3ca910911`)
- **brief,train**: every changed line of the brief and the train view is covered (`e8b1c6235`)
- **offload**: every changed line of the derive offload is covered (`ee320559e`)
- **coverage**: the UI, the landing and the allocation code of the train are covered (`cbd0b6525`)
- **integration**: the trains command, the allocation repair and their neighbours are covered (`ca05abf0b`)
- **mesh**: the journal's file paths and the trunk's refusals are covered (`0cd52569c`)
- **release**: the release module's declaration test names release.next (`6c2531385`)
- **http**: a forbidden capability error is covered on every path it takes (`554734807`)
- **mesh**: a value that is not a key is rejected by every trust policy (`3a0eec0d6`)
- **serve**: a server stops on a signal whether it reads a pipe or runs unbounded (`19fdd2a0d`)
- **executions**: the all-executions channel is subscribed before the execution it must see (`969c2d289`)
- **integration**: the fallback's own failures are tested, and refresh reads what was fetched (`7513bd1ce`)

### Pipeline

- **docker**: publish SHA images and reusable GHCR BuildKit cache (`ea8c47dc8`)

### Housekeeping

- **derive**: the derived data follows #963's assembled head 01c76facd0 (`17aab87ac`)
- **derive**: the derived data follows master 00496ba981 and ADR 0139 (`e88e4a3b3`)
- **derive**: the derived data follows ADR 0139 and the merged head (`6ffacd1ac`)
- **derive**: the derived data follows the merged head (I1981) (`ac95d3b32`)
- **derive**: regenerate the derived data for the per-subcommand effects (I1981) (`63ee47852`)
- **derive**: the derived data follows master 7377fe6568 (#944) (`58d0516f2`)
- **derive**: the derived data follows the train's segment-2 top refs/prep/948 (3935dc8c49) (`74747e93e`)
- **release**: the mesh views declare the 0.21.0 their public contract requires (`be515d65c`)
- **derive**: the derived data follows the release-record fix on the first link (`320651645`)
- **derive**: the derived data follows the first link on master 1393d9ef2a (`0feda12a1`)
- **derive**: the derived data follows master 1393d9ef2a (`7624fbd2f`)
- **campaigns**: the Operation Zero night-shift pack is tracked (`236b4691b`)
- **campaigns**: every pack that arrived after the first import is tracked (`c9f20dc7b`)
- **integration**: derive slice B on slice A's review fixes (`3b883e284`)
- **derive**: the derived data follows the head this branch is stacked on (fabcbcfcb5) (`68369163c`)
- **derive**: the derived data follows the head this branch is stacked on (42053b941b) (`fabcbcfcb`)
- **derive**: the derived data follows the head this branch is stacked on (54a91603bb) (`42053b941`)
- **derive**: the derived data follows the head this branch is stacked on (cd4d0fac1a) (`54a91603b`)
- **derive**: the derived data follows the head this branch is stacked on (7d1cb5c8e5) (`cd4d0fac1`)
- **campaigns**: every pack the owner handed over is tracked, and named with its plan (`9b44f6db5`)
- **derive**: the derived data follows the head this branch is stacked on (684af31491) (`7d1cb5c8e`)
- **derive**: the derived data follows the head this branch is stacked on (dc6d7bd088) (`684af3149`)
- **derive**: the derived data follows the head this branch is stacked on (0c9c59a268) (`dc6d7bd08`)
- **derive**: the derived data follows #909's head 969316d06d (`dcb097394`)
- **release**: raise to 0.21.0 for the public surface #860 moves, and derive (`0c9c59a26`)
- **release**: raise to 0.21.0, and derive on master's merge (`e87106f98`)
- **release**: raise to 0.21.0 for the reopen trail words, and derive (`969316d06`)
- **plan**: I2301 is done, with its evidence at ecb51b2772 (`2841459fa`)
- **release**: raise to 0.21.0, and derive on master's merge (`ded406196`)
- **derive**: the derived data follows #909's head dcf87a48cc (`f37c0facb`)
- **derive**: the derived data follows master 890e112 (`dcf87a48c`)
- **campaigns**: file the Opus/Fable mesh-pack prompt pack as received (`eebab6b00`)
- **derive**: regenerate after merging master (890e112d04) (`ecb51b277`)
- **derive**: the derived data follows master 4dfd1856d2 (0.20.0) (`8e648aa8e`)
- **derive**: the derived data follows the head this branch is stacked on (50abe5d31d) (`77cdfcbae`)
- **derive**: the derived data follows the head this branch is stacked on (8408185fa5) (`50abe5d31`)
- **derive**: the derived data follows the head this branch is stacked on (a313ef424c) (`8408185fa`)
- **derive**: the derived data follows the head this branch is stacked on (75667858da) (`a313ef424`)
- **derive**: the derived data follows the head this branch is stacked on (d3d6cedac9) (`75667858d`)
- **derive**: the derived data follows the head this branch is stacked on (e85b3c6c14) (`d3d6cedac`)
- **derive**: the derived data follows the head this branch is stacked on (f09bee167b) (`e85b3c6c1`)
- **derive**: the derived data follows the head this branch is stacked on (c69ffe4196) (`f09bee167`)
- **derive**: the derived data follows the head this branch is stacked on (396ba67341) (`c69ffe419`)
- **derive**: the derived data follows the head this branch is stacked on (10ebd1a1e4) (`eae89cd3f`)
- **derive**: the derived data follows the head this branch is stacked on (451576e426) (`10ebd1a1e`)
- **release**: record v0.20.0 and the metadata it publishes (`451576e42`)
- **integration**: carry the review fixes to origin without a run (`db90c48c0`)
- **integration**: carry the review fixes to origin without a run (`2bbbe6928`)
- **derive**: the projections of the committed merge (`07cc1b819`)
- **derive**: the derived data follows the head this branch is stacked on (1c5be3baa8) (`6c547edd9`)
- **derive**: the derived data follows the head this branch is stacked on (c343e834c0) (`1c5be3baa`)
- **derive**: the derived data follows the head this branch is stacked on (e8a75ddb75) (`c343e834c`)
- **derive**: the derived data follows the head this branch is stacked on (f37b129016) (`e8a75ddb7`)
- **derive**: the derived data follows the head this branch is stacked on (3fd1894393) (`f37b12901`)
- **derive**: the derived data follows the head this branch is stacked on (d9251c6a6f) (`3fd189439`)
- **derive**: the derived data follows the head this branch is stacked on (29b33daf93) (`d9251c6a6`)
- **derive**: the derived data follows the head this branch is stacked on (12d2524e5f) (`bfc0dd709`)
- **derive**: the derived data follows the head this branch is stacked on (2b782e1280) (`12d2524e5`)
- **derive**: the derived data follows #855's head 192ace8d0a (`2b782e128`)
- **integration**: carry the hand-made merge to origin without a run (`210b2194d`)
- **derive**: regenerate after merging master (`75a1962a0`)
- **derive**: regenerate after merging master (`adeda4a74`)
- **derive**: regenerate after merging master (`5fc4d6b37`)
- **derive**: regenerate after merging master (`a1da0ba23`)
- **derive**: regenerate after merging master (`437dad018`)
- **derive**: regenerate after merging master (`3804c84b1`)
- **derive**: regenerate after merging master (`4e555bf3a`)
- **derive**: regenerate after merging master (`71022ef7a`)
- **derive**: the derived data follows the merged master (`e927547a8`)
- **derive**: regenerate after merging master (`265750f78`)

### Published

- `macos-aarch64` — majordomus-v0.21.0-aarch64-apple-darwin.tar.gz (`0aa31ca2883e`)
- `macos-x86_64` — majordomus-v0.21.0-x86_64-apple-darwin.tar.gz (`93bf75213ab0`)
- `linux-x86_64-gnu` — majordomus-v0.21.0-x86_64-unknown-linux-gnu.tar.gz (`9ad96b9f7f53`)
- `linux-x86_64-musl` — majordomus-v0.21.0-x86_64-unknown-linux-musl.tar.gz (`d68f1e86c855`)
- `linux-aarch64-gnu` — majordomus-v0.21.0-aarch64-unknown-linux-gnu.tar.gz (`b81832a45721`)
- `linux-aarch64-musl` — majordomus-v0.21.0-aarch64-unknown-linux-musl.tar.gz (`904210a20060`)


## v0.20.0 — 2026-10-10

### Added

- **release**: a released archive carries a provenance attestation the installer checks (I2165) (`dd5ec84ac`)

### Fixed

- **plan**: I2114 names the files it touches, and I1900 is left as master has it (`192ace8d0`)
- **derive**: the lock's script holds under pipefail, and its cases see it (`7455ed364`)
- **derive**: one derive at a time, no orphans, sees what commits will (`6a1e63bbe`)

### Housekeeping

- **derive**: the derived data follows master a804ae2045 (0.20.0) (`f1a892ebc`)
- **release**: raise to 0.20.0 for the provenance feature, and derive (`7fb5e276c`)
- **release**: record v0.19.1 and the metadata it publishes (`03e1d8a59`)
- **ci**: the case bound and the slowest-case budget are measured, not typed (`b1ab190d0`)

### Published

- `macos-aarch64` — majordomus-v0.20.0-aarch64-apple-darwin.tar.gz (`74c11b2a46fb`)
- `macos-x86_64` — majordomus-v0.20.0-x86_64-apple-darwin.tar.gz (`7eb3e79b5125`)
- `linux-x86_64-gnu` — majordomus-v0.20.0-x86_64-unknown-linux-gnu.tar.gz (`beb2cd4bcf6d`)
- `linux-x86_64-musl` — majordomus-v0.20.0-x86_64-unknown-linux-musl.tar.gz (`7e544a0de55d`)
- `linux-aarch64-gnu` — majordomus-v0.20.0-aarch64-unknown-linux-gnu.tar.gz (`9bce00acf355`)
- `linux-aarch64-musl` — majordomus-v0.20.0-aarch64-unknown-linux-musl.tar.gz (`84b4201b176e`)


## v0.19.1 — 2026-10-10

### Fixed

- **mcp**: a refusal from a serving shared server is answered, and elects nobody (`8425606a1`)

### Housekeeping

- **derive**: follow the v0.19.0 record (`19aa4ff6a`)
- **release**: record v0.19.0 and the metadata it publishes (`121f7dd87`)
- **release**: raise to 0.19.1 and derive (`8811c8a23`)

### Published

- `macos-aarch64` — majordomus-v0.19.1-aarch64-apple-darwin.tar.gz (`9fd32a821069`)
- `macos-x86_64` — majordomus-v0.19.1-x86_64-apple-darwin.tar.gz (`01665bd8de68`)
- `linux-x86_64-gnu` — majordomus-v0.19.1-x86_64-unknown-linux-gnu.tar.gz (`6271f177b590`)
- `linux-x86_64-musl` — majordomus-v0.19.1-x86_64-unknown-linux-musl.tar.gz (`3e91ceb9e396`)
- `linux-aarch64-gnu` — majordomus-v0.19.1-aarch64-unknown-linux-gnu.tar.gz (`62589191c7cd`)
- `linux-aarch64-musl` — majordomus-v0.19.1-aarch64-unknown-linux-musl.tar.gz (`d508e4c54d30`)


## v0.19.0 — 2026-10-09

### Decisions

- **ADR-0123** The worker is asked for a handover before the context window fills, by a provider mod the policy switches _(proposed)_

### Added

- **capture**: the worker is asked for a handover before the window fills (`81ce30338`)

### Fixed

- **capture**: remove only what was installed, and type no context figures (`46bde5240`)
- **site**: the release feed shows the changes that exist, not three it assumes (`86312044e`)

### Documentation

- **plan**: four adopter and runtime findings, and the pages derive writes (`d45bdb577`)
- **plan**: I2059 and I1995, two runtime defects the plan did not hold (`10178b6f7`)

### Housekeeping

- **release**: raise to 0.19.0, record the verification episode, derive (`5b5367f08`)
- **release**: record v0.18.0 and the metadata it publishes (`1e09f7301`)

### Published

- `macos-aarch64` — majordomus-v0.19.0-aarch64-apple-darwin.tar.gz (`c5c4e75deab6`)
- `macos-x86_64` — majordomus-v0.19.0-x86_64-apple-darwin.tar.gz (`dbb82c3d9c2f`)
- `linux-x86_64-gnu` — majordomus-v0.19.0-x86_64-unknown-linux-gnu.tar.gz (`e8e6c7f6305a`)
- `linux-x86_64-musl` — majordomus-v0.19.0-x86_64-unknown-linux-musl.tar.gz (`063bfd95327c`)
- `linux-aarch64-gnu` — majordomus-v0.19.0-aarch64-unknown-linux-gnu.tar.gz (`af08acfd1c9f`)
- `linux-aarch64-musl` — majordomus-v0.19.0-aarch64-unknown-linux-musl.tar.gz (`48c52e57a5f9`)


## v0.18.0 — 2026-10-09

### Decisions

- **ADR-0111** A task names what it serves, and the binding is derived, pinned and never stored _(proposed)_

### Added

- **intent**: a task names what it serves, and the binding is derived, pinned and never stored (`4f4136ea6`)

### Fixed

- **site**: the source-pin debt is paid, eleven to none (`b72089932`)
- **batch**: the suite budget is re-measured, and two failures found in other runs are fixed (`5f01c9e07`)
- **probe**: a chosen tab's panel that did not appear is not a region nothing reveals (`707629eca`)
- **test**: two surfaces are compared without the age a finding quotes (`ee24f0d73`)
- **test**: case 938 sorts in the C locale (`bfcd1a7c4`)
- **integration**: an unlisted open successor and a successor read in part hold, they do not release (`4335a3002`)
- **integration**: a binding the forge spells unreadably is a requirement that was not read (`9591a68d0`)
- **integration**: a supersession is declared by someone who may, and closes on what git finds (`1970e9252`)
- **integration**: a required check bound to an app is only that app's run (S1) (`16d90ffeb`)

### Tests

- **ci**: case 97 reads the dispatch arm the release record's verdict split (`2ce9315fa`)
- **mcp**: every listed tool is one a client accepts, held over the wire (`310a2c87e`)
- **intent**: every changed line of the binding is covered in process, and its shell is declared (`f5de1f889`)

### Housekeeping

- **derive**: the batch's derived artifacts follow the v0.17.0 record (`3f7e93d0b`)
- **release**: record v0.17.0 and the metadata it publishes (`c174bca9d`)
- **batch**: batch 3 is composed, raised to 0.18.0 and derived (`6cbaa8a06`)

### Published

- `macos-aarch64` — majordomus-v0.18.0-aarch64-apple-darwin.tar.gz (`a751cfc79c27`)
- `macos-x86_64` — majordomus-v0.18.0-x86_64-apple-darwin.tar.gz (`c04f73944627`)
- `linux-x86_64-gnu` — majordomus-v0.18.0-x86_64-unknown-linux-gnu.tar.gz (`ee2746a1d6dd`)
- `linux-x86_64-musl` — majordomus-v0.18.0-x86_64-unknown-linux-musl.tar.gz (`4e6356f0c6d1`)
- `linux-aarch64-gnu` — majordomus-v0.18.0-aarch64-unknown-linux-gnu.tar.gz (`bb92f0a28263`)
- `linux-aarch64-musl` — majordomus-v0.18.0-aarch64-unknown-linux-musl.tar.gz (`dd24611623a2`)


## v0.17.0 — 2026-10-08

### Decisions

- **ADR-0110** MCP says nothing its capability does not _(proposed)_
- **ADR-0105** A handover moves between machines as a signed record in a ref of its own, and is resumed only on a plan _(proposed)_

### Added

- **mcp**: a tool says nothing its capability does not (`24f88e9cc`)
- **timing**: the timing report is one line of JSON under --json (`3142d2762`)
- **pack**: a source pack for ChatGPT, verified to carry only committed sources (`748b4cd22`)
- **cockpit**: the plan, one milestone and one issue, with moves run as executions (`54cf109dc`)
- **continuity**: a handover moves between machines as a signed record and is resumed on a plan (`d385ed52b`)

### Fixed

- **test**: case 886 gives both runs of context one clock (`acb1ebb45`)
- **mcp**: every tool's input schema is an object at its root (`88a03d1fc`)
- **pack**: every changed line of the source pack is reached by a test (`7573e2682`)
- **cockpit**: the plan pages spell no layer path and their links are 24px targets (`98f386a02`)
- **mcp**: the clients are ordered by the canonical order, and the case's requests are bounded (`e90368b7f`)
- **security**: a remote's URL is published without its credential (`8fdb8ed2e`)
- **pack**: every exported item shows itself, and the pack tools are called (`8592877a0`)
- **server**: the staleness judgement is unit-tested and case 885 is bounded (`dc1119870`)
- **cockpit**: the issue page's moves ask capabilities.describe, not the registry (`a23e632a3`)
- **server**: a server serving code that no longer exists stops calling itself ready (`8c29f1f3a`)
- **handover**: two records of one second resolve to the later write, not the larger suffix (`fd36929ae`)
- **continuity**: the offered handovers and the changed paths take the canonical order (`aa46dceac`)

### Documentation

- **continuity**: the continuity code carries its examples and its tests (`9b4d3f1b4`)
- **mcp**: every exported type of the projection carries an example, as rustfmt formats it (`fe6d177c5`)

### Tests

- **continuity**: every changed path of the continuity code is driven (`fbea8da0b`)
- **continuity**: the command writes its report once, and the cases name their remote branch (`c123ccd97`)
- **continuity**: the seven continuity tools are asked as a client asks them (`1aad152bd`)
- **mcp**: every changed path of the projection is driven; unreachable arms are folded (`f1fe41d8c`)
- **session**: every branch of the public URL rule is reached by a unit test (`28ada7aa6`)
- **mcp**: a remote keeps its login, and no shared record holds its password (`ba47d6651`)
- **mcp**: the projection's account of itself is held against the server (`a79fcdf9b`)
- **mcp**: the stdio round trip asserts a tool's hints, not the constant they replaced (`82a7ab31d`)
- **mcp**: the server is held to the protocol and its boundaries from outside (`032c54ee0`)
- **cockpit**: the last unreached arms of the plan pages are reached (`88c1edacb`)
- **cockpit**: every arm of the plan pages is reached by a test (`f4401610e`)

### Housekeeping

- **batch**: batch 2 is composed, raised to 0.17.0 and derived (`3b9f93790`)
- **release**: record v0.16.0 and the metadata it publishes (`52e9738c6`)
- **derive**: the projections follow the merge with master (`a993a21d4`)
- **derive**: the projections follow the conformance suite (`6bd507c91`)

### Formatting

- **integration**: the trail test is formatted as rustfmt formats it (`9f00e62c6`)

### Published

- `macos-aarch64` — majordomus-v0.17.0-aarch64-apple-darwin.tar.gz (`db80bb29dcc5`)
- `macos-x86_64` — majordomus-v0.17.0-x86_64-apple-darwin.tar.gz (`2293e5867027`)
- `linux-x86_64-gnu` — majordomus-v0.17.0-x86_64-unknown-linux-gnu.tar.gz (`d18f937c7493`)
- `linux-x86_64-musl` — majordomus-v0.17.0-x86_64-unknown-linux-musl.tar.gz (`7597dcf38b2b`)
- `linux-aarch64-gnu` — majordomus-v0.17.0-aarch64-unknown-linux-gnu.tar.gz (`fd4ca7f47470`)
- `linux-aarch64-musl` — majordomus-v0.17.0-aarch64-unknown-linux-musl.tar.gz (`a5fc3dd0a1d2`)


## v0.16.0 — 2026-10-08

### Added

- **serve**: the interface a local server binds is the machine's to name (`c39c30cb7`)
- **evidence**: a recording carries what its run measured, as a typed run (`0e0e9443e`)
- **evidence**: a crate binary that ran nothing is not a pass (`62f9ec556`)

### Fixed

- **batch**: the findings of the first run that are the batch's own (`90b6dff01`)
- **docs**: the heading a link names has one anchor in every renderer (`4f5baa60c`)
- **test**: case 991 judges a terminated command after the one that follows it (`a390e0235`)
- **live**: a request that arrives during a rebuild waits for it (`e3e122211`)
- **test**: case 413 records the reports its run stamped (`b2f66bc5b`)
- **evidence**: a run's records and its excluded outputs are put in canonical order (`2f12b7501`)
- **finish**: the registry declares exit 13, a note that could not be recorded (`bcaef20f7`)
- **finish**: a note is read once, and kept before the outcome says completed (`1b404b69c`)
- **lint**: a reporter that sets a flag is a reporter (`15f95faf9`)

### Changed

- **evidence**: a coverage summary is read without a debug-only assertion (`c0a296d0c`)

### Documentation

- **serve**: the host resolution's public items carry an example (`dec928066`)
- **evidence**: the stamp and record pair says why no documentation check runs it (`67daf5123`)
- **evidence**: a run's totals say what they count (`9687d8b0f`)

### Tests

- **serve**: every arm of the host resolution runs under a test (`29865face`)
- **evidence**: the newest failing recording is the one a stale judgement names (`3265284a1`)
- **evidence**: every line the typed run changed is reached by a test (`5a96f3a6e`)
- **evidence**: the text recording runs through the command and reports a failed write (`df28b9543`)
- **evidence**: every line the crate adapter changed is reached by a test (`d41005bbb`)

### Housekeeping

- **derive**: the batch's derived artifacts follow the case 991 fix (`a27f5b7d6`)
- **release**: record v0.15.0 and the metadata it publishes (`f062a71cd`)
- **derive**: the projections follow the merge with master (`0594c0097`)

### Published

- `macos-aarch64` — majordomus-v0.16.0-aarch64-apple-darwin.tar.gz (`b92c3af90eed`)
- `macos-x86_64` — majordomus-v0.16.0-x86_64-apple-darwin.tar.gz (`9ee53da26715`)
- `linux-x86_64-gnu` — majordomus-v0.16.0-x86_64-unknown-linux-gnu.tar.gz (`f784ddaf5c29`)
- `linux-x86_64-musl` — majordomus-v0.16.0-x86_64-unknown-linux-musl.tar.gz (`62d6248ebde7`)
- `linux-aarch64-gnu` — majordomus-v0.16.0-aarch64-unknown-linux-gnu.tar.gz (`2a2973a878f4`)
- `linux-aarch64-musl` — majordomus-v0.16.0-aarch64-unknown-linux-musl.tar.gz (`aa85553180e1`)


## v0.15.0 — 2026-10-07

### Decisions

- **ADR-0107** Completion discharges obligations and never decides whether an intent is satisfied _(proposed)_

### Added

- **mesh**: the owner's MacBook Pro is a rendezvous hub on the LAN (`5e81feda0`)
- **mesh**: the owner's second MacBook Pro and iMac are trusted (`45b8d5df8`)
- **intent**: derive an intent's verdict from its evidence alone (`ed66fee5e`)
- **site**: a feature page is placed under its domain and told in order (`55b52bf3f`)
- **site**: /features/ is the product by domain (`a03cfa6bc`)
- **site**: one feature card, drawn from a feature or a domain member (`b17efb1ad`)
- **site**: every badge component is drawn by the one status primitive (`3a538752c`)
- **site**: the challenge tells the story of its run before its output (`50997beaa`)
- **site**: one terminal primitive for the concise story and the full evidence (`16e1d5a92`)
- **site**: one status primitive, and the chain from a claim to its proof (`27445eaf8`)

### Fixed

- **tmp**: only a name directly in TMPDIR is moved, and a dead root is swept (`f9b6e00a1`)
- **tmp**: a command leaves nothing behind in TMPDIR (`744c7d552`)
- **update**: the renderer's scratch directory is named as the temp path it is (`f7b9d6d4c`)
- **site**: both committed site graphs are taken through one refusal, which a test reaches (`4e7d9f49a`)
- **economics**: the context suite is re-recorded at B3's tree (`2488bc630`)
- **graph**: the composed graph read as data is whole; a committed graph is never a prefix (`9f98133aa`)
- **integration**: a drain says why it stopped once (`bdabd87db`)
- **test**: case 545 waits for the re-read it is promised; multicast binds the port it uses (`7e78012d2`)
- **intent**: case 895 sorts pinned, and the satisfaction rule names no absent path (`f1880423b`)
- **test**: case 884's feature and domain declare what their schemas require (`4153efdd4`)
- **doctor**: a stale bootstrap warns; the rule names its enforcement (`e2a9a7b43`)
- **doctor**: a generated bootstrap names only rules in force (`48869b3ca`)
- **site**: the copy-button spec reaches a button inside a collapsed disclosure (`924ad830c`)
- **generate**: the registry counts the web surfaces every checkout has (`e6392cc17`)
- **intent**: a criterion is met only by evidence current at the judged revision (`88223bb02`)
- **intent**: preflight follows what an issue serves and answers serves, maintenance or refused (`9d3449a35`)
- **github**: --apply refuses an adapter loaded from a tree that is not the trunk (`64cd0ebef`)
- **release**: prose names no version label, and the limitations page says what is true (`82d3b7c67`)
- **github**: --apply projects only the clean trunk, one at a time, and a refused write fails it (`c97f92cf4`)
- **site-check**: the feature-id grep pattern braces its variable (SC1087) (`b60361833`)
- **site**: a feature's status and its claims' proof use the one status badge (`7dec31466`)
- **knowledge**: a domain is a kind the graph knows, and a feature is part of its domain (`c7484f446`)
- **knowledge**: every kind the skeleton declares is a kind the graph knows (`435392297`)
- **site**: a status badge contains its own screen-reader label (`39d3625f5`)
- **site**: a copy button says when the clipboard refuses, and a run's exit reads as a number (`eb0a11e66`)
- **site**: the features hero gives its three ways in equal weight (`21331aef4`)
- **site**: a feature's own headings read as headings and lead its outline (`3b91599f6`)
- **site**: the interfaces table keeps its caption inside its scroller (`7fa8d2240`)
- **site**: a recorded run wraps inside its block instead of scrolling sideways (`c3a04f441`)
- **yaml**: a doubled quote inside single quotes is one quote (`9030dc479`)

### Changed

- **site**: a graph artifact is one map, with no branch a graph cannot take (`19c183c6a`)
- **site**: both site graphs are one artifact helper, with no early return a test cannot reach (`92923fac5`)
- **intent**: the plan-records filter takes no branch a comparison never reaches (`dc06d3961`)
- **intent**: drop the unreachable unresolved-milestone branch in drift (`c65e0012f`)

### Documentation

- **readme**: restore the token-measurement limitation #785 wrote, and derive the batch on B3 (`a3c97b048`)
- **graph**: derive_whole links derive() unambiguously (`d44649947`)
- **integration**: the stage-1 transcript is marked as a run against the real forge (`2260311ef`)
- **integration**: rollout stage 1 is recorded, taken with the released 0.14.0 (`e5ff8962d`)
- **mesh**: MulticastProvider::port carries its example (`79ecdbb94`)
- **intent**: the preflight claim says what preflight does: serves, maintenance or a cause (`7dcb7b50f`)
- **rules**: the intent rules list every drift code the engine emits (`834e75bc2`)
- **intent**: completion never decides whether an intent is satisfied (`d8580ff69`)
- **intent**: the preflight says it is at least as strict as validate, not its twin (`307387374`)
- **intent**: the preflight claim says it is a question, and the new prose is wrapped (`a737898a7`)
- **intent**: the homepage lists what of the intent model is built from the claims matrix (`2bb84d39f`)
- the declared status is "rejected", and economics measures what it measures (`bdd0fd6f5`)

### Tests

- **integration**: a drain's JSON report is driven; its write takes no arm of its own (`b30dc76f9`)
- **intent**: every reachable path of the preflight is driven; unreachable arms are folded (`7ac448873`)
- **intent**: case 893 drives the preflight's three verdicts through a fresh init (`fe5e8be96`)
- **release**: every changed branch of the label check is covered by a unit test (`f76abc08f`)
- **site**: case 12 asserts the matrix's two defined evidence columns (`6b8611e51`)
- **site**: a feature added under a domain is drawn there, and removed, is gone (`3370cd526`)
- **site**: site-check holds the feature index and pages to the dataset (`b97de7c7c`)
- **site**: a status speaks the vocabulary of its kind, and a refusal count is the run's (`b9559c211`)

### Housekeeping

- **plan**: close the four issues master already holds, and the episodes nobody closed (`21704692c`)
- **release**: the intent batch is 0.15.0, the minor its public contract requires (`05ed532db`)
- **derive**: regenerate after refreshing the intent batch on master 5908234ce0 (`f1dafe906`)
- **derive**: regenerate B3 on master 5908234ce0 (v0.14.0 recorded) (`f6eeac612`)
- **release**: record v0.14.0 and the metadata it publishes (`e772b9e83`)
- **derive**: regenerate B3 on master 815b96b5a1 and the flakes fix (`340955ce5`)
- **derive**: regenerate after composing the intent batch (`96f214782`)
- **release**: release 0.13.2 with B3 (`3ea3c8bc1`)
- **derive**: the registry counts the surfaces a clean checkout has (`00e71aff3`)
- **derive**: the projections follow the preflight rebased onto master (`985053190`)
- **derive**: the intent claims' projections are current on master 2a4fe055b8 (`b7ec513ae`)
- **ci**: case 810's duration is measured, not defaulted (`8f7ec3cc8`)

### Published

- `macos-aarch64` — majordomus-v0.15.0-aarch64-apple-darwin.tar.gz (`744aee1f6fc9`)
- `macos-x86_64` — majordomus-v0.15.0-x86_64-apple-darwin.tar.gz (`e617e74ae909`)
- `linux-x86_64-gnu` — majordomus-v0.15.0-x86_64-unknown-linux-gnu.tar.gz (`17ea43108693`)
- `linux-x86_64-musl` — majordomus-v0.15.0-x86_64-unknown-linux-musl.tar.gz (`a9da8a5d941f`)
- `linux-aarch64-gnu` — majordomus-v0.15.0-aarch64-unknown-linux-gnu.tar.gz (`3218abe60a5e`)
- `linux-aarch64-musl` — majordomus-v0.15.0-aarch64-unknown-linux-musl.tar.gz (`ed6d4e068aa2`)


## v0.14.0 — 2026-10-06

### Added

- **integration**: one executor per repository across machines, and its throughput is seen (`9272aaf79`)
- **integration**: an overlap names its kind, and risk reads what a merge changes (WP16) (`46d40ac7f`)
- **integration**: a dependency is satisfied by a merge alone (WP14) (`83f35ec7f`)
- **integration**: continuous mode is gated by the trail's record of verified merges (WP21) (`3ba8aa2a1`)
- **integration**: an offline cleanup read, and Obsolete only from a person's label (WP23) (`85ca02dbf`)
- **integration**: every assessment names its issue and milestone (WP24) (`1e3317556`)
- **integration**: a rank names its factors; dependents and size order equals (`b4f0befd3`)
- **integration**: prs repair brings master into one named pull request; scripts/unblock retired (`2c241c981`)
- **integration**: cleanup reports the branches merged pull requests left on origin (WP22) (`3b2de048e`)
- **integration**: cleanup closes only what it decided, at the head it decided on (`239cfb2e4`)
- **integration**: a refused candidate does not block the queue (`fbbc05546`)
- **integration**: redundant work and a landed successor are told apart (WP13) (`e9c33d54b`)
- **integration**: a merge is proved where it landed; an unverified merge holds the drains (`60267eae0`)
- **integration**: auto-merge is Unsafe, only a merge commit merges, one label policy (WP12) (`34c521981`)
- **integration**: reasons, evidence and gates are typed, and every gate is reported (WP11) (`76e19e1d0`)
- **integration**: the integrator's throughput is folded from its trail (`d2eabd65d`)
- **integration**: a dry run is proved to move nothing (WP29) (`429b08201`)
- **integration**: required checks and reviews are authoritative (`2c8f32e8d`)

### Fixed

- **site**: a feature's capability row wraps a long tool name (`193a8ff7e`)
- **integration**: the proof's listings take the canonical order; a long tool name wraps (`5b5e2d9e4`)
- **test**: every scratch clone the executor merges in names its own identity (`9ac18b1d5`)
- **integration**: a branch that moved since it was observed is a stale refusal (`cc02c9a41`)
- **integration**: repair pushes without a lease; case 720 stays strict (`44ece5bf4`)
- **test**: cases 720 and 850 follow repair's pinned lease and the dependency read (`c6d71803b`)
- **integration**: the leftover claims are released through the claim the lease holds (`38eda121e`)
- **integration**: the private module's doc examples are text, as its convention says (`7fc1edd73`)
- **integration**: repaired heads hold the pipeline; a diagnosed repair dry run exits 10 (`1ccf07926`)
- **integration**: the dry-run proof reads the repository's trail, and a Rust test measures it (`16a73a02d`)
- **integration**: the dry-run proof judges only the mirrors it observed (`705920d5f`)
- **integration**: reads write nothing; an aged observation is said (`137b80590`)
- **integration**: a lease is renewed or lost, and taken for the observed base (`655ff9238`)
- **integration**: racing executors merge once; the lease is a held flock (`c75f2b201`)
- **integration**: every destructive act is on one shared trail before it happens (`e27f2aabb`)
- **ci**: nothing merges a pull request around the integrator (`701cda1e9`)
- **integration**: check-attr's paths are written while its answer is read (`62b7fec4d`)
- **integration**: a ref-keyed relation is dropped while master stands still (`c061ea5d5`)
- **integration**: the executor's own head holds the pipeline while its check is unreported (`11800aa4f`)
- **integration**: every changed line of the relation path is reached by a test (`f661feb39`)
- **integration**: a relation is decided on the observed head, and only facts are kept (`b8b270b00`)
- **integration**: the classifier reads what the forge and the trail say, not prose (`b9c8334c9`)

### Changed

- **integration**: the dry-run proof has no failure branch it cannot reach (`b7aeebd55`)
- **integration**: what merged pull requests left is read by cleanup, not refresh (WP22) (`e0b2908b5`)
- **integration**: a marker's word boundary is the number list's own (`2f062f215`)

### Documentation

- **integration**: review states, dependency markers and relation freshness (`e515d9ee3`)

### Tests

- **prs**: explain says a dependency that landed as well as one still open (`5ccc6fc2a`)
- **integration**: every changed path of prs repair is driven; unreachable arms are folded (`e350e5292`)
- **integration**: the reconciler, a squash landing, explain and a failed proof are driven (`6d901dc37`)
- **integration**: the model, the classifier, the relation, the queue and the metrics owe nothing (`9eec14527`)
- **integration**: every changed path of the executor is driven; unreachable arms are folded (`72d8b44dd`)
- **integration**: the executor runs against a scripted forge; a declared test module is test code (`7cb724e95`)
- **integration**: the trail harness answers the read before a closure; a silent refusal is named (`9d0dc8df1`)
- **integration**: case 850 names the strong case redundant, after the D2 rename (`756ab3597`)
- **integration**: every scripted forge answers the ruleset read, and 850 reads the shared trail (`e99fc7012`)
- **integration**: required checks and reviews, end to end; ADR 0101 item 3 (`dc5a363a6`)
- **integration**: the drain re-plans after every merge, across five drains (`f6d8b2310`)
- **integration**: what the trail must refuse and keep, stated before it is fixed (`51c559aa7`)
- **integration**: the bounded hold's trail lines without a time or a head are tested (`edb04cfcf`)
- **integration**: the fork filter, the base guard and the pending-only hold each have a test (`77fe089cb`)
- **integration**: a relation is the observed head's, stated before it is fixed (`bb1a0a44c`)
- **integration**: what the classifier misreads, stated before it is fixed (`26ff31abd`)
- **integration**: the harness scripts what changes between two observations (`4f655e79a`)

### Housekeeping

- **release**: record v0.13.2 and the metadata it publishes (`20ad547fd`)
- **release**: n6 is 0.14.0; a branch that cannot be listed is refused as such (`e49456930`)
- **derive**: regenerate batch-n6 on master 9a31fed5e5 (release 0.13.2) (`88f097dc2`)
- **derive**: regenerate the composed batch-n6 on master b557ab1c8c (`14dbff991`)
- **derive**: WP22 derived after merging batch-n6 1d6e1c9 (`26d1f4aae`)
- **derive**: batch-n6 derived after WP12 (`141c5b39f`)

### Published

- `macos-aarch64` — majordomus-v0.14.0-aarch64-apple-darwin.tar.gz (`6bd455c540bf`)
- `macos-x86_64` — majordomus-v0.14.0-x86_64-apple-darwin.tar.gz (`af3392bb8e73`)
- `linux-x86_64-gnu` — majordomus-v0.14.0-x86_64-unknown-linux-gnu.tar.gz (`4338a4643f69`)
- `linux-x86_64-musl` — majordomus-v0.14.0-x86_64-unknown-linux-musl.tar.gz (`84bfd813eb4b`)
- `linux-aarch64-gnu` — majordomus-v0.14.0-aarch64-unknown-linux-gnu.tar.gz (`91e824614e48`)
- `linux-aarch64-musl` — majordomus-v0.14.0-aarch64-unknown-linux-musl.tar.gz (`460f9a8b56d3`)


## v0.13.2 — 2026-10-06

### Fixed

- **providers**: the worktree guard is said in as many lines as before (`65940e375`)
- **context**: a path not created yet resolves from its ancestors (`e7e032f02`)
- **decision**: a supersession names one recorded decision, and list marks it (`261cc27a7`)
- **cli**: an installed majordomus runs the commands its bootstraps name (`ed11f797c`)
- **check**: scope leaves out the commits a fast-forward or rebase brought from the trunk (`5f42631e7`)
- **doctor**: the lifecycle doctrine counts the episodes of the branch it judges (`3f2076d13`)
- **doctor**: a linked worktree's hooks are found where git keeps them (`e39a83664`)

### Tests

- **cli**: the forwarding case accepts the executable under either name (`25313dbc9`)
- **doctor**: the linked-worktree case leaves when it cannot enter the worktree (`aa652b90a`)

### Housekeeping

- **release**: release 0.13.2 (`b0944db92`)
- **derive**: the batch follows #783 at its budget-sized worktree paragraph (`515bd83c3`)
- **derive**: the batch's projections follow master with the v0.13.1 record (`84a324cfa`)
- **release**: record v0.13.1 and the metadata it publishes (`35535728e`)
- **derive**: the batch's projections follow master at v0.13.1 (`3419a1c1f`)
- **derive**: the batch's projections follow its seven members (`3abde34fb`)
- **plan**: an adopter's upgrade is a planned outcome, I1980-I1991 (`1aaf9f86d`)

### Published

- `macos-aarch64` — majordomus-v0.13.2-aarch64-apple-darwin.tar.gz (`5e98d56a3e2e`)
- `macos-x86_64` — majordomus-v0.13.2-x86_64-apple-darwin.tar.gz (`02cd63725644`)
- `linux-x86_64-gnu` — majordomus-v0.13.2-x86_64-unknown-linux-gnu.tar.gz (`60280e182b2d`)
- `linux-x86_64-musl` — majordomus-v0.13.2-x86_64-unknown-linux-musl.tar.gz (`3a55f5dda405`)
- `linux-aarch64-gnu` — majordomus-v0.13.2-aarch64-unknown-linux-gnu.tar.gz (`3903322ee961`)
- `linux-aarch64-musl` — majordomus-v0.13.2-aarch64-unknown-linux-musl.tar.gz (`4e1c67dd8d30`)


## v0.13.1 — 2026-10-04

### Fixed

- **site**: the domain page's prose reads its headings as headings (`8294cc6d3`)
- **rules**: a unit-test module in its own file is proof the crate gate runs (`185a30977`)
- **release**: the installer reads /proc where the system has no ps (`4122d3192`)
- **install**: an upgrade keeps a superseded tree a running process uses (`fa305ca91`)
- **live**: a write this process made is a move, on every path that can make one (`7dfba01a8`)
- **pages**: a rebuild is chosen only for the trunk's own tree (`901a146c2`)
- **test**: CI fails on a skip, and a skip is excused only where another job holds it (`f04717040`)
- **reap**: a server that holds its checkout's lease is not abandoned (`849d314b0`)
- **site**: a prose heading reads as a heading, not as a link (`d5ba8abeb`)

### Documentation

- **how-it-works**: every claim the independent audit refuted or overstated is corrected (`456e42fe6`)
- **how-it-works**: the architecture as the code ships it, mapped and audited (`3facc7974`)

### Tests

- **batch-n2**: cover the batch's changed lines, refreshed against master (`d33d51e1c`)
- **rules**: a unit-test module in its own file is run by the crate gate (`104593bdf`)
- **site**: a prose heading reads as a heading, held by case 820 (`46e26b295`)

### Housekeeping

- **ci**: the suite's total budget follows its measured cost under load (`99d7196d6`)
- **ci**: the slowest case's budget follows its measured cost under load (`8bf38314f`)
- **ci**: the coverage job's bound leaves room for contended runners (`dc8c42ecc`)
- **plan**: close what master already holds, retire the manifest nobody can sync (`02706daea`)
- **release**: record v0.13.0 and the metadata it publishes (`a14e24542`)

### Published

- `macos-aarch64` — majordomus-v0.13.1-aarch64-apple-darwin.tar.gz (`d74c537a9aeb`)
- `macos-x86_64` — majordomus-v0.13.1-x86_64-apple-darwin.tar.gz (`cbb7e7988cbd`)
- `linux-x86_64-gnu` — majordomus-v0.13.1-x86_64-unknown-linux-gnu.tar.gz (`37adf8949714`)
- `linux-x86_64-musl` — majordomus-v0.13.1-x86_64-unknown-linux-musl.tar.gz (`1ff8628b3944`)
- `linux-aarch64-gnu` — majordomus-v0.13.1-aarch64-unknown-linux-gnu.tar.gz (`cba95ca7d885`)
- `linux-aarch64-musl` — majordomus-v0.13.1-aarch64-unknown-linux-musl.tar.gz (`0815c41b152a`)


## v0.13.0 — 2026-10-04

### Decisions

- **ADR-0104** The product is filed under a few domains, and a feature names its one _(proposed)_
- **ADR-0077** Published content may be written in the reader's language; the repository's own artifacts may not _(proposed)_

### Added

- **site**: the homepage is told overview first, and the product map is its domains (`b9f476920`)
- **product**: the product is filed under a few domains, and a feature names its one (`6de049eae`)
- **site**: two articles from one evidence base, and the rule change they needed (`d8c97273f`)
- **serve**: what a lease contest is judged by is declared, not compiled (`64143f0ed`)
- **ci**: the queue measures itself, and still cannot merge (`ab06f4382`)

### Fixed

- **site-check**: the homepage's navigation is read by landmark, not by its first 80 links (`b9a9a0e6b`)
- **batch-n1**: the product-domains claim page carries the five sections (`c4557bc0f`)
- **batch-n1**: the four CI findings on 487cedf (`70862769f`)
- **site**: the bar lights the group that holds the section a page lives in (`d5051df78`)
- **product**: what the product surface claims is what the model can back (`b1555082b`)
- **rules**: land-and-publish describes the serialised pages group, not push cancellation (`98e7fd9ae`)
- **lease**: the declared lease timings carry examples of what they answer (`7d8e0dde2`)
- **env**: publish the server after startup and show one entry heading (`48f13f3a0`)
- **site-data**: exit 13 names every internal step, and case 593 asserts what can fail (`bc2df49ff`)
- **site-data**: a read that failed is not an empty repository (`fc885ff63`)
- **serve**: a slow but live owner keeps its lease; a wedged one is still replaced (`48ce13299`)
- **pages**: a deploy is not cancelled by the next merge (`ffb62646f`)

### Performance

- **site**: the navbar needs no Flowbite; its collapse and dropdowns are site/nav.js (`edeb6ec0a`)

### Documentation

- **product**: every public domain type carries an executable example (`cd7caaa1e`)
- **cockpit**: the feature record states what the Cockpit can write (`f769f0d70`)

### Tests

- **env**: the entry's episode and checks are covered, not only their doc example (`487cedf6f`)
- **product**: every changed line of the domain model is covered (`d00baded7`)
- **shared**: a stopped server's listener is awaited, bounded, not probed once (`074b40046`)
- **coverage**: a replacement that cannot be started is an error, not a ready server (`17a4fe283`)
- **coverage**: the declared lease timings and the replaced-executable take-over are exercised (`86f785d5e`)
- **reasoning**: a linked mesh runtime advises, and its absence leaves reasoning intact (`bd9026db3`)
- **site**: the published pair must state the same figures (`1863e82ae`)
- **serve**: the lease-declaration case is 354, not 349 (`e46c62acf`)

### Housekeeping

- **derive**: the projections follow the merge with the v0.12.0 record (`b40738da8`)
- **release**: record v0.12.0 and the metadata it publishes (`a570e5c7b`)
- **derive**: regenerate after merging master (`ec462fb4a`)
- **derive**: the projections follow the merge with batch L (`0ddc56041`)
- **derive**: re-derive after merging the Cockpit record fix (`9e4e6a47b`)
- the lease branch follows master (`8980326f7`)
- **derive**: the projections follow the merge with master (`9c7733cf2`)

### Published

- `macos-aarch64` — majordomus-v0.13.0-aarch64-apple-darwin.tar.gz (`a556d96fcd8b`)
- `macos-x86_64` — majordomus-v0.13.0-x86_64-apple-darwin.tar.gz (`08a686e7a082`)
- `linux-x86_64-gnu` — majordomus-v0.13.0-x86_64-unknown-linux-gnu.tar.gz (`6c828fe60b3f`)
- `linux-x86_64-musl` — majordomus-v0.13.0-x86_64-unknown-linux-musl.tar.gz (`71fd00c91e1b`)
- `linux-aarch64-gnu` — majordomus-v0.13.0-aarch64-unknown-linux-gnu.tar.gz (`84ad1c317e21`)
- `linux-aarch64-musl` — majordomus-v0.13.0-aarch64-unknown-linux-musl.tar.gz (`52038e7a81d4`)


## v0.12.0 — 2026-10-03

### Decisions

- **ADR-0076** A skill is a capability only when something proves it — tested, documented, enforced and used are derived, never authored _(proposed)_
- **ADR-0075** Which work realises an intent is a join over the records the lifecycle already keeps, and closed work does not outrank evidence _(proposed)_
- **ADR-0073** An issue names the intent criterion it serves, and the coverage of every criterion is derived from the plan _(proposed)_
- **ADR-0070** An intent is a typed record, and its stage and satisfaction are derived from the plan and the evidence _(proposed)_
- **ADR-0091** Knowledge is derived at the episode boundary, from the ledger and git, and a person promotes it _(proposed)_
- **ADR-0046** A working context is derived and a closed record is composed; neither is a document somebody must remember to write _(proposed)_

### Added

- **site**: the homepage is a landing page, on a dark stage, with the evidence up front (`e4a652f1d`)
- **site**: the homepage is the landing design, composed of the kit (`fe16b7893`)
- **site**: the eight page designs, composed of the kit from real data (`20435f1b3`)
- **site**: /kit/ is a section, and a screen is a page composed of the kit (`13eadc43b`)
- **site**: a component kit the product and documentation pages are composed of (`86adf6dd5`)
- **derive**: a visitor watches the tool refuse an agent, from one recorded run (`973d885ab`)
- **ci**: a documented command is run, or says why it cannot be (`2e2b1b808`)
- **test**: a case can name what it covers, whatever kind of thing that is (`3d6aef2af`)
- **ci**: advisory is not an exemption from naming proof (`16226ef12`)
- **knowledge**: derive knowledge at the episode boundary, serve it everywhere, enforce it (`26a1dd777`)
- **evidence**: a subject is judged at the presented revision (`f27139ecd`)
- **evidence**: subjects are derived from what the layer declares (`405732fc3`)
- **site**: every tile that shows a fact names the test that proves it (`c1e87622d`)
- **skills**: a skill is a capability only when something proves it (`773cb6da5`)
- **cockpit**: an intent's page links each criterion to its test and the work realising it (`98b5921c4`)
- **intent**: the work realising an intent is joined across episodes, providers and handovers (`706f11593`)
- **intent**: an issue names the criterion it serves, and the plan is held to its intent (`127919bf6`)
- **convergence**: where the work is held is a verdict, not a thing somebody might notice (`8381c9c3c`)
- **ci**: a release follows the verdict, and a check that could not measure refuses (`2b22edfc4`)
- **intent**: an intent is a typed record whose stage is derived (`06cccb8ae`)
- **product**: a claim shows the test that settles it, and the matrix counts what is proven (`dcfbd073a`)
- **session**: every client gets an episode, and every provider capability cites its evidence (`ef103581f`)
- **session**: a closed episode's record carries a composed body (`f07c772fb`)

### Fixed

- **batch-l**: the realization read carries an example, and a tag header wraps at 320 px (`4f57a9c6c`)
- **cockpit**: the intents page asks objects.get whether it holds an object (`bb2b892a2`)
- **site**: a component argument that is a variable is passed in braces (`230828448`)
- **batch-k**: a proof link names the built commit, and case 409 reads find without a word split (`711d93d35`)
- **mesh**: the multicast test binds the port it then listens on (`31e0cc358`)
- **site**: the features screen builds without the getting-started page (`949d9a51b`)
- **mesh**: the UDP test binds the port it then listens on (`c1443916d`)
- **economics**: the context suite is re-recorded at batch L's tree (`6728930e0`)
- **intent**: plans and realised work are shown in the canonical order (`8d4486c1a`)
- **site**: a kit table scrolls by its class, and a route name breaks (`96dc16862`)
- **docs**: the release verdict step is marked as one only GitHub can answer (`af420c7fd`)
- **site**: two branches the coverage gate found untested, and a modulo read as a percentage (`6af537d80`)
- **probe**: a list filter is judged once its items have settled (`c1f7879cc`)
- **batch-j**: session context's fixture states #220's contract, and skills find their own executable (`87f8f2e08`)
- **probe**: a wait polls on an interval, because a starved renderer delays its frames too (`d0fe5ac81`)
- **site**: resolve the kit sheet the merge left in conflict, and the homepage's two overstatements (`630bcf661`)
- **probe**: the kit's copy and search specs judge what happened, not what remains (`8d43722d9`)
- **design**: text on a tone's ground is measured, and every tone ground passes it (`51093cff9`)
- **finish**: the vacuous-verification finding is one the reproduce gate can read (`45feab9a8`)
- **test**: a prefixed covers word names no command, and case 409 sorts in C (`ff0eeef42`)
- **site**: every sentence on the screens is true of this repository (`e3e486a33`)
- **site**: the kit builds without CSS and its secondary text reads on a tone (`07cbdc40c`)
- **probe**: a revealed panel is given the frame Alpine shows it in (`274794df1`)
- **site**: the rest of the screens scaffold, which the previous commit left out (`3cd59b334`)
- **economics**: the context suite is re-recorded at batch J's tree (`740d8624c`)
- **batch-l**: a plan is started by its transition, not a hand-written stamp (`511946c72`)
- **batch-j**: the members' cases, gates and rustdoc pass on the batch's tree (`78f09f8af`)
- **site**: the economics revision links are targets a finger can hit (`daaa96a73`)
- **site**: the kit passes the gates CI holds it to, and 0.12.0 carries the tone kind (`6690fb53a`)
- **economics**: the context suite is re-recorded at batch L's tree (`94e8d8c58`)
- **server**: an episode survives a reload, and a bench counts its own executor (`a7e206554`)
- **economics**: the context suite is re-recorded at batch I's tree (`fe1c9fefb`)
- **batch-i**: the intent kind is declared and the campaign check sorts as it joins (`8183164ad`)
- **usecase**: a recorded verification does not carry the recorder's tree (`537c4e7c9`)
- **test**: the intent record tool is run over MCP and its answer checked (`eb4ac5e0d`)
- **test**: case 409 skips through the helper, not with a bare exit 0 (`22affef5c`)
- **test**: batch I's cases hold to the rules its members bring together (`1fce827b0`)
- **test**: the convergence tool is run over MCP and its answer checked (`ad54d9bde`)
- **site**: the intent, gap and critique kinds say where they are published (`b128120a8`)
- **repair**: the context suite is re-recorded at the merged tree (`16713d31b`)
- **ci**: the rust job runs when the plan selects the intent-realization gate (`5f158bb06`)
- **adr**: a lock is broken only by a breaker its holder's release must wait for (`610c3fd29`)
- **ci**: the campaigns check refuses names and entries it cannot record (`c542a6a2a`)
- **shell**: the release verdict is a declared unit of the inventory (`19a8a130d`)
- **ci**: the campaigns check sees every manifest, and hashes without Perl (`cc48da39b`)
- **shell**: an installed tool finds its own executable, not one in the repository (`9a4a69a96`)
- **ci**: the campaigns check runs under pipefail and is declared (`67e6cf587`)
- **doctor**: a coverage name the installation cannot resolve is undecided, not broken (`39bd6ff2f`)
- **site**: a claim's test path wraps on a phone, and case 12 escapes its dot (`73cb31501`)
- **test**: every fixture that copies the harness or rust-check carries lib/sha256.sh (`a9a97b4a0`)
- **site**: the challenge pins its source links and a feature owns its use case (`cef1d85b5`)
- **ci**: the plan job exposes the intent_check output the rust job is gated on (`01a97489c`)
- **test**: case 638's foreign-tool example is a command the liveness gate does not flag (`5ca4912fe`)
- **test**: case 617 runs both coverage checkers and names are matched literally (`b58c13afe`)
- **project**: issue I1962 no longer scopes the deleted surface-coverage script (`42a5715ee`)
- **rules**: the liveness rules and the proof claim say what the gate now reads (`eae78d958`)
- **shell**: nothing in the repository runs perl, and two scripts nothing calls are gone (`e483e16e4`)
- **test**: the knowledge cases skip through the helper and never read as an early exit (`49eab1762`)
- **ci**: the rust job is gated on the intent-check plan output (`c7aa59493`)
- **release**: a tag older than the record practice is a note, not a defect (`86ba91c53`)
- **site-data**: two payloads are read from files, because Linux caps an argument (`6ea7331f2`)
- **convergence**: holdings are ordered once, and every public item shows what it does (`26dd52a38`)
- **ci**: a campaign brief may name what this repository does not have (`8ecf38b24`)
- **cockpit**: derive the areas and the route table from the dispatcher (`de8415b6e`)
- **gates**: the evidence gate stops adding a finding to the pipefail ratchet (`ba41577fe`)

### Changed

- **order**: fourteen collections take the canonical order, and the baseline falls to 92 (`8221eeac7`)

### Documentation

- **plan**: graph.json's node cap is an issue with its measurement (`d4a4ef37a`)
- **release**: the test names the baseline, not an unpublished history (`dc18ef273`)
- **site**: the manifesto says what exists now that coverage and the two records do (`c3c0c7fd5`)
- **intent**: every exported item of the intent surface carries an executable example (`b24618309`)
- **campaigns**: the briefs that drove these campaigns are tracked, and kept as received (`afeebbb9a`)

### Tests

- **coverage**: gather's open-episode and peer-checkout paths and the empty-work rendering (`c53bad7c7`)
- **coverage**: the subject judge's changed lines are exercised (`317a827a1`)
- **intent**: cover realization, review and plan coverage; one split per serves token (`37f9de5be`)
- **coverage**: the lines batch J changes are exercised (`de11b4b28`)
- **design**: the inventory is read with tones and without them (`0fe4658e9`)
- **coverage**: the intent lifecycle's changed lines are exercised (`b11ae7700`)
- **usecase**: case 94 keeps the challenge in its pruned catalogue (`a332ed7c6`)
- **installer**: a missing digest tool and a malformed digest both fail closed (`78da54d90`)
- **evidence**: every new line of the subject index is covered (`17776a8a3`)
- **evidence**: the two intent rules are proven by runs recorded against this tree (`ed5eaac2a`)
- **site-data**: the payloads that killed the site job are held under the cap (`35a60de7e`)

### Housekeeping

- **ci**: the pages trigger names the documented-command gate's inputs (`274f79f94`)
- **ci**: the ui audit is bounded by what the grown site costs (`3db3eea4a`)
- **ci**: the suite budget and the structure bound follow batch J's measured cost (`be9f675f1`)
- **ci**: the structure job is bounded by what a full plan costs (`d5e50774d`)
- **derive**: the derived projections follow the header-reader fix and its tests (`2c09e90bb`)
- **derive**: re-derive the screens over the kit branch at master 376ae59 (`a60626f91`)
- **derive**: re-derive the kit over master at 376ae59 (`04791d266`)
- **derive**: batch L's projections follow the plan-start fix (`48bcf921e`)
- **release**: the surface measured a minor, so the version is 0.12.0 (`f3fc57dc6`)
- **release**: record v0.11.0 and the metadata it publishes (`444d3c9eb`)
- **derive**: batch I's derived artifacts are current (`46fe003a5`)
- **derive**: the projections follow the committed merge with master (`4b4d77b68`)
- **derive**: the changelog keeps master's release decisions after the merge (`db46dc70c`)
- **derive**: the derived projections follow the merge of its base (`40600c9db`)
- **derive**: the derived projections follow the merge of its base (`b5fb0201a`)
- **derive**: regenerate the artifacts that name the digest tools (`51f7430e2`)
- **derive**: the derived artifacts follow the corrected knowledge cases (`8679cf5c9`)
- **release**: the version is bumped to 0.11.0, the minor the knowledge surface requires (`3588593c4`)
- **evidence**: record the runs that prove the skills (`7b6966e8b`)
- **derive**: the projections follow the merge with feature/intent-plan-coverage (`8d167ba90`)
- **derive**: the projections follow the merge with feature/intent-lifecycle (`c43af2b8e`)
- **derive**: the projections follow the merge with master (`e7dd4066e`)
- **derive**: the projections follow the merge with master (`730416173`)
- **derive**: the projections follow the merge with master (`25b14cace`)
- merge master and re-derive (`5cc801c96`)
- merge master and re-derive (`0b7a7c9ad`)

### Formatting

- **convergence**: rustfmt (`c5ab8fb45`)

### Published

- `macos-aarch64` — majordomus-v0.12.0-aarch64-apple-darwin.tar.gz (`76602cb005b1`)
- `macos-x86_64` — majordomus-v0.12.0-x86_64-apple-darwin.tar.gz (`2ee4bf28825c`)
- `linux-x86_64-gnu` — majordomus-v0.12.0-x86_64-unknown-linux-gnu.tar.gz (`5f49782c656d`)
- `linux-x86_64-musl` — majordomus-v0.12.0-x86_64-unknown-linux-musl.tar.gz (`da1000f3b0a1`)
- `linux-aarch64-gnu` — majordomus-v0.12.0-aarch64-unknown-linux-gnu.tar.gz (`c611ad630bcf`)
- `linux-aarch64-musl` — majordomus-v0.12.0-aarch64-unknown-linux-musl.tar.gz (`b9e9cd5d4d22`)


## v0.11.0 — 2026-10-02

### Decisions

- **ADR-0098** Reasoning is provider-independent, advisors are optional, and review is a recorded fact rather than a claim _(proposed)_
- **ADR-0101** Pull requests are integrated one at a time, each against a master observed a moment before _(proposed)_
- **ADR-0059** The mesh is on for this repository, and every session start holds it _(proposed)_
- **ADR-0089** An interface shows only what is derived, and unknown is never drawn as healthy _(proposed)_
- **ADR-0088** A dashboard is a projection of capabilities, and every card names the answer it was read from _(proposed)_
- **ADR-0097** A plan stamp carries the seal of its event, because the ledger never leaves the checkout _(proposed)_
- **ADR-0064** A declared tool has a version, and a mismatch is a finding _(proposed)_
- **ADR-0063** A peer's uncommitted work is part of its claim, and a commit that carries it is refused _(proposed)_
- **ADR-0062** A verification is proved against a commit, and two attempts that disagree are not proof _(proposed)_
- **ADR-0061** A gate's verdict is evidence, and a rule the repository enforces itself is proven by it _(proposed)_

### Added

- **integration**: the queue is seen, waits counted, outages retried, the drain continuous (`1042e2d12`)
- **ci**: the version workflow has a model (`ebf39cf7c`)
- **reasoning**: advisors are optional, discovered by capability, and review is recorded (`fa0df532e`)
- **worktree**: a new worktree shares the primary checkout's .envrc.local (`e5134c4e3`)
- **integration**: pull requests are integrated one at a time against the current master (`160b9d758`)
- **preflight**: generated docs are judged by the generation check that ran (`5f99283a1`)
- **health**: the runtime names the commit it was built from (`9fd573ef9`)
- **ci**: every mcp tool the registry projects is invoked by something, or the gate says so (`7e8614b67`)
- **pages**: a failed publication is retried on a clock, and a rebuild is asked only when it helps (`f44cab3f0`)
- **worktree**: the reclaim is a command, not a list somebody has to act on (`1b6a7f5c3`)
- **plan**: a plan stamp carries the seal of its event (`91080bd64`)
- **mesh**: the doctor judges the server, and every session start says it (ADR 0059) (`bfd7db5da`)
- **rules**: majordomus depends on nothing of prismatic, and a gate says so (`cd8e49984`)
- **cases**: a case number is surveyed before it is taken (`399a99bf5`)
- **ci**: the ui-integrity gate refuses UI state that is not derived (`ecd294124`)
- **site**: a module page links its crate reference (`4f377dffa`)
- **preflight**: coverage is judged by the measurement that was recorded (`589843483`)
- **dashboard**: the overview answers four questions from the capabilities that hold them (`6c81ac0e6`)
- **freshness**: every repository answer says when it was observed (`9580ccab6`)
- **evidence**: evidence is judged at the presented revision, and a failure outranks an absence (`87d9909e7`)
- **site**: the homepage shows one declaration reaching every surface (`01795a693`)
- **environment**: a declared toolchain the machine does not satisfy is a mismatch (`68c9cf633`)
- **ci**: the version bump has an actor, and it proposes rather than releases (`f20f6089e`)
- **cockpit**: the Cockpit shows who else is here, and says when it could not ask (`0a1e650d5`)
- **ci**: the knowledge graph's refusal of a doubly claimed identity is a gate (`6ae2f425e`)

### Fixed

- **usecase**: a recorded duration is masked wherever it stands (`77fcf861b`)
- **test**: case 97 builds the site outside the checkout (`cc90060bd`)
- **usecase**: a recorded scenario decides the schema check, so no line is masked (`3b02d3c20`)
- **ci**: the cockpit probe is bounded by what it now measurably costs (`27fc07f0d`)
- **release**: bump and version share one decision, and an unmeasured contract refuses (`9ace743ad`)
- **test**: case 304's fixture is judged without the generator it does not come from (`b8551b309`)
- **cockpit**: the peers page passes o_path by value, as clippy requires (`5c1af0ece`)
- **usecase**: a scenario's record does not say whether its recorder had an executable (`5a65de680`)
- **release**: release version answers with the contract, and commits are evidence (`d30c166d0`)
- **test**: two RepositoryInfo test fixtures name the observations (`9b46f2fd5`)
- **test**: cases 304 and 305 skip through the helper, not exit 0 (`001cca17a`)
- **test**: case 343 bounds its curl calls, and pages.rs is formatted (`04b09df1f`)
- **test**: case 624 skips through the skip helper (`dd656e045`)
- **test**: cases 581 and 582 skip through the skip helper (`4c1286b9b`)
- **cockpit**: the integration page's lane counts say why zero is the fact (`2ee6c1e01`)
- **usecase**: what doctor reports about reasoning is the recording machine's, so evidence masks it (`7386e6e45`)
- **derive**: every stage uses the executable the derivation resolved (`95c34a7c5`)
- **usecase**: a scenario runs in the ci reasoning mode, so its evidence is the same everywhere (`c4af0fbf8`)
- **ci**: the suite's tree is measured in every shard and joined once (`c2e03d6ce`)
- **test**: case 56 judges the faster of two timings (`39b422726`)
- **test**: the heaviest cases open one per shard (`bea1a47e5`)
- **test**: case 578 skips its positive half instead of passing it (`5da587f4a`)
- **ci**: the suite's verifier does not stand in for the suite (`2b60dad3a`)
- **test**: a case never inherits the runner's shard selection (`c296d16dc`)
- **worktree**: cleanup counts a branch the remote trunk contains as merged (`94a906e79`)
- **serve**: stop reports stopped only when the server process has exited (`57d7857c6`)
- **integration**: the subsystem is the executable's, and its contract is tested where it is served (`0d1929685`)
- **health**: the build's commit decision is tested, and case 635 can fail again (`e4324a36d`)
- **worktree**: the reclaim reads each branch's upstream again, not the listing's (`1e1768acf`)
- **site-check**: a failed run that spoke is not described as having printed nothing (`efe6f9178`)
- **test**: case 276's server sweep asserts its exit code and its output (`a9db2405a`)
- **site-check**: a checker that says nothing has not passed, and the relay is written once (`f34235568`)
- **github**: the adapter composes a projected body in one place (`a19588c50`)
- **test**: a case does not kill the machine's servers (`9fd908b99`)
- **test**: the stale-executable case reads a version that exists (`81c2d1bc9`)
- **rules**: the two liveness rules their scans can decide are blocking (`221fee8cc`)
- **finish**: the verified tree is seeded from head, so a flagged file cannot hide an edit (`21895d29c`)
- **cockpit**: the four questions ask for their route instead of reading the registry (`8f2a2a68a`)
- **finish**: a verification records the tree it proved, and refuses a moving one (`6a4a26608`)
- **gates**: a gate verdict that cannot be read refuses the outcome completed (`7a8814dcf`)
- **automation**: the independence gate's exemption states why and when it ends (`122fdc11d`)
- **test**: case 492 reads its diff without a pager, so the liveness gate passes (`2b3a40852`)
- **ci**: the collision guard has a caller, and cannot answer about nothing (`ff28c9710`)
- **mesh**: compaction keeps what is live, and a returning stream is heard again (`93472d7bf`)
- **freshness**: the gates read an observed answer as the branch meant it (`e7097f063`)
- **cockpit**: the compiled stylesheet follows the neutral guaranteed status (`60a52bd7e`)
- **scope**: case 241 takes the name #659 gives it (`4adb2df67`)
- **site**: the local preview serves the pinned build instead of re-rendering with zola serve (`e27be3254`)
- **coverage**: a binary killed mid-run is shown by its unfinished block, not the output's tail (`57b3cd5b5`)
- **coverage**: the coverage job names the test that failed, and two lib flakes are closed (`3f82fb2b8`)
- **scope**: case 241 is no longer mistaken for a generated file (`170f7af35`)
- **site**: committed page content links its source at the built commit (`f628ce053`)
- **test**: a case runs only in the fixture the runner made for it (`10460db3c`)
- **cockpit**: command words, a narrow distribution row, typed chip order (`3f9335b6c`)
- **site**: the status pages carry the evidence verdict beside the declared status (`8e50b09d3`)
- **cockpit**: the Cockpit says only what it knows (`1991e5c03`)
- **site**: a declared guarantee is not shown as proven (`5f72c6bfb`)
- **coverage**: each measurement writes its export to a name of its own (`fc821df1f`)
- **evidence**: a ledger row's commit is never an option, and the cap reads the same test (`388ff69c1`)
- **evidence**: the crate's tree excludes its outputs where rust-check writes them (`d356a7f27`)
- **finish**: the verify command runs without the outcome finish is claiming (`45c39c3bb`)
- **cli**: every value of --presented-tree says what it means (`8ab972f67`)
- **evidence**: CI publishes the tracked ledger's verdict and measures its runs where they ran (`c6a0072ad`)
- **ci**: a missing coverage export is a measurement that could not be trusted (`c57cca945`)
- **knowledge**: the knowledge reader honours the schema the index enforces (`7ecae80f7`)
- **test**: just test runs the plan CI runs, not three suites beside it (`8ff4efc1b`)
- **ci**: the differential coverage gate has a runner, and the case can tell (`12cd83e69`)
- **liveness**: the wait scan reads what the rule names, and a run: line is a command (`2a6f39dfe`)

### Performance

- **ci**: the rust job's tests run in three lanes (`3b7978474`)
- **ci**: the suite runs as four duration-balanced shards with one verdict over all of them (`19a3c0665`)
- **worktree**: the topology measures every work tree at once (`f36c1912e`)

### Documentation

- **build_identity**: executable doctests for the module and identify (`af6ecb206`)
- **worktree**: state::branch carries a doctest of its exact-name filter (`5d6856afd`)
- **worktree**: dirty_states carries an example that measures and keeps input order (`f063b78ef`)
- **environment**: Pin carries an executable example of floor versus release line (`d2a9daa02`)
- **reasoning**: link to the derive function, not the derive attribute (`e824bf06d`)
- **reasoning**: every public item carries an example and every module a behavioural test (`239644a3f`)
- **worktree**: merged_into_trunk carries an executable example (`739a8c934`)
- **security**: the deletion promise names compose, vendor update and migrate with their limits (`e26abf16c`)
- **security**: the promise about deletion says what holds, and a case binds it (`8df1ad0e0`)
- **BREAKING** **models**: a model's status is optional, and absent means the catalogue declares none (`d7e4731af`)
- **test**: case 275 names the renumbered unknown-gate case 578 (`cbfb5c573`)
- **preflight**: the coverage record types carry examples, as the quality gate asks (`cfee37456`)
- **architecture**: name the ADR directory and its file pattern apart, not as a path (`2c376d657`)
- **architecture**: how Majordomus derives its surfaces (`0a0b79b92`)
- the tree's version and the one a reader can obtain (`f5f11620b`)
- who runs the writer, and why nothing did (`8dabaf74a`)
- **adrs**: propose four decisions a shared-checkout session proved (`cfc6ecdca`)
- **environment**: news_digest carries the example its rationale describes (`2b4c09fc6`)

### Tests

- **coverage**: the lines batch H2 changes are exercised (`35c50d293`)
- **site**: case 406's fixture carries what master's homepage-check reads (`ba48ca00b`)
- **integration**: the prs renderings and the forge's JSON readers are proven (`3193d04eb`)
- **reasoning**: the reasoning MCP tools are invoked as a client invokes them (`1e8f57bee`)
- **integration**: case 720 calls the three MCP tools as a client does (`cbb2aa95c`)
- **pages**: a rebuild refusal names this commit's own build in flight (`606190dd3`)
- **ci**: the alphabet gate is asked its question under every locale (`3fdd878a2`)
- **finish**: a verify command that creates an untracked file is refused too (`2e063b7af`)
- **site**: case 535 refuses a local preview that re-renders with zola serve (`cffa760ae`)
- **evidence**: the monotone rule's containments are proven both ways (`4f937e7cb`)
- **evidence**: every changed line of the presented revision is covered (`0ed6f0f8a`)
- **capture**: a receipt names the provider's event in a field, not in its id (`120bb0235`)
- **knowledge**: case 74's fixture is a valid instance, and the tree derives after it (`f9e0e77f6`)
- **knowledge**: case 330 asserts what a de-declared contract now is — a refused file (`63f497288`)
- **ci**: the gate's clean pass names the node it read (`b3f659294`)
- **ci**: six gates that had never been seen to fail are proven by their violations (`dd909daea`)

### Housekeeping

- **generated**: re-derive the changelog once the merge reaches HEAD's history (`398700e58`)
- **derive**: re-derive the changelog and site data over the merge commit (`68708852e`)
- **derive**: the derived files follow batch G2's members (`cfb040d11`)
- **derive**: the derived files follow master and #706 (`5edc95278`)
- **derive**: the derived files follow the merge of master (`9f681784c`)
- **derive**: the derived data follows the widened deletion promise (`c18de2f01`)
- **derive**: the derived data follows the deletion promise (`b22c6c7e5`)
- **ui-integrity**: the four paid baseline lines are dropped, so the ratchet holds them (`465e4db96`)
- **derive**: the derived projections follow the merge of its base (`7c998e3de`)
- **derive**: the derived projections follow the merge of its base (`72641060c`)
- **derive**: the projections follow batch E (`76c56605a`)
- **derive**: the derived projections follow the merge of master (`224d92904`)
- **derive**: the derived projections follow the merge of master (`a491382a5`)
- **derive**: the projections follow batch D (`4f4b03c75`)
- **release**: record v0.10.0 and the metadata it publishes (`b34fd71f3`)
- **derive**: the projections follow the merge with master (`058cb0fbf`)
- **derive**: the projections follow the merge with master (`606b736e9`)
- **derive**: the projections follow the merge with master (`8197b45a9`)
- **derive**: the projections follow the merge with master (`c5f5e49d9`)
- **derive**: the projections follow the merge with master (`726f6436f`)
- **derive**: the projections follow the merge with master (`d21f05b11`)
- **derive**: the projections follow the merge with master (`16da6739e`)
- **derive**: the projections follow the merge with master (`25c30de9a`)
- **derive**: the projections follow the merge with master (`a4356d5a8`)
- **derive**: the projections follow the merge with master (`c695f924a`)
- **derive**: the projections follow the merge with master (`e263a1c89`)
- **derive**: the projections follow the merge with master (`c4d01e562`)
- **derive**: the projections follow the merge with master (`9c2858d25`)
- merge master and re-derive (`afd78acd2`)
- **derive**: the projections follow the merge with master (`0c69ef149`)
- **derive**: the projections follow the merge with master (`464d8f65a`)
- **derive**: the derived data follows the merge with master (`e7b7130f7`)
- **derive**: the stacked tree's derived data (`f60dd9080`)

### Formatting

- **test**: the coverage assertion is formatted as rustfmt writes it (`8ffc3e9dd`)

### Published

- `macos-aarch64` — majordomus-v0.11.0-aarch64-apple-darwin.tar.gz (`a61783ccdd9d`)
- `macos-x86_64` — majordomus-v0.11.0-x86_64-apple-darwin.tar.gz (`0d59cda3ec03`)
- `linux-x86_64-gnu` — majordomus-v0.11.0-x86_64-unknown-linux-gnu.tar.gz (`8b407725a8d9`)
- `linux-x86_64-musl` — majordomus-v0.11.0-x86_64-unknown-linux-musl.tar.gz (`21e8b3c4130b`)
- `linux-aarch64-gnu` — majordomus-v0.11.0-aarch64-unknown-linux-gnu.tar.gz (`4ecbdbb44209`)
- `linux-aarch64-musl` — majordomus-v0.11.0-aarch64-unknown-linux-musl.tar.gz (`8d3c335bfa84`)


## v0.10.0 — 2026-09-29

### Decisions

- **ADR-0087** Evidence is indexed by subject and judged at the presented revision _(proposed)_
- **ADR-0086** The crate's rustdoc is a published web surface at /rustdoc, produced by the invocation its gate proves and verified at the public URL _(proposed)_
- **ADR-0085** The version is authored once, in the crate manifest, and shipped to the shell tool as a generated projection _(proposed)_
- **ADR-0082** Token economics are measured from matched runs and published only past a declared rule _(proposed)_
- **ADR-0080** A CI run reaches the tracked ledger through a recording branch, never through the run that produced it _(proposed)_
- **ADR-0071** A feature exists only when its delivery is computed and every dimension passes _(proposed)_
- **ADR-0068** CI evidence is kept where the run happened, and published with the commit it proves _(proposed)_
- **ADR-0067** Mesh cooperation is authenticated links between runtimes and one replicated journal folded the same way everywhere _(proposed)_
- **ADR-0066** Entry reports a verdict per claim and the evidence behind it, from one preflight value _(proposed)_

### Added

- **test**: a case that declines to run is recorded as a skip, never as a pass (`d348a6c76`)
- **evidence**: text a run printed is redacted by one rule set (`5f1226aca`)
- **product**: every active use case has a feature, and case 503 holds it (`bd74cd30d`)
- **design**: the verdict words are status words (`ba0770c9c`)
- **economics**: record the context suite's first measurement (`54ee7cd24`)
- **site**: the schema reference links every schema file to its source (`300e93ce0`)
- **mesh**: this repository runs its own mesh (`372eccf36`)
- **entity**: every entity names its public documentation page (`ed1398a9c`)
- **economics**: measure token economics from matched runs, never typed (`f48b9b078`)
- **web**: the crate's rustdoc is a published surface, produced by its gate and verified live (`5baad7874`)
- **evidence**: CI records its runs, and the site publishes them with the commit they prove (`5a43a3a05`)
- **rules**: the report says what was proven, not merely that it found no fault (`9119e306a`)
- **delivery**: whether a feature exists is computed, and unknown is not delivered (`9ba6a17f5`)
- **served**: a deployment is evidence, judged by the commit it serves (`1f7e98bbb`)
- **mesh**: runtimes cooperate over authenticated links and one replicated journal (`964d26e3d`)
- **shell**: new shell is refused unless the inventory exempts it (`4a05b6292`)
- **environment**: entry reports a verdict per claim and the evidence behind it (`271989160`)
- **mesh**: a signed cooperation journal and a deterministic state fold (`7bea9265b`)

### Fixed

- **test**: a server's URL is read from its own listening line, once (`a5ea413ff`)
- **site**: long directory paths wrap on the context page (`7f824c3ec`)
- **economics**: one context measurement counts each file once (`b5fc0d5bf`)
- **economics**: the explain example expects the answer, not a refusal (`51a84dd4f`)
- **site**: the economics page links through Zola and names the built commit (`6973ef9fe`)
- **economics**: the benchmark fixture is English and its references resolve (`ad09800e5`)
- **ci**: crate-boundary argues for tiktoken-rs (`da63efb49`)
- **economics**: explain answers a repository without a methodology (`782a7e97f`)
- **rust**: a checkout with no site config still hands off its rustdoc reference (`02bcb9aac`)
- **test**: case 89 empties the server log before it waits on it (`cf5fb2fe6`)
- **test**: the batch's cases decline through skip, the one shape case 413 accepts (`9b52c572b`)
- **shell**: the inventory declares the four CI scripts the batch brings (`5118a3140`)
- **shell**: declare the entity gate master added since the branch (`b594d9f8f`)
- **test**: case 532 declines through skip, the one shape case 413 accepts (`e0d35c170`)
- **pages**: the deploy job's bound exceeds its steps' again, evidence step included (`6974910c8`)
- **evidence**: public text refuses a secret once redacted or cut off (`21d22d944`)
- **evidence**: public text refuses a credential its shape only announces (`9e654b61b`)
- **claims**: deployment-contract-refuses-by-name promises only what case 84 asserts (`6e4b40867`)
- **test**: no case in test/cases/ can decline with exit 0 any more (`c2fb7a66e`)
- **ci**: the job summary counts a SKIP as skipped, not as failed (`1ba9834cd`)
- **test**: a status 4 the case never declared is a failure, not a skip (`08209a66d`)
- **ci**: a case is bounded at 3600 s, above the budget that warns of it (`b1386ae7b`)
- **ci**: suite-budget fails a pipeline on any stage and sorts in byte order (`d68bd8dc5`)
- **ci**: the suite's bound is measured, declared once, and refuses growth (`39b2916f9`)
- **mesh**: a runtime dials its present siblings before the ones that stopped (`2b4aa04cb`)
- **site**: the responsibility pages' file links and site-check follow the built commit (`07035a459`)
- **mesh**: a node's streams are compacted before its quota refuses a sibling (`84ad1dd4e`)
- **pages**: the comment no longer reads to case 97 as a timing phase (`937343683`)
- **site**: a page's source links name the commit it was built from (`2548eff4c`)
- **execution**: a cancel is recorded before its token is visible (`45439dbf0`)
- **pages**: verify-docs says an index it never fetched was not measured (`a50e2385b`)
- **ci**: the evidence collector and rust-check fail when any part of a pipeline does (`c22cb2c11`)
- **evidence**: a proof older than what it is about does not support a guarantee (`e24f21dd7`)
- **mcp**: the instructions say what writes, counted off the registry, not a promise of read-only (`92f641910`)
- **evidence**: a run recorded while the tree was dirty is not proof of the commit it sat on (`b77bfbcdb`)
- **release**: the carrier scan orders canonically, and case 484 pages nothing (`8a0a68ddc`)
- **release**: a version declared in TOML, JavaScript or Python is refused too (`7c5f58da0`)
- **mesh**: a push is counted as it leaves, not when its answer returns (`8173f333d`)
- **site**: git's listing of tracked paths reaches jq by file, not by argv (`6122a6bc9`)
- **mesh**: the lab evidence sorts in one collation and the diagram ids stop colliding with headings (`1f3f1cece`)
- **shell**: declare the shell gates master added since the branch (`be2eeec35`)
- **release**: a release record is derived before it is proposed (`018293b15`)
- **served**: list the deployments in canonical order (`447b76a8b`)
- **test**: case 384 observes a non-zero verdict instead of dying on it (`16a319417`)
- **serve**: stop waits for the lease it read, not for the path to be free (`c0e885275`)
- **serve**: the shutdown's own bound is stated, and begin_stop carries its example (`b3214a797`)
- **serve**: the lease is released before the mesh drains, not after it (`016b04b4f`)
- **site**: the tracked listing reaches jq through a file, and a slow run is not a failed one (`249e597a1`)
- **mesh**: a command with nobody to ask says so, and the lab waits for the runtime it asks (`fb0285477`)
- **mesh**: the gates the first commit failed, each at its cause (`f086432f7`)
- **environment**: a verdict names the tree it was measured on, and the lease is read once (`9fc7284d8`)

### Performance

- **release**: a release's history is listed once, not asked once per decision (`08cbd76d5`)

### Changed

- **order**: the economics sort sites adopt the canonical order (`358a0ab69`)
- **evidence**: the redactor orders its kinds canonically and writes English (`799834606`)
- **evidence**: the redaction table spells a one-byte class inline (`418ebac6b`)
- **release**: the shell tool reads its version from share/version.txt (`7655e9a7f`)
- **release**: the version is authored once, in the crate manifest (`f3553508d`)

### Documentation

- **environment**: the RulesTally example builds its report with a verdict (`e1d61945f`)
- **adr**: the recording stops where the ledger covers, and trunk proof needs a quiet trunk (`3145123fd`)
- **adr**: the recording stops at a ledger-only commit, and publication states match first (`a1ccbebed`)
- **ci**: the per-case budget is the pass limit, and its worst cost is censored (`d0b912444`)
- **adr**: evidence is indexed by subject and judged at the presented revision (`bea35d448`)
- **mesh**: the guide and how-it-works describe cooperation as it ships (`1c35c5d3c`)
- **execution**: say which guard makes a cancel land before its token is seen (`d8f91e79f`)
- **rules**: the verdict's two accessors say what they are for, not what the signature says (`a258c6894`)
- **adr**: ADR 0085, the version is authored once and shipped as a projection (`30a0cb128`)
- **served**: every exported item carries an example that runs (`c71152a3b`)

### Tests

- **evidence**: the ledger-row test names no CI run, a field #577 added (`85d21b7d6`)
- **evidence**: the byte class test spells its samples as byte strings (`d478261fb`)
- **evidence**: the rules side of the tree cap is judged over a test that exists (`8c8efd8bd`)
- **ci**: the artifact case asserts the pairing, not the order of the uploads (`1d4ff7935`)
- **release**: a refused derive-check must fail the publication job (`b5ff5ecce`)
- **release**: the version is authored once, and the cases read it through its one reader (`7f6e3842f`)
- **mesh**: the lab's record is judged from outside the lab, and two claim joins are corrected (`a4ead40eb`)
- **mesh**: the rendering layer is measured, and the version says what the contract did (`e83842630`)

### Housekeeping

- **ci**: the rust job is bounded by what the batch measurably costs (`bf5dbd274`)
- **ci**: the suite budget is re-derived from the batch's measured cost (`fabdc4442`)
- **derive**: the projections follow the economics fixes (`55c556e73`)
- **derive**: the projections follow the merge with master (`b9fdfef74`)
- **derive**: the projections follow the merge with master 35f6457 (`937da6db6`)
- **derive**: the projections follow the merge with master (`a366f3185`)
- **derive**: the projections follow the merge with master 35f6457 (`3f5a84527`)
- **derive**: the projections follow the merge with master (`67b589d7d`)
- **derive**: the projections follow the rebase onto master (`e60a0101d`)
- **derive**: the projections follow the rebase onto master (`e7d6d60b0`)
- **release**: the surface measured a minor, so the version is 0.10.0 (`51a685abd`)
- **derive**: the projections reach their fixed point after the merge (`08d9820d0`)
- **derive**: the projections follow the merge with master (`131c04ee6`)
- **derive**: the projections follow the merge with master (`07b48a0db`)
- **derive**: the projections follow the merge with master (`3959fe407`)
- **derive**: the projections follow the rebase onto master (`bf0dd8305`)
- **derive**: the projections follow the rebase onto master (`c99c34e4d`)
- **derive**: the projections follow the v0.9.0 release record (`0dba33e5b`)
- **evidence**: the baseline records the debt a stricter gate can see (`a009197d1`)
- **release**: record v0.9.0 and the metadata it publishes (`f0f19dea1`)
- **derive**: the projections follow the merge with master, at 0.9.0 (`415cd1d4d`)
- **derive**: the projections follow the merge with master 823ca04 (`1d00b7b80`)
- **derive**: the projections follow the canonical carrier scan (`91f7cc0c7`)
- **derive**: the projections follow the wider version-carrier detector (`ece5faead`)
- **derive**: the projections follow the version authored once (`f2d128fea`)
- **release**: 0.9.0, because served.show joined the contract (`8b7da5672`)
- **release**: 0.9.0, because the environment gained the preflight capability (`814d72a76`)
- **derive**: the projections follow the merge with master (`380c721b4`)
- **release**: 0.9.0, because delivery.feature and delivery.report joined the contract (`c00501b4a`)
- **derive**: the projections follow the merge with master (`b2ef9560e`)
- **merge**: feature/a-deployment-is-evidence meets master (`e3cf03be8`)
- **derive**: the projections follow the merge with master (`83a8af1c1`)
- **derive**: the projections follow the merge with master (`89eea3687`)
- **derive**: the projections follow the merge with master (`8d7125e75`)
- **derive**: the projections follow the merge with master (`58a0428b0`)
- **derive**: the projections follow the merge with master (`13cad65c7`)
- **derive**: the projections follow the merge with master (`8587973f9`)

### Formatting

- **mesh**: the rendering tests are formatted as rustfmt writes them (`dab6e1b56`)
- **delivery**: cargo fmt — the module lists stay sorted and the git args wrap (`9e830d8fa`)

### Published

- `macos-aarch64` — majordomus-v0.10.0-aarch64-apple-darwin.tar.gz (`eec02d0717ff`)
- `macos-x86_64` — majordomus-v0.10.0-x86_64-apple-darwin.tar.gz (`e7657b7bcfa1`)
- `linux-x86_64-gnu` — majordomus-v0.10.0-x86_64-unknown-linux-gnu.tar.gz (`35e6e8fd6eb4`)
- `linux-x86_64-musl` — majordomus-v0.10.0-x86_64-unknown-linux-musl.tar.gz (`06351fb7ac00`)
- `linux-aarch64-gnu` — majordomus-v0.10.0-aarch64-unknown-linux-gnu.tar.gz (`5b7cb6a18f86`)
- `linux-aarch64-musl` — majordomus-v0.10.0-aarch64-unknown-linux-musl.tar.gz (`17e56539424b`)


## v0.9.0 — 2026-09-25

### Decisions

- **ADR-0056** Every object of the layer has a derived address, both directions of every relation, and a declared place of publication _(proposed)_

### Added

- **docs**: the documentation hub and its index are read off the registry and the build (`c3555511c`)
- **entity**: every object of the layer is addressable, cross-linked and published (`c0b1e9424`)

### Fixed

- **docs**: the hub names every kind it links, and the release is the minor the surface measured (`f8a5caf56`)
- **entity**: an edge's order is the collection's own, and the surface is exampled (`0fdca807c`)
- **site**: an entity's URI may not push a phone page sideways (`cb2a69059`)

### Documentation

- **coverage**: the differential rules are a table, not a hand-drawn block (`d15df020d`)
- **entity**: the entity capability's surface carries executable examples (`054c312bf`)

### Tests

- **cockpit**: the entity routes join the read-only and rebuild sweeps (`c0b15d496`)

### Housekeeping

- **derive**: the projections follow the v0.8.0 release record (`6932fc67b`)
- **release**: record v0.8.0 and the metadata it publishes (`49a8f053a`)
- **derive**: the projections follow the merge with master df244393d (`803dc0e47`)

### Published

- `macos-aarch64` — majordomus-v0.9.0-aarch64-apple-darwin.tar.gz (`c2f71342018d`)
- `macos-x86_64` — majordomus-v0.9.0-x86_64-apple-darwin.tar.gz (`8a0e587dcc45`)
- `linux-x86_64-gnu` — majordomus-v0.9.0-x86_64-unknown-linux-gnu.tar.gz (`15189a2bb8d6`)
- `linux-x86_64-musl` — majordomus-v0.9.0-x86_64-unknown-linux-musl.tar.gz (`28d1644ba2f9`)
- `linux-aarch64-gnu` — majordomus-v0.9.0-aarch64-unknown-linux-gnu.tar.gz (`c619f973950a`)
- `linux-aarch64-musl` — majordomus-v0.9.0-aarch64-unknown-linux-musl.tar.gz (`4cd7a2edac6d`)


## v0.8.0 — 2026-09-20

### Added

- **graph**: a dangling reference refuses the generation, and runtime stays beside the graph (`d8355cd5c`)
- **ci**: the crate's dependency boundary is measured, not asserted (`377af4a38`)
- **site**: the inventories the homepage shows are held to the model that owns them (`c386b227a`)
- **finish**: a refused finish is a record, so a caught false done can be counted (`e3a27dd3d`)
- **site**: the homepage weighs what it shows, and loads no runtime it does not use (`f617c5287`)

### Fixed

- **env**: a file the index refused is named, not counted (`d69888e52`)
- **test**: the disclosure spec retries a dropped click, on a bound it measured (`a6fdf859a`)
- **test**: the disclosure spec waits for the state it asserts (`bb3f7ab0d`)
- **site**: every diagram node is unique, and three pages pass the UI audit (`2d4754f54`)
- **site**: scenario cards fit a narrow screen; case 94 commits with its own identity (`939535e25`)
- **site**: a use-case page renders live scenarios and wraps its evidence output (`763b94d7a`)
- **site**: the claims matrix reaches jq by file, not by argv (`6186c7272`)
- **docs**: the case context's doc link names the type it means (`87a904494`)
- **ci**: the UI audit is a job of its own, so it reports instead of being cancelled (`de485b178`)
- the tool's statements about itself match its code; a tested guide (`7a3f035fd`)
- **cockpit**: a page asks a capability for what the layer holds, and a gate holds it there (`f9375c65c`)
- **docs**: two references name what exists (`5e451944d`)
- **test**: the interactive scan walks its globs instead of listing them (`6d7492c29`)
- **ui**: a record page's view parameters are one family, so a pager cannot multiply the routes (`d38384dcc`)
- **release**: the publish job may open the pull request that carries the record (`f36ee380a`)
- **rules**: five rules say how they are decided, and one of them is now a gate (`8b58ff876`)
- **ui**: the audited pages pass their audit, and a graph page is paged like every listing (`3692d34d2`)
- **serve**: concurrent ensures start one process, so a stopped checkout stays stopped (`d00652552`)
- **ci**: a pull request's run reaches its verdict, and a release record is proposed or says why not (`6d08c63d5`)
- **server**: a server answers where it stands from memory, not by probing itself (`173b70a8d`)
- **knowledge**: the discovery cases hold the contract class init now declares (`d82450dce`)
- **serve**: a busy server is not a dead one, so every client attaches to the server entry started (`98c5a26e0`)
- **site**: the refusal is about what the site offers, not every version it prints (`cac5aa6bb`)
- **site**: the version the page shows is the one a reader can obtain (`e6443701c`)
- **skeleton**: init declares the context class, so a new repository indexes its contracts (`587cf472b`)
- **cockpit**: the palette probe tells an empty list from a wrong order (`ffb0ad365`)

### Documentation

- **adr**: decision 0010 states its own context, alternatives and consequences (`f4b6b7d1a`)
- **adr**: the lease already has one reader, and three documents said otherwise (`50e3de273`)
- **claims**: the two live-scenario claims carry every section their page owes (`9697b1098`)
- **graph**: the runtime overlay's exports carry examples, and derive's link is unambiguous (`4fe593b3a`)
- **architecture**: how Majordomus works as one model with many projections (`6778eb7f3`)

### Tests

- **ci**: a whole file's JSON may not travel to jq on the command line (`bf8ce0837`)
- **claims**: every guaranteed claim is asserted by a runnable test that names it (`09a188388`)
- **graph**: the case takes a number above the high-water mark (`ecbfd9b46`)
- **cockpit**: the projection case is 389, above the high-water mark (`611bd605a`)
- **ci**: the crate-boundary case takes 360, which nothing else claimed (`226fd1ecf`)
- **usecase**: a live scenario is held to what a question may do, and its evidence is local (`d2eece55b`)
- **site**: cases 12 and 95 assert what the site builds today, not the pages it used to have (`035ec4490`)

### Housekeeping

- **derive**: the projections follow the merge with master (`1d43b5ffb`)
- **derive**: the projections follow the merge with master (`f4fa183b2`)
- **derive**: the projections follow the merge with master (`f0609de98`)
- **derive**: the projections follow the merge with master (`e069249c0`)
- **derive**: the projections follow the merge with master (`4c2572e49`)
- **derive**: the projections follow the merge with master (`e9891ec1b`)
- **derive**: the projections follow the ui-job split (`d3c2fbf31`)
- **derive**: the projections follow the merge with master (`0cb56094b`)
- **derive**: the projections follow the merge with master (`d8b58ebaf`)
- **release**: 0.8.0, because the layer summary gained a public field (`1b3fb1ac8`)
- **derive**: the projections follow the merge with master (`c9e978504`)
- **derive**: the projections follow the merge with master (`70b9fd202`)
- **derive**: the projections follow the merge with master (`781008657`)
- **derive**: the projections follow the merge with master (`c1580df0e`)
- **derive**: the projections follow the merge with master (`12be9f406`)
- **derive**: the projections follow the merge with master (`5dfefc2ca`)
- **derive**: the projections follow the merge with master (`f8d858f1b`)
- **derive**: the projections follow the merge with master (`67554aed6`)
- **derive**: the projections follow the merge with master (`dba5ac100`)
- **derive**: the projections follow the merge with master (`72d9e1c0d`)
- **derive**: the projections follow the merge with master (`1e9498326`)
- **derive**: the projections follow the merge with master (`e07725676`)
- **derive**: the projections follow the merge with master (`38326fa17`)
- **derive**: the projections follow the merge with master (`ae586ebce`)
- **derive**: the projections follow the merge with master (`29dacf6bf`)
- **derive**: the projections follow the merge with master (`421ad1a87`)
- **derive**: the projections follow the merge with master (`851a34519`)
- **derive**: the projections follow the merges that make master green (`85a6678eb`)
- **derive**: the projections follow the merge with master (`5143e6e59`)
- **derive**: the projections follow the merges that make master green (`787c030ed`)
- **derive**: the projections follow the family rule (`b405caa6b`)
- **derive**: the projections reach their fixed point after the merge with master (`b1f563025`)
- **derive**: the projections follow the merge with master (`8ff2a0833`)
- **derive**: the projections follow the merge with master (`59e7d87c4`)
- **derive**: the projections follow the merges that make master green (`0060fd454`)
- **derive**: the projections follow the merge of the v0.7.0 record with master (`65afe42d9`)
- **release**: record v0.7.0 and the metadata it publishes (`32f59b52f`)
- **derive**: the projections follow the merge with master (`3ed3dbbe6`)

### Formatting

- **test**: rustfmt the surface-writes test (`60b0172b0`)

### Published

- `macos-aarch64` — majordomus-v0.8.0-aarch64-apple-darwin.tar.gz (`b0c899119267`)
- `macos-x86_64` — majordomus-v0.8.0-x86_64-apple-darwin.tar.gz (`14360af012ee`)
- `linux-x86_64-gnu` — majordomus-v0.8.0-x86_64-unknown-linux-gnu.tar.gz (`231d665124a5`)
- `linux-x86_64-musl` — majordomus-v0.8.0-x86_64-unknown-linux-musl.tar.gz (`23bceda3eacb`)
- `linux-aarch64-gnu` — majordomus-v0.8.0-aarch64-unknown-linux-gnu.tar.gz (`383829eee7af`)
- `linux-aarch64-musl` — majordomus-v0.8.0-aarch64-unknown-linux-musl.tar.gz (`1218d079863e`)


## v0.7.0 — 2026-09-15

### Added

- **site**: every link resolves and every control does what a spec says, held by a blocking rule (`7d5fe591a`)
- **site**: the HTTP API reference is an index, a page per tag and a page of shared types (`aa48fef52`)
- **site**: the homepage tells the product's story in a declared order, and a rule holds it (`75729f35c`)
- **recover**: the stray-file sweep is a capability, and the command asks it (`c9dc7a945`)
- **site**: the homepage replays runs that happened, and shows what the tool refuses (`094d34e3b`)
- **site**: the homepage carries the evidence and commit chapters, at 0.6.1 (`11744faf8`)

### Fixed

- **ci**: a step that runs out of time says so itself (`b3e261428`)
- **pages**: the deploy fetches the history and tags its link check reads, so master deploys again (`593cbde3e`)
- **ci**: the terminal refusals apply where scenarios ran, and a log assertion waits (`af0dfd49a`)
- **site**: the links and controls change passes what CI found on it (`3bf9b9f37`)
- **cockpit**: a card title breaks a long identifier instead of widening the page (`9d44b1f40`)
- **site**: the diagram theming writes no colour of its own, so the design gate holds (`167f46ed0`)
- **ci**: site-check counts every failure it prints, and shell-lint refuses the shape that lost them (`0dab3ff51`)
- **docs**: the differential coverage mapping is a table, not a hand-drawn diagram (`2fc1a80ab`)
- **ci**: three reds that CI found on #310's own tree (`30403bcc7`)
- **coverage**: the crate floor is a ratchet, the coverage measured, and only rises (`f15bb4e47`)
- **test**: case 318 forces the SIGPIPE it measures instead of hoping for it (`e01d4013f`)
- **test**: case 135 gives the executable its share, and names the failure (`38b21c221`)
- **ci**: three always-on reds that #310's own fixes leave standing (`bb3138114`)
- **mesh**: the loopback socket tests are host-independent (`f5800b3a7`)
- **coverage**: the crate floor stays where its rule holds it, and only the domain ratchets (`b3d1c2fe2`)
- **coverage**: the coverage floors are the coverage measured, and only rise (`d5dd4b967`)
- **ci**: the benchmark comparison runs where its baseline is, and the rust job fits its budget (`b253a0643`)
- **site**: an example heading is anchored under the type that documents it (`33f2faf2d`)
- **site**: rendered doc comments repeat no id, and case 09 judges each <pre> (`5ae3e808f`)
- **session**: ten closes at once no longer leave the pointer aiming at nothing (`e1a3ab937`)
- **knowledge**: a contract is the kind it declares, and it is that kind once (`47a4477b1`)
- **adr**: the identity survey runs before the lock, not inside it (`f03c1a316`)
- **test**: three suite cases read a tree that is correct as stale or incomplete (`e33566f5c`)
- **site**: a transcript box names the axis that widens a phone (`ecdade1c7`)
- **site**: a rendered doc comment demotes its headings and its functions carry examples (`8b6436fca`)
- **test**: case 12 asserts the hero title, not a markup it never had (`b752ff9b7`)
- **test**: a restored target keeps its cache tag, and the fixture runs CI's cargo (`fd80ca045`)
- **bench**: plan.transition is required where the plan holds an issue, and says so elsewhere (`00128ff63`)
- **site**: capability prose renders its rustdoc instead of publishing it (`7b6421c8d`)
- **ci**: english-only-check sees every script, in any locale (`561d1c1e8`)
- **site**: capability descriptions render rustdoc markup instead of publishing it raw (`caa82f0b9`)
- **rust**: news_digest carries the example its behaviour owes (`26c326ee0`)
- **site**: an enum value's description takes its own line, and stops the API page overflowing (`94ca7ff91`)
- **site**: the panel a reader lands on is the one with the most to show (`106c4e088`)
- **reap**: the reaper survives its own worktree listing, and a consent names its subject (`fc4cad858`)
- **release**: the record reaches master through review, not through a protected push (`11a027868`)
- **events**: task.gate names the module that writes it, and every event is documented (`16e084243`)
- **coverage**: the coverage gate states its own verdict (`3ad3fa014`)
- **docs**: the coverage mapping is a table, and the claim names its case (`55dcc2be6`)

### Documentation

- **coverage**: the merge keeps master's table and this branch's ratchet (`e2d375793`)
- **capability**: the benchmark policy and its precondition carry examples (`796fe233e`)
- **recover**: four examples that prove what the prose asserts (`0ab4e8722`)
- **ci**: english-only-check names the locale CI actually runs, and its real ranges (`1110f2e6f`)

### Housekeeping

- **derive**: the projections follow the merge with master (`1388ad2c3`)
- **release**: 0.7.0, the version the public contract requires (`3dfccb0ff`)
- **derive**: regenerate derived data over the #348 fold (`616c1a55f`)
- **mesh**: mark the host-independent test fix under this task (`9e005ef75`)
- **derive**: the status documents and the registry follow the three case fixes (`59d747ea9`)
- **derive**: the projections follow the merge with master 19b275520 (`f3ad6d694`)

### Published

- `macos-aarch64` — majordomus-v0.7.0-aarch64-apple-darwin.tar.gz (`3f8a635bd7fe`)
- `macos-x86_64` — majordomus-v0.7.0-x86_64-apple-darwin.tar.gz (`729455c0af63`)
- `linux-x86_64-gnu` — majordomus-v0.7.0-x86_64-unknown-linux-gnu.tar.gz (`2a7dd5997444`)
- `linux-x86_64-musl` — majordomus-v0.7.0-x86_64-unknown-linux-musl.tar.gz (`9267cf684dc0`)
- `linux-aarch64-gnu` — majordomus-v0.7.0-aarch64-unknown-linux-gnu.tar.gz (`9d8b18e1fb21`)
- `linux-aarch64-musl` — majordomus-v0.7.0-aarch64-unknown-linux-musl.tar.gz (`375bb5f8cbba`)


## v0.6.0 — 2026-09-12

### Decisions

- **ADR-0055** Disk is the fifth self-accumulating quantity — build output is reclaimed by predicate and a build is bounded before it starts _(proposed)_
- **ADR-0054** A commit message is a judged value, not a convention and not a prompt _(proposed)_
- **ADR-0053** An identity is allocated above the high-water mark, and a collision is refused on the branch _(proposed)_
- **ADR-0051** The minimum release version is measured from the public contract, and conventional commits are demoted to evidence _(proposed)_
- **ADR-0048** A rule declares how it is enforced, and review is one of the three ways _(proposed)_
- **ADR-0047** The session domain is typed, its identities are not interchangeable, and closing is exactly once _(proposed)_
- **ADR-0044** Cooperation is repository-wide because the board is gathered, not because the server is shared _(accepted)_
- **ADR-0043** Entering the repository brings its runtime up, through one bootstrap command the entry file calls _(proposed)_
- **ADR-0041** Proof is a recorded execution, and inputs unchanged is not proven _(proposed)_

### Added

- **adr**: an allocation names what is outstanding, and what it cannot reach (`595a5aa6d`)
- **coverage**: the changed-code coverage gate of #214 (`dbb6c26cf`)
- **reap**: disk is the fifth quantity — reclaim landed build output, bound a build before it starts (`f4c320de6`)
- **commit**: the commit message is a value this repository judges (`b59988774`)
- **finish**: a session does not finish behind its own published site (`043c5c85f`)
- **site**: the navigation is declared once, and every projection derives from it (`a8e002b5b`)
- **site**: nine top-level categories become five intents and one action (`8357b37c9`)
- **docs**: every hand-drawn diagram becomes mermaid, and the baseline reaches zero (`e51600d73`)
- **docs**: an architectural relationship is drawn, not typed in box art (`cbe3e3917`)
- **adr**: allocate an identity above the high-water mark, and refuse a collision on the branch (`3733e94ad`)
- **merge**: the merge policy is a projection of what the generators write (`d4325fafd`)
- **release**: the public contract decides the version, not the commit subjects (`ea05dc261`)
- **plan**: an issue moves through the lifecycle as a capability, not only a command (`a946ae06d`)
- **rules**: a rule declares how it is enforced, and review is one of the three ways (`080349f5b`)
- **backlog**: bound the four quantities that grow on their own (`d65cc145d`)
- **capture**: a prompt record carries its episode, and the archive is private and bounded (`842f93159`)
- **gates**: the done invariant, answered from what already knows (`38157e00b`)
- **gates**: completion is a state the repository decides, not a claim (`6f6293ebc`)
- **peers**: the board is the repository's, gathered from every checkout (ADR 0044) (`853bf6369`)
- **session**: the session domain is typed, and closing an episode is exactly once (`acf06af46`)
- **models**: the model catalogue is one declaration, and routing explains itself (`387e22dcf`)
- **mesh**: every surface projects the one mesh runtime, off until declared (`21aed93fd`)
- **rules**: project.a-verdict-states-its-subject, with its case and its CI gate (`ad4d19c0e`)
- **entry**: entering the repository brings its runtime up, through one bootstrap command (`ad434114b`)
- **ci**: a promise names a proof a machine can follow (`faa9107a4`)
- **session**: the subsystem gets a reading of itself, and the docs stop describing a lifecycle it does not have (`bf51ca0dc`)
- **evidence**: a claim is proven by a run that happened, not by a file that exists (`a65a21951`)
- **mesh**: the mesh core — identity, protocol, registry, trust, providers (`e1cb97992`)
- **recover**: close episodes nobody ended, and stop records claiming the tool's exhaust (`9d87da3b1`)
- **rules**: a rule names what proves it, and the naming is load-bearing (`c2efe3368`)
- **session**: the briefing an episode opened with is checked against git (`c9f40810f`)
- **devcontext**: compile a session's context from the repository, structured (`802dfb670`)

### Fixed

- **test**: a dedup assertion compares two lengths, and could never pass (`75f09c04c`)
- **site**: the version is visible on a phone, not only from 640px up (`df4cfe7e3`)
- **cockpit**: the mesh reason sits beside the facts list, not inside it (`275ffbe3c`)
- **recover**: a stale staging directory is reported and left, not deleted (`ce7339594`)
- **docs**: every intra-doc link resolves, so the crate documents itself again (`5a81aa0c2`)
- **commit**: the commit subsystem orders through one place (`0384f21b6`)
- **docs**: an example path is not a machine path (`dc1a18f92`)
- **environment**: entry draws one banner and never names a server it just replaced (`ff36cacfb`)
- **commit**: a record id in a code span is shown, not cited (`d5efa5cdd`)
- **records**: a record reads the whole declaration, and absence is not damage (`60326fcb0`)
- **capture**: capture status names the providers it has no adapter for (`f258a0e75`)
- **ledger**: the published order and the envelope are facts, not accidents (`ac4e27037`)
- **pages**: the age of an owed publication is the oldest debt, not the newest (`e0c309df3`)
- **site**: a collapsed badge is not a gone badge (`0c05c07da`)
- **ci**: the gate dispatcher clears a share that names another checkout (`eeb5797d4`)
- **site**: the generator supplies the raw wrapper, so the source must not (`1c8548673`)
- **evidence**: the ratchet refuses a lost proof, not a new claim (`454d7a908`)
- **schema**: a transparent newtype input projects the shape it wraps (`bfa04d4c5`)
- **executor**: the handler counter counts every handler, at every depth (`b8f6606b2`)
- **liveness**: six waits are bounded, and the supervision window counts code (`139558f2e`)
- **github-sync**: a human-written title is not a record id (`6c291d082`)
- **development-semantics**: a finding key is an identity, not a coordinate (`5a39369fc`)
- **site**: the two diagrams the widened gate found — back to a measured zero (`8021a7e27`)
- **git**: a read must not move the repository (`d39cf280d`)
- **ci**: diagram-check was honest about a corpus it did not look at — 0 becomes 2 (`706214a63`)
- **claims**: the seventeen guaranteed claims whose test never named them back (`ed077cb13`)
- **site**: the matrix is shown whole and the questionnaire is grouped (`d098c88f9`)
- **commit**: the history benchmark case runs in a repository with two commits (`5957bbc78`)
- **commit**: the use case asserts what it is about, and not that the repository is healthy (`b747440af`)
- **capture**: the archive mode is read GNU-first, so Linux reads a mode and not a filesystem (`ff00a4b9c`)
- **commit**: the crate's own lints, at the pinned toolchain (`f9c4da433`)
- **rules**: the vendored package installs the doctrine, and no count is written by hand (`b8a7a07d1`)
- **pages**: a run that published is no longer indistinguishable from one that did not (`32c6fb606`)
- **gates**: one reader per property, and a citation that survives a rename (`fffcd32b4`)
- **commands**: every command says once whether it is a capability or why it is not (`c90b8c66d`)
- **gates**: one verdict on enforcement — the duplicate rule reader is removed (`cdb25a2cc`)
- **site**: two headings that boasted, dated, or were written twice (`18d58852d`)
- **lint**: five dead names removed, one directive restored, one exit code put to work (`ea19b5ad2`)
- **ci**: two pipelines stop hiding their own failures (`7a0747e6a`)
- **ci**: the order ratchet goes back to 92; raising it was moving the goalpost (`b45e86e28`)
- **test**: the gate is executed, not handed to sh, and a rule's citation follows its file (`f7f0aad3b`)
- **cockpit**: the page CI actually renders is the refusal, and it overflowed too (`cc87fc11e`)
- the ADR statuses I had no right to write, and a 320px overflow (`2e0bd9341`)
- **release**: the three defects CI found, two of them mine (`4fe78bf96`)
- **release**: the writer stays usable where no contract can be measured (`f562bf5cb`)
- **lint**: the newer clippy is answered, not baselined (`0aef4f140`)
- **merge**: restore master's files the merge had reverted (`7928fbe50`)
- **site**: the API page has one title, and a scenario's record stops depending on the date (`ccc1404d1`)
- **session**: an example path stops looking like somebody's home directory (`11e6749df`)
- **rules**: three blocking rules name the case that proves them, and the derived-merge glob stops eating hand-written schemas (`0cc5a114e`)
- **schema**: restore the policy schema's prompts block the derived driver dropped (`ec680346d`)
- **adr**: the session domain decision moves to 0047, because 0044 was claimed twice (`5ba6dd15e`)
- **test**: case 133 builds its fixture in the harness's repository (`94fad98c5`)
- **rules**: three references the merge and the rename left pointing at the wrong thing (`b5e1c628e`)
- **ci**: the pages paths follow the model, and 09 stops depending on run order (`f6b35877b`)
- **usecase**: a compressed size is decided by the machine that compressed it (`456f3b647`)
- **test**: the help assertion holds the claim that is true now (`d57b3e1dc`)
- **test**: name the peer-board tuples so clippy's type_complexity passes under -D warnings (`d8ed512bd`)
- **evidence**: a verdict carries the denominator it was reached over (`7837275c7`)
- **rules**: yield ADR 0047 and case 134 to their earlier claimant; take master's release fix (`a24629655`)
- **merge**: the design system's ten projections resolve on merge (`cdac4f5b4`)
- **release**: the release schema declares the key the release automation writes (`b9c91d9e8`)
- **release**: the schema is told about the field its writer and reader already share (`65fb52d6d`)
- **evidence**: three defects review found in the fabric's first cut (`3a20fe9a1`)
- **gates**: answer where the model is absent, and add no debt to three ratchets (`3f987bde0`)
- **merge**: declare the thirteen derived site/content trees the generator rewrites (`7444f0209`)
- **recover**: stat -f is a different flag on GNU, and the event was undocumented (`0eabc4109`)
- **ci**: rule-proof-check pipes under pipefail, and counts once (`29329afee`)
- **entry**: a server entry starts inherits none of the caller's descriptors (`69fd5641a`)
- **records**: a killed writer no longer leaves its staging file in a tracked section (`613608904`)
- **order**: the ratchet counts what ships, and a jq filter is not sort(1) (`9d57fe1ab`)
- **claims**: ci-verdict asserted a branch rule that does not exist (`6e9f223f0`)
- **doctrine**: `doctrine status` reports the registry beside the set it is a subset of (`8b7043366`)
- **ci**: a cancelled job says why it carries no verdict, and the bound cites its measurement (`29a9e38dc`)
- **ci**: three ratchets that were counting something other than their subject (`02fd25c41`)
- **cockpit**: the two findings this branch added to the structure gate (`6c1cb8634`)
- **rules**: the rule that landed while this branch was open names its proof (`c6977d99c`)
- **ci**: pin the toolchain, and four more gates that could not run (`04320b329`)
- **doctor**: the rule and doctrine verdicts state their own denominators (`815116686`)
- **cockpit**: the graph drawing is chosen from the graph's shape, not one name for all eight (`3b1c1a6d3`)
- **rules**: the cases and the governing rule describe both enforcement modes (`83198d51d`)
- **pages**: an errored Pages build is a failed deploy, and nothing local is published (`5b59e18ce`)
- **session**: the lifecycle is the episode's, not the task's (`d5d541850`)
- **ci**: a gate CI cannot run, three fixtures that inherited the machine (`5544c1afb`)
- **server**: the shared server follows the repository instead of freezing it (`a8adc5a56`)
- **ci**: the gates that were red on master read the evidence they meant to (`b7eee54a2`)

### Performance

- **policy**: the doctor and watch budgets are derived from runs, on both their subjects (`66fef5f21`)

### Changed

- **site**: the homepage is nine sections, not twenty-two (`ac1e6e32e`)
- **order**: fifteen collections adopt the canonical order, and the baseline falls to 90 (`eb7f1fad2`)
- **order**: ten of the collections that arrived ordered beside their renderer (`bb4abafd8`)
- **order**: fourteen collections take the canonical order, and the baseline falls to 92 (`4803476f2`)

### Documentation

- **commit**: the subsystem's three new exported items carry an executable example (`212935e02`)
- **quality**: the new surface documents itself (`9d1734595`)
- **commit**: the architecture diagram is drawn, not typed (`4e57c3675`)
- **commit**: the timings are measured, and the hot paths are measured not to pay them (`76a1615d8`)
- **commit**: every exported item of the subsystem carries an executable example (`ce1d5f2fb`)
- **rule**: say why check does not ask the publication gate (`2b0abbc37`)
- **plan,ledger**: an executable example on every public item this branch adds (`dadf7ab36`)
- **mesh**: the claim pages carry their full contract, and the site tells the story (`60037f4ce`)
- **evidence**: an executable example on every exported item of the fabric (`c4cd7cf9b`)
- **claims**: the detail pages for the split ci-verdict claim (`672f08fa2`)
- **order**: the rule says what the shell ratchet does not count (`86469fbbf`)
- **cockpit**: the gap matrix is re-verified and the gaps are in the plan (`34794c73e`)

### Tests

- **commit**: the three claims carry a recorded execution, not only a named test (`3239df657`)
- **budget**: the two cases stop naming the budget value they widen (`0f1aa6a11`)
- **commit**: the fixture's temporary worktree is named after the case it belongs to (`b7082d8aa`)
- **rust**: every exported item of the rule and evidence modules carries an example (`8b6a844af`)
- **release**: the writer and the schema are tied, so the next field cannot slip through (`53d995eff`)
- **design**: case 107 asserts the graph viewer's exemption is gone, not that it holds (`965d4df5d`)
- **session**: the subsystem is covered, and the coverage says what it means (`2783a5556`)

### Housekeeping

- **derive**: the projections follow the merge with master 33a792ec7 (`4776118c0`)
- **derive**: the projections follow the merge with master 19b275520 (`7406c6638`)
- **derive**: the projections follow the merge with master a77c35a74 (`c07c510f4`)
- **derive**: the projections follow the merge with master df244393d (`0995772eb`)
- **derive**: the projections follow the merge with master (`c668bbbb0`)
- **derive**: the projections follow the merge with master 623ff738e (`26818260e`)
- **derive**: the projections follow the merge with master 687a23f4d (`555d0887a`)
- **derive**: the projections follow the merge with master cd04012f1 (`dd5f81493`)
- **derive**: the projections follow the merge with master 5e31dd498 (`02d3212c0`)
- **derive**: the projections follow the merge with master 33806c04e (`74430913e`)
- **derive**: the projections follow the merge with master 9f7644fb0 (`c7aa6c32b`)
- **derive**: the projections follow the merge with master 958bd1fb7 (`c8fca0049`)
- **derive**: the projections follow the merge with master 958bd1fb7 (`034ffaa4c`)
- **test**: cases 278 and 279 become 281 and 282, and the merge with master 860951619 (`ae3320046`)
- **derive**: the projections follow the merge with master 860951619 (`0dfb1aa4e`)
- **derive**: the projections follow the merge with master 860951619 (`4b531558e`)
- **derive**: the projections follow the merge with master 860951619 (`9b754cd3a`)
- **derive**: the projections follow the merge with master be8b66672, and case 278 becomes 279 (`7d225fe35`)
- **derive**: the projections follow the merge with master 6739f4668 (`0fb04b2c6`)
- **derive**: the projections follow the merge with master 593de20f8 (`490818e42`)
- **derive**: the projections follow the evidence gate (`860132db0`)
- **derive**: the projections carry the two devcontext parameter lists (`eb3efc3f3`)
- **evidence**: the baseline records both halves, at the number the corrected predicate earns (`c1299e268`)
- **development-semantics**: the baseline retires the two entries that closed (`5bd5016ca`)
- **derive**: refresh derived data a stale merge left on master (`1e3b47e04`)
- **derive**: projections follow the merge of master (`d1c4d063c`)
- **derive**: the projections follow the merge with master e2de2e5ec (`23fc77fcf`)
- **coverage**: mark the differential gate work under task t-20260912105728-7385 (`9f7aea390`)
- **derive**: the projections follow the merge with master f9bee12bc (`1fb0d74ea`)
- **derive**: the projections follow the merge with master 04d9910e4 (`120affcdf`)
- **derive**: the projections follow the merge with master 3407ca2d4 (`b1d71d0d2`)
- **derive**: the projections follow the merge with master f22f34dde (`e9031a9b7`)
- **site**: the doctrine reaches the site as a page, like every other (`ce767b18a`)
- **derive**: the projections follow the merge with master b8d3eb59b (`aa8b2e78a`)
- **derive**: the projections follow the merge of master (`d80967756`)
- **derive**: the projections follow the navigation change (`b2d4efe90`)
- **derive**: the projections follow the diagram conversion (`dd67c01d4`)
- **derive**: the projections follow the merge of the allocator and the merge policy (`16c83c67f`)
- **derive**: the projections follow the merge with master (`a523bcf5f`)
- **derive**: the projections follow the fifth batch (`966b141ca`)
- **derive**: the projections follow the fourth batch (`c099fadf0`)
- **derive**: the derived tree for the merge of master (`187bc625c`)
- **derive**: the projections follow the third batch (`445e783ed`)
- **derive**: the projections follow the batch of seven (`870825d9a`)
- **derive**: the projections follow the merge of master (`b5736b605`)
- **derive**: the projections follow the fifth merge of master (`efddd1289`)
- **evidence**: the first recorded runs, and six claims that are proven (`36a6f1da0`)
- **derive**: the registry dataset follows the fabric's new documentation (`3d3cdcc27`)
- **derive**: the projections follow the third merge of master (`b45d52718`)
- **release**: record v0.5.0 and the metadata it publishes (`8128683c0`)
- **derived**: regenerate after merging master (`4d40de2a7`)
- **derive**: regenerate over master@3a032c1e0 (`f320b46b7`)
- **derive**: the projections follow the second merge of master (`e3ee16de3`)
- **derive**: the projections follow the merge of master (`cbc2c72cd`)
- **derive**: regenerate the projections over master@87b9ac659 (`fbea3d9dd`)
- **derive**: regenerate the derived artifacts for the merge result (`2c3d3a75c`)
- **derive**: the projections follow the merge with master (`d6b9d90d6`)
- **derive**: the projections follow the merge of master (`7cb8f8e6c`)
- **quality**: the baseline stops accepting three findings that no longer exist (`c7f47c667`)

### Formatting

- **rust**: the two test files rustfmt still rewrites (`67f6f66de`)
- **rust**: format the crate, and name the type clippy could not read (`c15e45aa5`)

### Integration

- **commit**: bring origin/master 3407ca2d4 into feature/the-commit-is-an-intent (`16eac16e7`)
- **commit**: bring origin/master 98a1f0800 into feature/the-commit-is-an-intent (`bd9be64a8`)
- **entry**: take master's batch of seven, restoring the policy schema's prompts block (`77225cbb9`)
- origin/master (#221) (`fe66b4b7e`)

### Other

- Merge origin/master into feature/canonical-versioning (`73386b837`)

### Published

- `macos-aarch64` — majordomus-v0.6.0-aarch64-apple-darwin.tar.gz (`3efdd7125063`)
- `macos-x86_64` — majordomus-v0.6.0-x86_64-apple-darwin.tar.gz (`a7801c74905b`)
- `linux-x86_64-gnu` — majordomus-v0.6.0-x86_64-unknown-linux-gnu.tar.gz (`9b77678d697c`)
- `linux-x86_64-musl` — majordomus-v0.6.0-x86_64-unknown-linux-musl.tar.gz (`300c92c9c0c7`)
- `linux-aarch64-gnu` — majordomus-v0.6.0-aarch64-unknown-linux-gnu.tar.gz (`d7415dbfb6cc`)
- `linux-aarch64-musl` — majordomus-v0.6.0-aarch64-unknown-linux-musl.tar.gz (`01521975b729`)


## v0.5.0 — 2026-09-11

### Decisions

- **ADR-0040** Development semantics are capabilities of one runtime; every surface is a consumer _(accepted)_
- **ADR-0039** Liveness is four rules, advisory until a scan can decide them _(proposed)_
- **ADR-0038** A scenario declares where it runs, and an obligation is a step _(proposed)_
- **ADR-0037** A gate whose runner the repository cannot get on demand is planned on demand, so that the verdict arrives _(accepted)_
- **ADR-0036** The design system is a typed declaration in the crate and every first-party surface consumes its projections _(accepted)_
- **ADR-0035** A checkout's server is one of the repository's, and its state is served like every other fact _(proposed)_
- **ADR-0031** The design is one declaration and every surface is a projection of it _(accepted)_
- **ADR-0030** A task owes obligations, and the evidence that discharges them goes stale _(proposed)_
- **ADR-0029** The changelog is a projection of what the repository already records, and the version has one writer rather than one source _(proposed)_
- **ADR-0026** A capability module's area is derived from the features that name it, not declared beside it _(accepted)_
- **ADR-0025** Presentation order is one total order owned by the crate, not a decision each surface makes _(accepted)_
- **ADR-0024** An orchestrator is a provider only at the bootstrap level, and its worktrees are scratch checkouts _(accepted)_
- **ADR-0028** The crate's public surface is measured by the crate, and absence from the API is a decision _(accepted)_
- **ADR-0027** A command is declared once and every surface is a projection of the command graph _(proposed)_
- **ADR-0023** Product features are objects of the layer, and the landing page is a projection of them _(accepted)_

### Added

- **archive**: a snapshot of the tracked tree that can leave the machine (`71d716cc1`)
- **site**: the landing page is the intent-driven development argument (`a47f93ef9`)
- **devtask**: a declared session link is derived, a branch guess is inferred (`b08d5de58`)
- **devtask**: issues and milestones as executable development scopes (`2c3749fc3`)
- **peers**: a peer holds more than one claim (`83f75b088`)
- **rules**: the liveness doctrine lands as four rules (`bb8e5d76b`)
- **entry**: a rule and a gate hold repository entry shut (`b59009b22`)
- **site**: the method as a projection — governance layers and the four axes (`bd367dd0b`)
- **site**: the site is published at majordomus.dev (`ef96b2351`)
- **release**: the version is measured against the public surface, not claimed (`303dca256`)
- **use-cases**: a scenario declares where it runs and an obligation is a step (`8e2c1afd7`)
- **design**: the gate asks whether a declared colour can be read (`61e2989c7`)
- **session**: one episode per provider session, not one per checkout (`e91a07d5b`)
- **design**: measure the declared colours against WCAG 2.1 AA (`5f646e867`)
- **ui**: audit every surface the executable serves, not only the built ones (`b80b1d2de`)
- **health**: the report names the server, and readiness stops meaning three things (`eb00abc10`)
- **integration**: scripts/unblock, the local master merge as one command (`5525f4b9b`)
- **mcp**: the shared server's lease has one reader, and a gate that keeps it (`534e8fb7b`)
- **mcp**: an announcement outlives the server it was made to (`f200a90fe`)
- **design**: the design system is a typed declaration and every surface consumes its projections (`95ffb1b7c`)
- **serve**: entry converges — ensure, status and stop, the start event calling ensure, and the election under a slow start (`4a4a945d5`)
- **server**: a checkout's server is one of the repository's, and its state is served (`7c4bc6a1b`)
- **release**: the changelog links where it can, names its producer, and the published site is gated (`efebf080c`)
- **obligations**: the outer obligations are established from git and the published site, not typed into a ledger by the worker who owes them (`f2000a52c`)
- **obligations**: what a task owes, and whether its evidence still describes this tree, over every surface (`dc14dd0d3`)
- **rules**: an empty result and a failed command must not look alike (`824223cb5`)
- **ci**: three failures of one session, turned into checks (`1e96e91b7`)
- **plan**: one issue walks the whole chain and back, and a remote close still does not complete it (`67d6298c2`)
- **context**: the board reaches the worker who never asked for it (`8ad5bb6a7`)
- **plan**: the plan's derivations are capabilities, so a server can answer what to work on next (`e41faa62d`)
- **peers**: a session is told it has not announced, and doctor names the work that exists on one disk (`1487d729e`)
- **trace**: an issue leads to the commits and pull requests that realised it, and back, without either being written down (`3831e151c`)
- **design**: one declaration for the design, and every surface projects it (`2b3b9fffa`)
- **closure**: a task owes obligations, and the evidence that discharges them goes stale (`cf1ee94ca`)
- **peers**: an announcement outlives the connection that made it, and a claim is answered rather than merely recorded (`6aeec67b1`)
- **ci**: a gate says the published site came from master, and is being served (`fbc1da919`)
- **release**: the version rises to 0.4.0, and the projection gains its page, gate and record (`957a6ff5a`)
- **release**: the changelog is a projection, and the version is derived from the commits (`82583eba1`)
- **ci**: the GitHub projection has a gate, so it cannot rot green again (`597ebe4e5`)
- **skills**: a description that does not tell two skills apart is a finding (`7af039a4a`)
- **plan**: the projection to GitHub becomes a gated, identified, traversable milestone (`209a82c30`)
- **completion**: the shell integration installs itself, and takes itself out again (`2a7af4272`)
- **workspace**: a workspace is a kind, and only its content is local (`f9bbc636a`)
- **commands**: a surface that withholds a command says why, including the last silent case (`26743dac5`)
- **product**: a capability module's area is derived from the features that name it, and the sidebar groups by it (`1c36ca9a2`)
- **order**: one canonical presentation order, total and natural, owned by the crate (`d7df74e93`)
- **providers**: the provider table is a generated document, a rule and a gate hold every document to it, and a job runs the gates its model assigns (`798a58094`)
- **providers**: what a provider is beyond its template is data, and bb is the first orchestrator declared (`c1666878c`)
- **just**: the distribution group delegates, it does not reimplement (`4f5cc1b07`)
- **quality**: the measurement is a capability, and the rules that hold it are the layer's (`578ff6545`)
- **cli**: every command says whether it is a capability, or why it is not (`db8b6f227`)
- **quality**: the crate measures its own public surface from its syntax tree (`94b0c22fc`)
- **commands**: the palette, fish, and the cost measured rather than claimed (`8e7faed93`)
- **product**: product features are objects of the layer, and the landing page is a projection of them (`675068f01`)
- **execution**: an execution is a watched capability call, not a second registry (`65dded963`)
- **commands**: one canonical command graph, and every surface derives from it (`427b73248`)
- **capabilities**: projection closure as a capability, a gate and a ratchet (`d05b1eafe`)
- **capabilities**: the command line is compared with the registry, not with itself (`d2ac127a7`)
- **cockpit**: the sidebar reads alphabetically (`8518f00dc`)
- **web**: every projection route carries its availability, and the served-only ones are checked (`ade9050e6`)
- **env**: a direnv adapter that carries no repository logic, and a modular justfile (`d648e974c`)
- **cli**: majordomus env, and the split between what may build the index (`c6bbce3a2`)
- **env**: one typed snapshot of what a checkout is (`aa64215da`)
- **generate**: the composed graph is an artifact of the one plan, with a schema and nothing private (`674b89907`)

### Fixed

- **archive**: the archive command's pipelines stop hiding their own failures (`60a04f33f`)
- **ui**: the pin test counts the foreign-surface marker the audit added (`4e04aa6ff`)
- **site**: the landing page stops stating the unbuilt half as present fact (`87b9ac659`)
- **site**: the landing page's own gate stops claiming a check it does not run (`46fd96bd2`)
- **schemas**: the subset is a subset, and a gate asks an independent parser (`4e48a5677`)
- **mcp**: a server that lost its lease says so, and takes on nobody new (`f245eb630`)
- **capture**: a provider shim declares whether the person waits for it (`cb4207874`)
- **site-probe**: the sweep serves the site it measures, and the guard cannot pass on a listing (`67c3c2405`)
- **test**: every wait in this repository's shell carries a bound (`d66904da8`)
- **site**: a use case carries the same shape whether or not it declares a list (`4a87331cf`)
- **pages**: report the deploy that did not happen (`b0b095709`)
- **site**: the group's self-link carries a target of its own (`8b9409897`)
- **doctor**: the vendor's evidence is verified where it lives, not in the repository that adopted it (`c99aee554`)
- **session**: one episode writes one record, and an uncommitted one is visible (`a038591b4`)
- **capability**: environment.status answers with a clock in it, so it is not cached (`0839627b7`)
- **site**: the base path the old address had, and the file the new one rests on (`05fdb333a`)
- **site**: the link to a changelog group is beside its disclosure, not inside it (`a0d6b0f90`)
- **release**: one group per kind, so the changelog stops repeating itself (`0799bc91f`)
- **build**: cleaning stops at the checkout that asked for it (`e02305e24`)
- the site builds again, and clippy reaches the tests (`28a68421b`)
- **site**: a deploy that lost the CNAME is refused rather than published (`92c0098a3`)
- **task**: obligations gain a producer, and an honest outcome is no longer stranded (`05ee9dbd8`)
- **ci**: the implied level is a string, and shellcheck says so (`d40b236e5`)
- **generate**: a derivation refuses an executable of another generation (`9a34710c6`)
- **ui**: the audit measures the served surfaces it can now reach (`e798d42f5`)
- **site-probe**: the walk that discounts a clipped element is asked about the element (`0ece4be20`)
- **plan,release**: a repository with no plan or release still sets every parameter (`2b59268ec`)
- **commands**: evidence declares a class and a task requirement the vocabulary holds (`79ce87d97`)
- **site**: the navigation row wraps, so no page is wider than the viewport at 768px (`fcb70bb30`)
- **security**: the peer-board request is a declared exception, not an undeclared one (`f66418147`)
- **ci**: a gate whose runner is scarce is planned on demand, so the verdict arrives (`ff921a93f`)
- **execution**: the queued transition is published before anything may start the work (`759307be5`)
- **session**: the server line reaches the briefing as an argument, not as a shared name (`8b6a03f98`)
- **order**: every shell sort is pinned with LC_ALL=C, and the debt is zero (`ad9ff819d`)
- **docs**: a public item's documentation stops linking to a private one (`efdcefb3c`)
- **generate**: the composed graph is a generated artifact again (`e380088ee`)
- **capability**: a handler answers from the distribution the process located (`861f81a5a`)
- **site**: the page never scrolls sideways to reach one identifier (`48c445170`)
- **ci**: the changelog is checked against the history it is composed from (`fe5fdc91e`)
- **design**: the gate reaches its own implementation, and the crate's new surface stays inside the crate (`4b7122a73`)
- **ci**: the ratchets and projections master raised without noticing (`b994eeaa9`)
- **pages**: a short sha names the commit it abbreviates (`873c9e1ac`)
- **design**: file the data-page vocabulary and tag an unfiled data word (`13b3abc33`)
- **ci**: the two ratchets the branch moved, moved back (`a3cd91399`)
- **release**: the command line is accounted for once, and every new export shows itself (`da4f72511`)
- **tests**: four examples and one assertion catch up with contracts that already changed (`91f9fb851`)
- **execution**: a cancellation after a failure keeps the failure, said as a guard (`ce40e2b49`)
- **context**: a peer that said nothing no longer ends `context` with nothing said (`89c814490`)
- **ci**: the structure job is green on two of its three gates (`b6743dc39`)
- **release**: the committed changelog stops describing the commit it is in (`ebc10de4d`)
- **ci**: the probe reads 426 from the WebSocket route as a surface answering (`7af40d230`)
- **ci**: master is green again on the three causes nobody owned (`6c979b376`)
- **site**: two checks meet new content and stop reading it as an inventory (`709407a1d`)
- **worktree**: a new path starts approved for direnv (`b3ffee4e9`)
- **obligations**: the pathspecs an obligation is hashed over reach every depth (`c5268505b`)
- **scope**: a task that merged the trunk has not touched what the trunk touched (`83775c7ab`)
- **derive**: where cargo builds is asked, not assumed (`bd63891d9`)
- **ci**: the gate model parses again, and the gate a merge dropped is declared again (`d7f492294`)
- **site-check**: a route is named on the API page whatever it is styled with (`9fcd06f8f`)
- **runtime**: a stale runtime says so, on the lease and on the way in (`53b21b4bb`)
- **tooling**: two failures of this session become refusals with tests (`b90e0dd47`)
- **release**: the module asserts the two projections it documents (`98f6606ce`)
- **gates**: a derivation refuses a degraded index, and one pass says everything a command still needs (`f6b73b05a`)
- **design,adr**: the three failures this session hit, each with the gate that would have caught it (`1cbb36869`)
- **traceability**: an open pull request keeps the columns after its empty ones (`efe5afdb4`)
- **commands**: the module asserts the three projections it documents (`5609ab148`)
- **site**: the changelog page reads the groups the document carries (`b551a3327`)
- **doctrine**: the schema check reads the schemas, instead of reporting a clean bill of health over an empty set (`0ad022303`)
- **docs**: the semantics module documents a validation that is not an item (`ed649c769`)
- **share**: a MAJORDOMUS_SHARE naming another worktree of this repository is refused (`fc9864ebc`)
- **derive**: every stage names itself as it begins and reports what it cost (`59aa3769c`)
- **ci**: ci-plan tells a bare path list which spelling it wanted (`5bd2f77bc`)
- **cli**: an unknown command that the Rust executable provides says so and names the launcher (`59f5c8b75`)
- **github-sync**: the protection against overwriting a person's edit was blind when the plan had moved (`21f7ad848`)
- **github-sync**: a projected record carries its own identity, so a rename cannot orphan it (`3db47f7d6`)
- **ui**: a scroll container is a containing block for what it positions, and the finding that says so names it (`2d328460b`)
- **ci**: a job that hangs fails instead of holding a runner for a shift (`1ea8da126`)
- **completion**: TAB in this repository asks a program that can answer it (`ff2d64e57`)
- **order**: the collections that disagreed with each other, and a rule that keeps them from disagreeing again (`28304f300`)
- **ci**: the wiring case asserts the dispatch, and the provider table is a document of the docs target (`38cec32a9`)
- **derive**: the catalogue carries the rewritten summaries, and the crate is formatted (`441aa73f4`)
- **worktree**: the scratch roots are read from the checkout the command runs in (`24a21375f`)
- **usecase**: the tool under test is on PATH for its scenarios, so the evidence is the same on every machine (`28af54468`)
- **site**: the document router parses under bash 3.2, so the macos job runs the site cases again (`1df70e37e`)
- **rust**: the test build compiles again on master, and clippy 1.98 is satisfied (`5540b762f`)
- **merge**: the test-only index literal gains the field master added (`52bfbe7f0`)
- **site**: the mobile overflow findings, and the measure that invented three of them (`93977cce1`)
- **test**: the five suite cases that fail on master, and the silence that hid them (`e5933f485`)
- **site-data**: a scenario failure says which scenario, and case 11 re-pins the package it edits (`82ec36404`)
- **release**: the guard died on the archive it was written to catch (`1ef73612a`)
- **distribution**: a release is complete over the set it was published against (`8c48954e8`)
- **release**: the release asks Pages to publish, because its own commit cannot (`238e95089`)
- **release-verify**: a report a person reads, and no OK it has not earned (`5374ca524`)
- **release**: the packer that lost v0.2.0, and the status nobody could ask for (`84aa37b49`)
- **site**: the dependency-graph links are distinguishable without colour, and the crate is formatted (`bbe74e431`)
- **quality**: the re-export arm is a guard rather than a nested if (`99623c0ac`)
- **test**: case 11 mutates the version the tool has, not the one it had (`6658d2c1d`)
- **test**: case 12c stops testing a subsystem that moved out of it (`5d849ae60`)
- **gate**: derived-current asks both halves, not the one that is cheap (`94eb63142`)
- **quality**: a test builds its report in one initialiser (`95ca55bd8`)
- **quality**: the documentation links only to what the crate still exports, and the renderer is testable (`0f25df64d`)
- **test**: case 12 counts the moments the catalogue features, and matches the tag the build writes (`26d263b2c`)
- **generated**: the typing case asserted more than the rule, and less than it could (`fe4958aa2`)
- **generated**: the two documents nobody published a contract for, and the rule case 52 meant (`2257db97b`)
- **site**: the command route wraps, and the closure is proved where it is written (`348431bc1`)
- **ci**: the install gate gives the deployment the window it needs (`84e8b526d`)
- **product**: meet master where it moved — ADR 23, the shared install block, the UI standard (`6bd67fe57`)
- **product**: the feature route has a benchmark case in a repository with no features (`0b385b67a`)
- **test**: the homepage's moment count reads the catalogue, not a directory that moved (`0fcd91fce`)
- **test**: two site checks that no longer describe what the build produces (`5e04284fc`)
- **worktree**: the container test names no home directory (`efc6a085b`)
- **test**: the duplicate plan test and the unformatted tests a merge left behind (`de52b8497`)
- **cli**: the crate passes fmt, rustdoc and its test build again (`a10f0af08`)
- **environment**: describe the services master serves, from its route constants (`b405705ba`)
- **tests**: master's own two failures, and the size cliff behind one of them (`6815b9bf5`)
- **why**: the acceptance case asserts the schema identity the tool prints (`5314bd6c8`)

### Performance

- **server**: let a caller ask about this checkout alone (`e042a4cac`)
- **context**: a file that cannot be a context document costs no process (`d0bda2132`)

### Reverted

- **design**: the pull request numbers stay as they were written (`2600b2932`)

### Changed

- **entry**: the rule defers the lease to project.the-lease-is-read-once (`9f6926ac0`)

### Documentation

- **audit**: complete phase 01 — gap matrix, drift audit and the phased plan (`b2ebc5fbd`)
- **audit**: phase 01 baseline — the system run end to end, with evidence (`174a1d24a`)
- **architecture**: name the canonical development runtime and make it enforceable (`93c5f3f87`)
- **ci**: where a gate cannot reach, and a milestone for derived-data integrity (`c17b62387`)
- **plan**: the plan meets the tree it is supposed to describe (`a2882e3b6`)
- **BREAKING** **release**: `scope classify` left the public surface after v0.3.1 (`4fcccc69c`)
- **rules**: four rules this day taught, and the two documents that meet them (`6d2c54ad1`)
- **ui**: the skipped-subtree record carries its own example (`7f813a56d`)
- **design**: the rule sets a floor on the value, and the branch that measures it carries the amendment (`f13273f9e`)
- **schemas**: task.evidence is documented where every other event is (`a547850b3`)
- **claims**: two claim documents carry the five sections the contract requires (`20b2f941e`)
- **entry**: the operator's page for entering the repository (`766adaec6`)
- **entry**: the audit of repository entry — what converges on its own and where it breaks (`143af7b54`)
- **bootstrap**: every worker is told it is not alone, and told to push before it is (`e6571df53`)
- **plan**: I1301 is done, and I1302 may not land its schema alone (`012fed644`)
- **plan**: the projection claim splits in two, and only one of the halves was ever true (`990bd0588`)
- **plan**: the external-workspaces milestone and its ten issues (`54c5f45ee`)
- **adr**: the audit withdraws two thirds of ADR 0025 (`7f75414bf`)
- **adr**: an external workspace is not a provider (`a845624ad`)
- **adr**: the synthetic run under bb was done, and it says what archiving a thread does not (`cc6540f2e`)
- **adr**: .mcp.json reaches Claude Code under bb, and the projection baseline takes the parent command the unwired gate let in (`929c29f8e`)
- **adr**: an orchestrator is a provider only at the bootstrap level, and its worktrees are scratch checkouts (`7da5ad9b2`)
- **claim**: the claim says what the gate now does, and where it cannot reach (`d485fe9ae`)
- **worktrees**: the cleanup nobody is prompted to do has a bill (`a3afb1dac`)
- **adoption**: close the warning block, and say apparatus not ledger (`c713bf419`)
- **adoption**: what doctor reports in a repository that is not this one (`7eebc1e9e`)
- **plan**: the deployed surfaces learn who is asking, and the providers are data (`9f973e52c`)
- **environment**: the document the env recipes cite (`97f4cad8c`)

### Tests

- **entry**: the cold start is proven — a storm, a crash, two worktrees, planted drift (`96257970f`)
- **suite**: a waived benchmark is a declaration, so the two cases read it (`bf3f22e0a`)
- **suite**: the staleness case answers for the executable beside the sources (`11263d490`)
- **suite**: the docker context case reads the first rule, not the sixth line (`19fca96a2`)
- **session**: the case takes 111, since 110 was allocated twice (`51aa6e92e`)
- **suite**: the rust-check step list follows the gate that was added to it (`5d72ae69d`)
- **suite**: each stage of the future-command case gets its own repository (`ab00adc8d`)
- **unblock**: the case takes a number nobody else is taking (`d912a58ea`)
- **design**: the zero-registration mutations use a word the declaration does not file (`60b5b2492`)
- **workspace**: the declaration is proved to carry no credential (`3d9c929a5`)
- **order**: the gate is driven against fixture trees, and driving it found five more unpinned sorts (`f6af28819`)
- **gate**: case 56's first export is a repository too, so CI asks the same gate as a laptop (`8b6b9a982`)
- **env**: the two tests the adapter rule names, written (`000e40d67`)
- the recipe set is asked of just --dump, not of one file's text (`010541612`)
- **distribution**: the installability report, over states this repository is not in (`b1be17353`)
- **gate**: the registry half needs a repository, not just a tree (`b7f88c73e`)
- **cockpit**: the sweep renders the routes the dispatcher actually answers (`4fd2d838c`)
- **site**: the feature pages vouch for the claims the homepage used to (`ca45221c0`)
- **execution**: the terminal's rendering is asserted, and a surface count is derived (`9badf2448`)
- **commands**: the generated completion, loaded into a real zsh (`f37da5681`)
- **worktree**: the migration's classification, its guard, and a dirty move (`e4ef4cba4`)

### Housekeeping

- **env**: source machine-local .envrc.local from .envrc and ignore it (`1a651e644`)
- **derive**: the projections follow the merge of master (`83cd83ee6`)
- **derive**: the projections follow the peer claims merge (`702d06d5f`)
- **derive**: the projections follow the quoted scalars (`cf1cc1b54`)
- **derive**: the projections follow the merges master never regenerated (`06fa25891`)
- **derive**: the projections follow the master merge (`8a51ba050`)
- **derive**: the projections follow the master merge (`95b775cec`)
- **derive**: the projections describe the trunk again, so it can publish (`84b90b193`)
- **derive**: the projections follow the merge of master (`2aee5b1c1`)
- **derive**: the site data follows the tree (`08caf51aa`)
- **derive**: the projections follow the merge of master (`4cfd25574`)
- **derive**: the projections follow the second master merge (`e120f1996`)
- **derive**: the projections follow the template repair (`2511a3241`)
- **derive**: the projections follow the master merge (`f27d366d2`)
- **derive**: the projections follow the merge of master (`6042f4286`)
- **derive**: the projections follow the master merge (`bb82e058d`)
- **derive**: the projections follow the last four merges (`940b00262`)
- **release**: 0.5.0, and the projections that follow it (`4cd5cf6ac`)
- **derive**: the projections follow the contrast capability (`89b039165`)
- **derive**: the projections follow the merge of master (`4e015631a`)
- **derive**: the projections follow the registry entry and the claim documents (`a46012d3e`)
- **derive**: the projections follow the on-demand gates and ADR 0037 (`2a6ba6d8b`)
- **derive**: the projections follow the master merge (`3b3ac4313`)
- **derive**: the projections follow the master merge (`1ae462da0`)
- **derive**: the projections follow the master merge (`371e33659`)
- **derive**: the projections follow the second master merge (`47f87721c`)
- **derive**: the projections follow the master merge (`829e9ca5a`)
- **derive**: the projections follow the master merge (`8ea2cbe5a`)
- **derive**: the projections follow the master merge (`998eee46a`)
- an empty commit, to prove a commit no longer owes a derive (`e54def3af`)
- **derive**: the projections follow the master merge (`ec677b281`)
- **derive**: the projections follow the master merge (`04599afd2`)
- **derive**: the projections follow the master merge (`ea5592a9e`)
- **derive**: the projections follow the master merge (`789e95ab8`)
- **derive**: the projections follow the merge of #142 (`c1a3e7930`)
- **derive**: the projections follow the master merge (`08348b0c1`)
- **derive**: the projections follow the merge of #128 (`7c3e9f9ac`)
- **derive**: the projections follow the master merge (`c4b6d0715`)
- **derive**: the projections follow the target-directory fix (`6be9c8080`)
- **derive**: the projections follow master once more (`f36420c30`)
- **derive**: the projections follow the master merge (`8c53e2b11`)
- **derive**: the projections follow the master merge (`a4f9780b0`)
- **derive**: the projections follow the master merge (`16cc159c7`)
- **derive**: the projections follow the merge, generated by a binary that knows every module (`0df29dbcf`)
- **derive**: the projections follow eight branches and current master (`98f3dfbda`)
- **derive**: the projections follow six branches folded together (`e15b311e7`)
- **derive**: the projections follow the master merge (`c114a92f3`)
- **derive**: the projections follow the master merge (`341451cb7`)
- **derive**: the projections follow the master merge (`91b4e8be6`)
- **derive**: the projections follow the master merge (`3a2aeab52`)
- **derive**: the projections follow the merge (`f2b018f26`)
- **derive**: the projections follow the two merges, and the baseline records lib/doctor.sh (`501ea2d41`)
- **derive**: the projections follow the merge of origin/master (`11fc8bf2b`)
- **derive**: the projections follow the master merge, and the baseline records three scripts that arrived with it (`8ddb6496d`)
- **derive**: the projections follow the final master merge (`cdc859654`)
- **derive**: the projections follow the master merge (`5e6d324c1`)
- **derive**: the projections follow the master merge (`0d45da189`)
- **adr,test**: renumber to the identities the board actually left free (`50f276bc3`)
- **derive**: the projections follow the master merge (`21a1203cc`)
- **derive**: the projections follow the second master merge (`e94b760d1`)
- **derive**: the projections follow the master merge (`5fe8f2ec8`)
- **derive**: the projections follow the master merge (`b79363681`)
- **derive**: the projections follow the master merge, with the ADR renumbered to 0030 (`f1c1b6556`)
- **derive**: the projections follow the merge of the base branch (`e4c40b814`)
- **derive**: the projections follow the entry-point fixes (`691fab7db`)
- **derive**: the projections follow the merge with master (`44468dbc1`)
- **order**: the merge brought three modules and fourteen comparators; two are adopted and the baseline says what the rest are (`c72d294cc`)
- **derive**: the projections follow the two adopted sessions (`4066bdf4f`)
- **sessions**: adopt the two episodes this checkout left unrecorded (`1015d8ff8`)
- **derive**: the projections follow the merge with master (`ee9e79b3f`)
- **derive**: the projections follow the five-PR integration, with adr-0028 renumbered (`60bc499e4`)
- **derive**: the projections follow the skill description invariant (`6d19fc7ba`)
- **derive**: the projections follow the merge with master (`d5d95779c`)
- **derive**: the projections follow ADR 0025 (`a3a267e8e`)
- **derive**: the projections follow the merge of #114 (`867f3a9f8`)
- **derive**: the projections follow the merge (`946bc36e0`)
- **derive**: the projections follow the merge (`09d410b5a`)
- **derive**: the projections follow the merges (`ebec65fd1`)
- **derive**: the projections follow the merges (`3bdc000ac`)
- **sessions**: adopt the record the hook wrote on 2026-09-08 (`86b63f649`)
- **derive**: the registry follows lib/context_docs.sh (`f3880a89a`)
- **derive**: the projections follow the merge with master (`914c13f56`)
- **derive**: the projections follow the merge (`004af3b53`)
- **derive**: the projections follow the merges (`e47531a4e`)
- **derive**: the projections follow the rebase onto cdd7f1cd (`41beeb077`)
- **derive**: the projections follow the merge (`6b21a9e85`)
- **derive**: the projections follow the merges (`19fa9d1f1`)
- **derive**: the artifact manifest records the fingerprint the last commit made (`a1b0d2637`)
- **derive**: the projections follow the merge with master (`cdd7f1cd9`)
- **derive**: the projections follow the second rebase (`c217f2359`)
- **derive**: the projections follow the merge (`4cda9f9f2`)
- **derive**: the projections follow the rebase (`2c077cfad`)
- **derive**: the projections follow the merge (`9ee7c2952`)
- **derive**: the projections follow the merge with master (`bebf76f03`)
- **release**: record v0.3.1 and the metadata it publishes (`c2629a26c`)
- **product**: follow master to 1fd5e32e, and take the sharper form of the moment count (`04bbeb483`)
- **derive**: the projections follow the moment-count correction (`015d21dda`)
- **derive**: the projections follow the two check corrections (`6cda9bcc2`)
- **derive**: the use-case evidence re-recorded at normal speed (`3c5705d1e`)
- **derive**: the projections follow (`99ead0772`)
- **derive**: the projections follow the merge (`c85c2cb0b`)
- **derive**: the projections follow the merge with master (`8bafa2854`)
- **derive**: the projections follow the merge with master (`e25278a71`)
- **derive**: the projections follow the merge with master (`2365af0cf`)
- **plan**: I1005 done, with the evidence the contract asked for (`27662495a`)

### Other

- wip: milestone, issue and the CI.md section before the master merge (`7143d199e`)
- Merge origin/master into feature/github-work-graph (`6b5c2eb45`)
- Merge feature/github-work-graph into int/everything, with I1305 restored to its own milestone (`d52f43334`)
- **feature/graph-model**: wip: uncommitted work found in the worktree at the topology migration, committed as it was (`034f334bf`)
- **feature/deployment-core**: wip: uncommitted work found in the worktree at the topology migration, committed as it was (`6579f1978`)
- **chore/derive-master**: wip: uncommitted work found in the worktree at the topology migration, committed as it was (`1410a0c79`)

### Published

- `macos-aarch64` — majordomus-v0.5.0-aarch64-apple-darwin.tar.gz (`cadb5ed2a7a6`)
- `macos-x86_64` — majordomus-v0.5.0-x86_64-apple-darwin.tar.gz (`f04ce1b034bd`)
- `linux-x86_64-gnu` — majordomus-v0.5.0-x86_64-unknown-linux-gnu.tar.gz (`1180951cf5bb`)
- `linux-x86_64-musl` — majordomus-v0.5.0-x86_64-unknown-linux-musl.tar.gz (`420407cc6a47`)
- `linux-aarch64-gnu` — majordomus-v0.5.0-aarch64-unknown-linux-gnu.tar.gz (`2d2465e1ea34`)
- `linux-aarch64-musl` — majordomus-v0.5.0-aarch64-unknown-linux-musl.tar.gz (`1cbc4d05e086`)


## v0.3.1 — 2026-09-08

### Decisions

- **ADR-0021** The branch-to-worktree topology is derived from git identity and enforced everywhere _(accepted)_
- **ADR-0020** The capability graph is composed, not centralised, and projected twice _(proposed)_
- **ADR-0017** An episode that opens is handed what the last one left _(proposed)_
- **ADR-0015** The episode boundary is drawn by the provider, not by the model _(proposed)_
- **ADR-0014** A closed session is a shared object of the layer, not a local note _(proposed)_
- **ADR-0013** Every web surface is discovered from its producer, resolved once, and projected _(proposed)_
- **ADR-0012** The Cockpit is a sixth projection of the registry, not an application over it _(proposed)_
- **ADR-0016** The scenario is a body section, not a header field _(accepted)_
- **ADR-0011** Every directory in the layer carries a contract _(proposed)_
- **ADR-0010** Curated knowledge is one kind with a class, not a second store _(proposed)_
- **ADR-0009** Prompt capture happens below the model, or is reported as unavailable _(accepted)_
- **ADR-0008** Use cases are executable canonical objects of the layer _(accepted)_
- **ADR-0007** Skills are data under the layer's skills section, and every surface that shows one is a projection _(accepted)_
- **ADR-0006** CI planned from one model of what affects what, gates as repository-owned scripts, and the site deployed from the run that verified it _(accepted)_
- **ADR-0005** One projection plan over the canonical inputs, named owners for every truth, and the site as a view of the registry _(accepted)_
- **ADR-0004** One canonical declaration, composed modules, derived projections, and performance as evidence _(accepted)_
- **ADR-0003** One shared MCP server per repository, peers, and clients that start it themselves _(accepted)_
- **ADR-0002** A canonical capability registry, with MCP, HTTP, OpenAPI, Swagger UI, the CLI and the reference as projections _(accepted)_
- **ADR-0001** A Rust executable with a data-driven, read-only MCP surface over stdio _(accepted)_

### Added

- **site**: the install block sold as one command, and /why/ organised by who you are (`933dba91d`)
- **worktree**: the branch-to-worktree topology, derived from git and enforced everywhere (`ad414ce6e`)
- **plan**: adoption as a reversible two-folder contract, decomposed (`fc25581d7`)
- **doctrine**: nothing committed names this machine, and every answer satisfies its schema (`1bf5135fe`)
- **git**: the derived merge driver covers everything and is required (`103b06e6f`)
- **git**: the derived artifacts resolve on merge instead of conflicting (`29a8b2119`)
- **graph**: the layer's kinds enter the graph as themselves, with their own vocabulary (`434d18d72`)
- **capability**: where a capability means anything is metadata, not a condition in a page (`5e297b1a5`)
- **graph**: the references the layer already carries become typed edges (`c5ecfa1f4`)
- **plan**: the capability graph milestone, decomposed into execution contracts (`05e1c378a`)
- **plan**: the Fly deployment milestone, decomposed into execution contracts (`1943519d5`)
- **graph**: one derivation composes the registries that already own their facts (`837f146bb`)
- **web**: Swagger moves to /swagger; /docs belongs to the documentation site (`5cae94754`)
- **directories**: the contract hierarchy on the wire and in the Cockpit (`916a6a7f6`)
- **generate**: a projected JSON Schema says it is projected (`7404c2b96`)
- **generate**: every artifact declares its document, its encoding, its contract and its source (`832fdc997`)
- **site**: the install command is one command, on the page a person lands on (`a13d93e3f`)
- **ui**: hold every page to one audited standard, discovered rather than listed (`12a3adb35`)
- **continuity**: the lifecycle runs itself, and every surface can read it (`13b7d9779`)
- **session**: the episode that opens is told what the last one left (`5901b55b1`)
- **derive**: a continuation record that exists without a worker typing one (`28c93f9e5`)
- **doctrine**: the derived-data rule gets a gate, so staleness stops billing the next person (`01aee7352`)
- **directories**: the contract hierarchy on the wire and in the Cockpit (`16e907fc9`)
- **deploy**: the listen address is one fact with two projections (`58a2c377a`)
- **release**: a tagged release builds every supported target, or publishes nothing (`76b61cdd2`)
- **deploy**: deployment reads are declared once and projected everywhere (`1f0749639`)
- **health**: liveness and readiness are registered capabilities, not router hacks (`527ce5b76`)
- **why**: the map gets its own route, and the reverse links reach further (`8fc26357e`)
- **release**: package, verify, record and simulate a release from the model (`3d6b9cd6b`)
- **distribution**: one canonical model for how this project is shipped (`11b58c6b4`)
- **deploy**: the deployment object becomes typed Rust that refuses nonsense (`7d6d93a82`)
- **capability**: every command module asserts what it declares, and the gate is strict (`7402acf55`)
- **deploy**: a deployment is a kind of the layer, not a provider file (`f9089710a`)
- **directories**: the contract hierarchy on the wire and in the Cockpit (`2f4528a3b`)
- **continuity**: the lifecycle runs itself, and every surface can read it (`8fc689782`)
- **session**: the episode that opens is told what the last one left (`173cca442`)
- **derive**: a continuation record that exists without a worker typing one (`0c5e796cf`)
- **web**: the rule, the gate, and a surface nobody wrote code for (`e98d7c3b1`)
- **rules**: high is a number, and a module composed for nothing (`1624c6031`)
- **rules**: a command's evidence belongs in the file that declares it (`1f5e03a13`)
- **why**: the site becomes a reader of the catalogue, and the invariant is tested (`048ceeb64`)
- **site**: the layer's own memory, on the website, generated from the records (`a3c4ba7a8`)
- **generate**: every artifact declares its document, its encoding, its contract and its source (`54f91fa8e`)
- **why**: one domain model, and every interface a projection of it (`ca6f3615c`)
- **session**: the episode boundary is drawn by the provider, and the working context store finally has a producer (`e9ffa54f1`)
- **web**: the running server answers every static surface, from one arm (`c304c41ef`)
- **why**: the catalogue, from ten moments to the operational surface (`c1c10382f`)
- **why**: the operational moments become objects of the layer (`aadcf67a9`)
- **sessions**: the record's contract, enforced, proved and documented (`092fa19f1`)
- **web**: the reserved names are data, and the validator holds them (`7f910995f`)
- **plan**: the capability graph milestone, decomposed into execution contracts (`2485e9548`)
- **plan**: the Fly deployment milestone, decomposed into execution contracts (`36995bd77`)
- **site**: the web surface as a page nobody maintains (`c386d4b72`)
- **rules**: the web surface has one declaration and two reserved names (`d2408372b`)
- **web**: the topology reaches the page, the API, the docs mount and the site (`e64dadece`)
- **sessions**: a closed episode is an object of the layer, projected like every other (`2d58e3473`)
- **capture**: the record is majordomus.capture/v1, and every prompt is three files (`ced86e655`)
- **web**: the test and benchmark reports, as surfaces nobody registered (`7a3c63a44`)
- **web**: the surface model, discovered from producers rather than registered (`39640ca39`)
- **cockpit**: the registry, rendered for a person, and the graphs it already implies (`b2a092411`)
- **adr**: the threshold for recording a decision, and what a change set reaches (`b92215f10`)
- **site**: the layer's directory contracts as a page, from the tool's own verdict (`8e683b7ca`)
- **adr**: a decision states what it put in force, and the graph reads it backwards (`2e08815a9`)
- **knowledge**: the curated record carries the contract its class declares (`ec10af9ec`)
- **use-cases**: the scenario is a body section, not a header field (`3aec9ce98`)
- **context**: a directory of the layer that says nothing about itself is a failure (`917f086d7`)
- **schema**: state the order the rules resolve in, and hold every kind and schema to being applied (`f50ce0684`)
- **schemas**: a document schema is one file whose identity fixes its path (`bd08d089e`)
- **capture**: keep every prompt as a record and a rendering, described by its own schema (`8d0fa98ff`)
- **adr**: architecture decisions are canonical objects of the layer (`2492d5b7e`)
- **capture**: record the person's prompts from a provider hook, and prove it by running it (`953e21349`)
- **pages**: a generated status document, and a diff on staleness (`9abe90595`)
- **site**: one route per command of the Rust executable, under /docs/cli/ (`ebd6c82ec`)
- **use-cases**: the skills command is named and run by a use case (`1f74291be`)
- **cli**: the clap declaration and its examples are the command line's canonical documentation (`88c15f788`)
- **site**: the contract animation reads at a glance, one task and one verdict at a time (`0f365719f`)
- **skills**: implement, the second skill — files only (`b6fd3b18d`)
- **skills**: repo-review, the first skill (`5fbb704ed`)
- **site**: the skills section is a projection of the skills catalogue (`db20ba32e`)
- **skills**: majordomus skills, the skill-integrity doctrine, and the skill contract (`0df08c76d`)
- **kinds**: the skill kind, its schema and the source classes that discover it (`e7a16fa4b`)
- **site**: each outcome has its page, and the hero cards lead there (`ed1b8f63d`)
- **site**: hero titles at display scale, constant height while switching (`e1be0e93c`)
- **site**: the homepage hero sells the outcome, not the machinery (`0114ddd28`)
- **site**: five more recognition moments under /why/ (`7c5506d36`)
- **site**: one deploy path, scripts/site-deploy, for the Pages workflow and for a person at a terminal (`40ab2fd62`)
- **site**: use-case pages from the objects, with executed evidence and coverage (`5b01ce524`)
- **shell**: doctor checks the scope file and init seeds it (`6ae786602`)
- **scope**: the repository scope as data, read once and applied everywhere (`018ec8b47`)
- **use-cases**: fourteen more use cases, each with a scenario the tool runs (`58c80d089`)
- **site**: five more recognition moments under /why/ (`c937db765`)
- **shell**: doctor checks the scope file and init seeds it (`82f3fa53b`)
- **scope**: the repository scope as data, read once and applied everywhere (`8461a8345`)
- **bench**: per-target allowances in the policy; the process-cold spawn gets twice the room (`ab0c14e1f`)
- **ci**: one model of what affects what, a planner, the verdict, and the gate scripts (`0e6fd975a`)
- **site**: five more recognition moments under /why/ (`04ebc67dc`)
- **site**: the Registry page is a projection of the registry, generated and checked (`c04ce58c7`)
- **generate**: the provider bootstraps are a target of generate, one renderer, checked in CI (`4100d6112`)
- **use-cases**: use cases as canonical executable objects of the layer (`f78d69c6d`)
- **site**: the Rust executable's reference routes, generated from the registry manifest (`bae489275`)
- **site**: the API reference at /docs/api/ and the raw document at /openapi.json (`9cea510ec`)
- **openapi**: tags, examples, per-status responses and one source of prose (`877922140`)
- **generate**: the site dataset carries every surface of the executable; cli.md from the clap declaration (`5cbb3e5e1`)
- **site**: the Registry page is a projection of the registry, generated and checked (`fb09749a5`)
- **generate**: the provider bootstraps are a target of generate, one renderer, checked in CI (`d922fc4fe`)
- **site**: the API reference at /docs/api/ and the raw document at /openapi.json (`4ee4d9880`)
- **openapi**: tags, examples, per-status responses and one source of prose (`e3c79f076`)
- **catalogue**: three use cases and one application for the served layer (`39f16ec45`)
- **shell**: doctor checks the scope file and init seeds it (`6ca2c0eb8`)
- **scope**: the repository scope as data, read once and applied everywhere (`b2bac9be0`)
- **site**: the Registry page is a projection of the registry, generated and checked (`f829f2151`)
- **generate**: the provider bootstraps are a target of generate, one renderer, checked in CI (`bd292ca57`)
- **bench**: the policy gates a percentile only past a sample count; the noise floor is a millisecond (`17c9d01d1`)
- **architecture**: the canonical architecture as doctrine, property-tested, benchmarked and gated (`8bec4025f`)
- **objects**: resolve a majordomus:// URI once for objects.get and the MCP resource read (`1df8c12aa`)
- **perf**: no eval anywhere, the baseline and check proved, budgets proved, and the performance doctrine as five rules (`90362ebaf`)
- **generate**: per-module reference, the benchmark matrix and the registry manifest (`c575b4357`)
- **bench**: the benchmark projection, runners, coverage, baselines and the bench command (`134e3682c`)
- **bench**: every public command benchmarkable from the registry, cold and warm, with a baseline and a check (`0334deff1`)
- **executor**: one execution path with counters, a policy-driven cache and prepared MCP listings (`c5d089104`)
- **capability**: modules compose capabilities, the root composes modules (`01467b30c`)
- **plan**: milestone M004, performance truth, with its execution contracts (`fcab0f888`)
- **perf**: phase timing and work counters behind MJ_TIMING (`e2a00a74b`)
- **mcp**: clients start the shared server themselves; justfile; ADR 3 (`d1f1efbfb`)
- **mcp**: one shared server per repository, peers, and MCP over HTTP (`6ae97b8f0`)
- **schema**: kinds and JSON Schemas read at run time; the shell allow-lists derived from the schemas (`abf50a11c`)
- **kinds**: context documents, claim members, implementation and test files as data; allow-lists scanned at build (`5735bbf28`)
- **cli**: HTTP, OpenAPI, Swagger UI, introspection and generation as projections of the registry (`f32014c30`)
- **capability**: canonical capability model and registry; MCP derived from it (`74fb6c393`)
- **cli**: add the Rust majordomus executable with a data-driven stdio MCP server (`4d59957e6`)
- **context**: adopt scoped context documents in this repository and its skeleton (`aeb931365`)
- **context**: enforce context integrity, prove it, and brief the worker with the chain (`4dad86f19`)
- **context**: scoped context documents — discovery, resolution, validation, impact (`bbb44ed98`)
- **migrate**: upgrade legacy repository layout to .ai (`cf0d73179`)
- **init**: seed portable .ai repository layer (`049de9571`)
- **knowledge**: edges only where a file states one, each naming where it was stated (`9b7320f61`)
- **session**: read closed episodes back, labelled and ordered by what they assert (`4a7c934eb`)
- **knowledge**: nodes whose identity survives a rebuild and whose kind is never guessed (`4fd808379`)
- **site-data**: check the responsibility join in both directions (`47d3e2998`)
- **session**: close an episode into an envelope, attributed by a ledger stamp (`ef65f95f2`)
- **site-data**: responsibility links come from the canonical record (`b34f25077`)
- **knowledge**: discover sources from the repository index, never by walking the tree (`0db0501da`)
- **session**: open an execution episode, and refuse to open a second (`34cd24971`)
- **plan**: M003 — a session is an envelope and knowledge is compiled from what is canonical (`2c4dc6fad`)
- **site**: a responsibility page carries its state, its command, its proof and its links (`82edb74e8`)
- **commands**: the command surface as two doctrines, and the contract where workers read it (`d97985fc3`)
- **commands**: a page per command, demonstrated by the fixtures the tests execute (`410b62c2a`)
- **events**: close the ledger vocabulary, on the way in and on the way out (`4c9c1ab97`)
- **commands**: one owner for the command surface, and coverage computed from it (`13d0242ba`)
- **plan**: M002 — a durable record names the worker that wrote it (`9edba86aa`)
- **plan**: the status vocabulary is declared once and every surface reads it (`98ecb87a1`)
- **plan**: the milestone gate reaches the work inside it (`bd97c4993`)
- **plan**: give every forward roadmap milestone its issues (`b3b106ec6`)
- **site**: a claim page names the milestone that would make it guaranteed (`8b276d897`)
- **plan**: M001 is done — a blocking question survives a handover (`181c86ab0`)
- **blockers**: a question refuses a completed finish anywhere on its branch (`06383f2fc`)
- **plan**: I0102 decides that a question blocks its branch, not only its task (`7bcce2d73`)
- **readme**: the roadmap table becomes a pointer to the projection (`a1288fe9c`)
- **doctrine**: the roadmap is a projection, and a document may not be a second authority (`99a8dc0b2`)
- **catalogue**: use cases and applications as data, enforced like everything else (`9a1a8b6b0`)
- **plan**: milestone acceptance evidence is written by the command, not by hand (`8ce1ac7b2`)
- **site**: the roadmap is the milestone graph, not prose in the README (`c8ac338c9`)
- **site**: an interactive finish contract at /contract/, and two checks that hold it (`b2c62261e`)
- **docs**: the plan's current state as a derived document, and a wider next-ready search (`44f24556f`)
- **plan**: the first real milestone, and the GitHub projection applied for real (`6a6371fca`)
- **roadmap**: milestones become a gated graph, and the roadmap derives from it (`2d17b29a0`)
- **site**: the plan is a derived surface — roadmap, milestones, issues and the DAG (`c27d6f1f3`)
- **site**: moment pages carry their evidence; two checks restated as properties (`019df0af6`)
- **plan**: the canonical project model, its derivation engine and its command surface (`d7b949fed`)
- **site**: the why section becomes data, links both ways, and stops collapsing code (`6780071cb`)
- **site**: the hero shows the contract working; the transcript moves below it (`f27c7aa86`)
- **site**: derive two graphs from the sources and render them interactively (`9cabd296c`)
- **doctrine**: a declared, dispatched and wiring-verified enforcement layer (`1f18a7475`)
- **memory**: context, checkpoint, history, decision, question, prompt, search (`6f0b1ea93`)
- **site**: route audit — funnel homepage, five-intent navigation, limitations and roadmap pages (`f0516e3fd`)
- **site**: rebuild GitHub Pages as a Zola projection of canonical data (`f5cfab37e`)
- **site**: one detailed document per claim, rendered as its page (`0e3cdc1a6`)
- **site**: detailed claim pages, and every claim mention links to one (`16ee5f4a6`)
- **site**: every recognition moment is a case-study page (`322d4d446`)
- **site**: pain-first homepage — recognition strip and before/with comparison (`1d87ca9b2`)
- **site**: every tile is a page (`84af3af7d`)
- **site**: who-it-is-for section from claim-free positioning copy (`9acfd7980`)
- **cli**: region projections and dispatcher-aware wiring (`f58310de0`)
- **site**: migrate the derived site to Zola, Flowbite 4, Alpine and Mermaid (`08c083b48`)
- **site**: derived GitHub Pages site, claims matrix, and pages workflow (`b596dcce2`)
- self-supervision, example walk-through, README for v0.1 (`8d1cae6e6`)
- update, start, check (`e5242b4f2`)
- scaffold entry point, shared helpers, YAML subset parser, test harness (`18c4080de`)

### Fixed

- **distribution**: the release builds on runners that still exist (`98772ecbf`)
- **ui**: the audit follows the topology, and finds the pages it was missing (`0151eaa20`)
- **distribution**: the advertised install command is proved against the published site (`e574d800b`)
- **ci**: the Pages workflow triggers on the paths the gate model now names (`99af7b927`)
- **discovery**: the in-process glob matches bracket expressions, so the ADRs are indexed (`621303ad5`)
- **test**: case 76 says why it failed, and follows the schema path rule (`6b4da355d`)
- **rust**: the four rustdoc errors that fail the gate on every branch (`48299b76f`)
- **ci**: the baseline is named where the tool writes it, and the merge driver is declared once (`d3a36fd25`)
- **bench**: a refusal is a measurement, and only a failure stops the run (`cd836a67e`)
- **distribution**: the release archive carries the launcher the MCP one execs (`5213b79dc`)
- **why**: a repository without a catalogue still has a case for every route (`434437f00`)
- **ci**: rustdoc builds under -D warnings, and two cases read the surfaces as they are served (`0dd8c4ace`)
- **site**: the shared moment card on the audience and area pages, and a command note that folds (`71dd7bd00`)
- **site**: long code tokens fold on a phone instead of pushing the page sideways (`dd5164556`)
- **usecase**: a recorded scenario says nothing the clock decides (`711035897`)
- **ci**: every gate green after the topology landed (`a60b65e04`)
- **ci**: no example path names a home directory, and the merge driver is wired relative to the worktree (`46f55ef8f`)
- **worktree**: a session's scratch checkout is never cleanup-eligible (`db9a5a349`)
- **ci**: the runner's clone wires the merge driver it is checked for (`5a190b041`)
- **rust**: the three lints that fail the gate, written the way clippy asks (`6864d4a5c`)
- **ci**: the runner's clone wires the merge driver it is checked for (`62334ee59`)
- **rust**: the three lints that fail the gate, written the way clippy asks (`5c749b262`)
- **site**: the web dataset carries the site's own version, and a description is not a path (`4c1884e1b`)
- **claims,test**: three claim pages the contract rejects, and a rule that fired on a file (`27af381df`)
- **release**: one entry per path in a release archive (`4db957624`)
- **git**: an agent's worktree is not part of this repository (`8967a8b49`)
- **generate**: a plan writes inside the repository it was planned for (`de76ee617`)
- **schema**: four component names that meant two things each, and one ADR number claimed twice (`58b552932`)
- **generate**: a plan writes inside the repository it was planned for (`6fdfdc4db`)
- **site**: the published contract path points at a file that exists (`fbbf668e5`)
- **release**: publication regenerates every projection of the record (`77681eeac`)
- **distribution**: the public release metadata says it is generated (`2d83344c8`)
- **generate**: one definition of a repository-relative path, and the projections it makes reproducible (`f3be42bca`)
- **tests**: the fixture's own declaration had grown past its size limit (`623970e9b`)
- **ci**: a home directory in an authored document fails where it was written (`0873947de`)
- **web**: a declared surface that was never built keeps its mount, and the tree is regenerated (`2c3be37c9`)
- **plan**: an issue's reasoning stops naming a developer's home directory (`138840b40`)
- **examples**: the walk-through stops naming a directory that no longer exists (`059f04a90`)
- **generate**: a generated artifact names its schema by a path, not by this machine (`e0a5e71f0`)
- **generate**: a provenance banner names the repository, not the machine (`3d99a94cc`)
- **ci**: one declaration per plan output, so the workflow parses at all (`9f0fe56cb`)
- **ui**: the audit reaches zero, and stops attributing its own failures to pages (`2ca03db97`)
- **capture**: capture render repairs the archive it is named to repair (`5ba6d4c52`)
- **capture**: the provider sanitiser took over the rendering's field reader (`a7610c142`)
- **integration**: the defects the merged tree's own gates found (`b43b0b5ce`)
- **deploy**: the deployment schema takes the identity master's loader requires (`1fb7cca3b`)
- **ci**: the Cockpit probe gets the libraries it probes for (`b99d84822`)
- **schema**: the context contract keeps its constraints where they are now declared (`61016a9ba`)
- **ci**: satisfy the gates the branch had not been run through (`76a71048f`)
- **context**: a directory's contract is discovered because it is one (`be874eef8`)
- **site**: the published session projection says nothing true only of its reader (`bdd150c55`)
- **test**: an allow-list is a banner and then patterns (`dcfef63cb`)
- **site**: a skill example's path is a path, so it wraps like one (`40e8c430b`)
- **ci**: the Cockpit job probes pages whose libraries it never built (`7b76530dd`)
- **ci**: the three things the first CI run found (`5ff8036a6`)
- **ci**: the Cockpit probe gets the libraries it probes for (`f992a15af`)
- **adr**: 0017 uses the heading levels the validator requires (`973dbc711`)
- **ci**: the Cockpit gate is declared, selected, and never ran (`1ce4d09dc`)
- **usecase**: the commit a handover quotes, not only the one it states (`7014e7c4f`)
- **usecase**: a derived record states its commit in prose, and evidence must not (`d28a771ac`)
- **ci**: one cockpit_assets output, not two (`0b64ef18e`)
- **derive**: a quoted section kept its words and lost its paragraphs (`adb949968`)
- **ci**: three gates that were not gating anything (`4667e9245`)
- **site**: the generator converges in one pass (`b4fde1f59`)
- **site**: a generated document's page is projected from what the run produced (`d93ba2bd9`)
- **schema**: the context contract keeps its constraints where they are now declared (`1c8af6b72`)
- **ci**: satisfy the gates the branch had not been run through (`4044f0181`)
- **context**: a directory's contract is discovered because it is one (`e65a47cf4`)
- **ci**: satisfy the gates the branch had not been run through (`c73c52fc1`)
- **site**: a skill example's path is a path, so it wraps like one (`2b42014b5`)
- **ci**: the Cockpit job probes pages whose libraries it never built (`a6dfd8c86`)
- **ci**: the three things the first CI run found (`3ffbbad55`)
- **usecase**: the commit a handover quotes, not only the one it states (`18f6e1d47`)
- **usecase**: a derived record states its commit in prose, and evidence must not (`05919656e`)
- **context**: a directory's contract is discovered because it is one (`db9906cf3`)
- **ci**: one cockpit_assets output, not two (`ef0400a91`)
- **docs**: the use-case example nests its fences, and the capture example uses no real home (`994561f73`)
- **derive**: a quoted section kept its words and lost its paragraphs (`a9dbdf927`)
- **ci**: three gates that were not gating anything (`504126f84`)
- **evidence**: the EPIPE diagnostic in recorded output, at its source and in the normaliser (`7719c72d1`)
- **rules**: one assertion that runs, in whichever form the author chooses (`19fb3b56d`)
- **usecase**: the run's verdict is a fact about the run, not about its printing (`5ea1db0f4`)
- **evidence**: the recorded evidence carried the operating system's user name (`0241fbff5`)
- **usecase**: the run's verdict is a fact about the run, not about its printing (`1742142d4`)
- **ci**: the plan job never declared cockpit_assets, so that gate could not fire (`2b6447d3c`)
- **schemas**: the session-record schema joins the convention, and the tree is regenerated (`f17cd99c0`)
- **session**: the four ways the session subsystem was already inconsistent with itself (`a840487c3`)
- **web**: the declaration module asserts what it wires, and an error names its kind once (`eb38d9207`)
- **rules**: the reserved names are held by tests, not by a runtime finding (`9dea53fdb`)
- **capture,state**: no eval in the writer, and current.yaml declares the key it writes (`a7fb5a6c9`)
- **schemas**: the capture and document schemas join the one convention (`2751b3950`)
- **web**: the listing header is a format string, not four arguments (`762cd61c3`)
- **adr**: renumber the scenario decision to 0014 (`b872ba2f5`)
- **bench**: a capability module called `system` vanished from three denominators (`af56e2e3f`)
- **site**: two assertions that moved with the tree under them (`7f3d9b96b`)
- **layer**: the checks the last three kinds left behind (`c38bd83c8`)
- **site**: route the documents a run writes before it writes them (`a1ba27a7c`)
- **pages**: the derived content is committed, so the fast path can render a clone (`4f98d412c`)
- **site**: a command page's breadcrumb names the command it is under (`dcb596c79`)
- **site**: a command page names the whole command, and says its summary once (`8b038698e`)
- **usecase**: pin the scenario repository branch (`fafd4acb8`)
- **claims**: the scope claim carries its own note (`02af4b255`)
- **site**: a new selection clears the previous one's edges; Registry is listed once in the navigation (`c9a173590`)
- **site**: the core-model diagram sits in the section's width, beside its callout (`a7d5847bd`)
- **site**: the homepage README fragments link site routes, not repository paths (`37dd48266`)
- **site**: a new selection clears the previous one's edges; Registry is listed once in the navigation (`8059b4fca`)
- **site**: a long identifier wraps instead of widening a registry or section page (`b3ceaf6e3`)
- **site**: a description on the executable's pages wraps at 320px (`c53b8f0dd`)
- **usecase**: the evidence normaliser masks the verification wall time (`969696df3`)
- **site**: the executable's route slug follows Zola, one Registry page (`3e8dac549`)
- **usecase**: the scratch directories are named as the no-forbidden-constructs case reads them (`a974d2c31`)
- **bench**: the routes bench loads the scope the index builder now takes (`b32dcdeb1`)
- **bench**: the routes bench loads the scope the index builder now takes (`144771d09`)
- **scope**: migrate seeds the scope, knowledge reads it, case 65 ignores the budget line (`94cdec334`)
- **bench**: the routes bench loads the scope the index builder now takes (`9454c0e4c`)
- **scope**: migrate seeds the scope, knowledge reads it, case 65 ignores the budget line (`1fce6f281`)
- **bench**: a baseline fails a run only on the host that recorded it (`38dedb392`)
- **plan**: the model accessors and the status JSON hold under bash 3.2 (`0a6d71d0d`)
- **site**: a path in a claim or an issue title wraps inside a 320px card (`27db8a983`)
- **scope**: migrate seeds the scope, knowledge reads it, case 65 ignores the budget line (`f28a5dad3`)
- **serve**: ask descriptor 0 whether stdin is a pipe, not the path /dev/stdin (`2356c588a`)
- **shell**: the plan model reads its rows under bash 3.2 (`6d3f18d39`)
- **site**: a path in a claim or an issue title wraps inside a 320px card (`aae7115e1`)
- **bench**: a baseline fails a run only on the host that recorded it (`cd3f04a43`)
- **serve**: ask descriptor 0 whether stdin is a pipe, not the path /dev/stdin (`8de168618`)
- **site**: plan and issue rows shrink at 320px instead of widening the page (`2a4537e9c`)
- **site**: a path in a claim or an issue title wraps inside a 320px card (`ff4406b50`)
- **mcp**: settle_alone answers a Reply, as the session does since the prepared listings (`277b366cb`)
- **merge**: the YAML subset types decimals, and two cases stop counting objects.get's cases (`c7b937cf6`)
- **mcp**: nothing a client leaves behind locks the others out of the shared server (`4a2f20aac`)
- **mcp**: order resources with sort_by_key, as clippy 1.98 asks (`d3ed96886`)
- **context**: drop the document chain under budget before any record body (`2db43cc0a`)
- **doctrine**: separate registry rows with the one byte no rule value can carry (`4f8b55dff`)
- **state**: reading commands answer on a checkout with no local store yet (`6808848fc`)
- **migrate**: back up and remove the legacy provider body instead of carrying it into .ai (`9b9f14c33`)
- **paths**: derive the distribution root from the entry point, never the environment (`e6df13ad7`)
- **site-data**: the plan model reaches jq through files, not argv (`780a76b07`)
- **site**: declare the inputs the generator reads, and use one fixture helper (`75e118cf2`)
- **claims**: a claim that belongs to no responsibility must say so (`45ff51821`)
- **checkpoint**: a finished task has no checkpoint due, so stop reporting one (`4c4b19a54`)
- **test**: assert the key a policy error names, not how it punctuates it (`e23c7e117`)
- **test**: assert the key a policy error names, not how it punctuates it (`fd48689df`)
- **site**: the overflow probe names what actually overflows (`7fa8057d6`)
- **ci**: the deploy waits for the measurement, not for a heuristic that predicts it (`0fdb682bb`)
- **prompts**: the shipped assets open with a heading, so they are valid Markdown (`6c680977a`)
- **policy**: a required key that is missing is reported, not turned into a shell error (`c1c2686f5`)
- **site**: a claim's note may wrap, so a path in it cannot set the page width (`a9c9cc000`)
- **site**: a sample says it is a sample (`2787b3b47`)
- **site**: the route map mirrors Zola's slug, and a canonical path may wrap (`1a50a2ccf`)
- **site**: let a step's note wrap to its own line on a phone (`fb0eb1d56`)
- **site**: expand the generator's input globs in byte order (`df6b79b7b`)
- **ci**: export the subshell setting shellcheck cannot see through, regenerate (`9483779a1`)
- derive what the tool knows about itself, and refuse a second copy (`920fce43a`)
- **docs**: PLAN_STATUS.md recorded the commit it was generated at, so it was stale on commit (`80ab7b25f`)
- **site**: refuse an invalid project model from the model's own findings (`5d63204fc`)
- **test**: the site fixtures copy the project registry the generator now requires (`95862bc25`)
- **site**: regenerate the manifest after editing one of its inputs (`8d4df52cf`)
- **site**: untrack two generated doctrine pages the new rule caught (`dbf5b1107`)
- **site**: the doctrine filter announces its state, not only its colour (`70b5768e0`)
- **site**: derive the responsibility count instead of writing it (`38327c0e7`)
- **site**: parenthesise the edges pipeline so jq 1.7 parses it (`60f83e37c`)
- **test**: replace ls|grep with a glob loop in the why-route count (`f6488ca07`)
- **question**: the file's own template is not an entry, and publish the blocker gap (`204f1c792`)
- **state**: a task record names its checkout, and does not export its scope (`8efa84799`)
- **site**: derive the route check from generated content; match node_modules as a symlink (`d0028b9ae`)
- **site**: point the npm scripts and gitignore at the current site scripts (`6b5aaafb7`)
- **start,check**: accumulate repeated --scope, tolerate spaces, normalise scope at read time (`a98a0a342`)
- **site**: min-w-0 on grid columns holding code; multi-line wrapper check in test 09 (`940fdf45d`)
- **site**: carry the GitHub Pages path prefix on every generated path (`06e85990f`)
- **site**: make --no-css genuinely need no Node (`b24691ce9`)
- **site**: set core.hooksPath in pages, and stop citing generated files as sources (`2738c69f6`)

### Performance

- **ui**: the audit drives a pool of tabs, and covers more for less (`1e8f5f658`)
- **context**: read the scope once and the document keys in-shell (`9f0c26824`)
- **shell**: path normalisation without a subprocess (`d280bedde`)
- **why**: the search text is built once, with the catalogue (`e6996ac62`)
- **context**: the coverage walk is one awk pass, not sixteen hundred subshells (`1b182f1f5`)
- **cockpit**: the pages are benchmark targets, timed with the routes they sit beside (`a277e0509`)
- **pages**: a publication path separate from the merge gates (`b959046cd`)
- **site**: one generation per build and check, not three (`dd846ec08`)
- **site-probe**: one browser process per route, three widths as iframes, a bounded pool (`c764ed3c7`)
- **mcp**: one git ls-files per build, listings as prepared bytes, a cheaper fingerprint (`1880cd434`)
- **capabilities**: list summaries, not schemas; derive the object view schema once (`e2a674459`)
- **generator**: JSON strings and arrays without a process each (`c578fefe5`)
- **plan**: the milestones, issues and graphs as JSON in one pass each (`5f14cb32b`)
- **generator**: load the plan model and every flat input into variables once (`cf4ad7560`)
- **knowledge**: hash every source in one process, and read each kind in one pass (`79a47a48d`)
- **yaml**: flatten many files in one awk, and load the plan with four processes (`b974e02fe`)
- **doctor**: load a flat file into variables once, and check the plan keys in one awk (`9494aba9c`)
- **project**: one unknown-key check per file, and the id check on the row pass (`144621957`)
- **commands**: read the command and event registries in one pass (`3a39694d7`)
- **doctrine**: read the registry as rows, once, in the dispatcher and the wiring check (`4f5907e29`)
- **rules**: load, check and register the rule set in one pass per file (`8d1dffb38`)
- **rules**: load, check and register the rule set in one pass per file (`59a5deb72`)
- **yaml**: one process per key lookup, and refuse an unclosed rule front matter (`e6321032e`)

### Changed

- **rules**: a project rule gets a gate, not a doctrine (`c0bb1bc29`)
- **web**: the router asks the topology instead of naming its routes (`797a88d7a`)
- **skills**: the hand-deploy procedure becomes a canonical skill (`1e862ddb6`)
- **site**: derive the number of why-moments instead of writing five (`373da8770`)
- **site**: derive the number of why-moments instead of writing five (`0cac92302`)
- **doctrine**: derive enforcement from portable rule objects (`220fd264c`)
- **context**: make provider files thin .ai bootstraps and remove the provider body (`a1ce10c82`)
- **ai**: move this repository's canonical data into .ai/repo and its state into .ai/local (`bd7cb2a57`)
- **projection**: detach provenance from legacy .majordomus state (`d1848ab0e`)
- **paths**: separate tool distribution from repository ai state (`66e318813`)
- **plan**: an M003 issue claims the case files it owns, not the whole directory (`7ca51d2ce`)
- **site**: every count on the site is rendered, never written (`20f94850e`)
- **plan**: one counts builder, and the milestone chain the gate makes redundant (`8917efa90`)

### Documentation

- **deploy**: a preview publication is visible, and says so on the page (`e40cefe9f`)
- **ci**: the reference gate in the model, and out of the suite's way (`2eb2c01bb`)
- **consolidation**: the forensic findings and the plan they justify (`95d9a773c`)
- **references**: every path a document names is a path that is there (`df7dcc385`)
- the directories say what lives in them, and every schema reference resolves (`17d3e1b23`)
- **why**: the schema paths and the assertion name the identity, not the old file (`029f26260`)
- **adr**: the capability graph is composed, not centralised, and projected twice (`665f1c4e8`)
- **session**: the working context declares itself where every other record does (`24af4a5de`)
- **use-cases**: the directory hierarchy is a question a client can ask (`c81090d13`)
- **context**: a carried subtree is released by name, and the tool knows no names (`0704b1faf`)
- **why**: the schema paths the merge moved, and the assertion that quoted one (`c50befe3f`)
- **adr**: the episode decision takes 0017, and every projection follows (`ef57411b4`)
- **claims**: the command line's documentation is a guarantee, not an accident (`fb7a30f95`)
- **use-cases**: the directory hierarchy is a question a client can ask (`fb30b23cf`)
- **context**: a carried subtree is released by name, and the tool knows no names (`1e0d176e7`)
- **use-cases**: the directory hierarchy is a question a client can ask (`9fa28449a`)
- **context**: a carried subtree is released by name, and the tool knows no names (`e8cd3339b`)
- **rules**: the case for a floor, from the branch that hit it (`70a299939`)
- **web**: the surface a reader can navigate, and the one a developer extends (`fa0d8245d`)
- **web**: the Swagger UI is at /swagger, and a quoted URL is not a link (`bc77bf6b0`)
- **claims**: what the two web-surface guarantees mean, and how to see them (`4e25cfc18`)
- **cockpit**: the Cockpit on the matrix it belongs to, and what it will not do (`c43f8bc9a`)
- **adr**: web surfaces are discovered from their producer and projected once (`8cee64583`)
- **cockpit**: the decision, the reference, and a use case that names it (`27f55987d`)
- **pages**: the publication path, its budgets and its evidence (`2e2299401`)
- **ai**: a native command exists only with its documentation and its executable examples (`8375d9ddf`)
- **use-cases**: the doctrine, the workflow, ADR 5 and the concept (`e358be230`)
- **ci**: the pages job publishes through scripts/site-deploy; the wiring case accepts it (`a7db43652`)
- **scope**: the doctrine, the claim, case 93 and the documentation (`104533222`)
- one contributor workflow (just derive, just derive-check); the benchmark-policy edge tested; long path links wrap at 320px (`0aa1ce285`)
- **scope**: the doctrine, the claim, case 93 and the documentation (`8aa2e0051`)
- **architecture**: the documentation projection graph — stages, boundary, ownership, classification — with its claims and gates (`97e3986f1`)
- **adr**: ADR 5, one projection plan over named canonical owners, the site as a view of the registry (`312be2a1f`)
- **claims**: name the test file of mcp-uri-resolution as committed (`47e6aa520`)
- **openapi**: claim openapi-inferred, case 92, and the projection rows (`2f39b57a7`)
- **adr**: ADR 5, one projection plan over named canonical owners, the site as a view of the registry (`cdbe95c70`)
- **openapi**: claim openapi-inferred, case 92, and the projection rows (`ecdce9d01`)
- **scope**: the doctrine, the claim, case 93 and the documentation (`0240a2652`)
- **claims**: name the test file of mcp-uri-resolution as committed (`99f2137ee`)
- **adr**: ADR 5, one projection plan over named canonical owners, the site as a view of the registry (`3fad40b7f`)
- **ai**: codify the shared server's resilience as doctrine (`b806960a2`)
- **capability**: resolve the intra-doc links to the module macros (`2bba3cf88`)
- **mcp**: the shared server, peers and client auto-start, with their claims (`195b19416`)
- **capabilities**: the capability model as doctrine, with its claims, ADR, CI gates and the every-interface case (`56a5f4063`)
- **rust**: document every public item, add executable examples, raise coverage to 91% (`b070d3796`)
- **cli**: document the Rust executable and its MCP surface, and wire it into the checks (`7e2f5484f`)
- **ai**: codify simplicity and extensibility doctrine (`7e826bf4c`)
- **claims**: file the layer and migration claims under the layer responsibility (`46bfff17b`)
- **ai**: say exactly what init writes and what the bootstrap check detects (`ab450a1ce`)
- **ai**: rewrite README, CLI, CONCEPTS and DESIGN for the layer as it is (`82128fcf9`)
- **claims**: state the .ai layer's guarantees and bring the matrix to master semantics (`20a804d4c`)
- **ai**: document the manifest, the vendored package and the knowledge sources; sweep the last legacy paths (`4ec84dfdd`)
- **rules**: list exits 2 and 13, and say doctor does not read rule objects yet (`0d3e125f6`)
- **ai**: sweep the pre-.ai paths out of docs, claims and the site (`e637f763c`)
- **readme**: the profiles row no longer says how many there are (`e7766ad40`)
- **continuity**: a session is the seventh record, and it is an envelope (`9c13909ca`)
- **design**: the boundary excludes semantic machinery, not a table of stated references (`3c9ba2f1c`)
- **ledger**: record why no rule guards a declared reader (`fb43dfa07`)
- **architecture**: the de-hardcoding ledger and the ownership rules behind it (`503edce06`)
- **schemas**: decisions.md names the commands that read it (`4f9f55aee`)
- **plan**: evidence states what a command showed, not a count that goes stale (`2dc3f514a`)
- **claims**: the roadmap version comes from the milestone, not from prose (`e0d0e054c`)
- **claims**: task-dependencies says what is refused, not that no graph exists (`cfd0f594e`)
- correct why the input order diverged, and record the environment lesson (`d48f9a636`)
- **plan**: the planning semantics, the dogfooding rule, and the schemas behind them (`879efbd30`)
- say that doctrines exist, where people actually read (`d7c9fa174`)
- **memory**: the continuity model, its schemas, and twelve claims with evidence (`2c4bbb4c2`)
- complete the documentation index and correct claims that outran the code (`fb1be3eab`)
- region projections, dispatcher wiring, and a flag that never existed (`87bbd51ae`)
- **economics**: cost decomposition, ledger-measurable metrics, per-outcome unit (`0107a0bad`)
- record the base-path lesson as the cleaner two-writers example (`967a95b55`)
- **site**: document the deployment prefix and the Node-free generation mode (`d24538f7c`)
- **site**: document the derived Pages architecture and content ownership (`5521c5da5`)
- add logo, social card, and website link to README (`6a224ad8b`)
- record the two-writers-one-checkout limitation observed while building v0.1 (`67c99d730`)
- cross-reference CLI and schema specs from design and report (`0c7abaebf`)
- expand README and add CLI, schema, contributing, and security docs (`6136dc3c5`)
- point README at design, extraction report, and contract (`54a869e29`)
- add extraction report, repo operating contract, and docs index (`9ce093642`)
- **design**: add typed outcomes, check --explain, presentation layer (`f0b4bc6ad`)
- add v0.1 design specification (`232b2797b`)
- add design-phase README (`556e524c2`)

### Tests

- **lib**: a fixture carries what the templates read, not what one generator names (`3a0b38c13`)
- **lib**: a command can be quiet when it works and speak when it does not (`22c18f959`)
- **registry**: the removal case takes a capability the catalogue does not need (`77e2a7a17`)
- **ci**: three cases read the repository as it is (`33b52ebf6`)
- **rules**: the DAG case and the CLI reference read the enforcement column as it is printed (`ae48c7051`)
- **ui**: the two cases this branch invalidated, both read from the model now (`1ccc432bf`)
- **generate**: the document schemas are projected where they belong, and proved so (`78cd4fae5`)
- **ci**: a gate that refuses a document naming a file that is not there (`368cd4ce9`)
- **ci**: a workflow is legitimate when it has a model, not when there are two of them (`c79ab0cef`)
- **cli**: every refusal of the documentation contract, provoked (`ab94b74bb`)
- **generate**: a projected JSON Schema is JSON by its own contract (`6206079f4`)
- **ci**: a workflow is legitimate when it has a model, not when there are two of them (`0f67cd5fb`)
- **continuity**: the populated page, and a determinism check that stopped watching the clock (`537c54000`)
- **continuity**: the populated page, and a determinism check that stopped watching the clock (`e328d2f79`)
- **web**: the vocabulary, the selection and the renderings the gate had not seen (`b312f32ec`)
- **web**: the reserved names and the derived page, proven over a socket (`e3d8895b7`)
- **cockpit**: every route the server serves, and the interactions, in a real browser (`2614422a0`)
- **use-case**: prove the refusal of an undocumented directory by running it (`eb4ecf267`)
- **pages**: the fast path is held to the shape that makes it fast (`8a8925a67`)
- **suite**: the timing cases and the API page case run alone; two cases clear MAJORDOMUS_BIN (`2ca48d4bf`)
- **scope**: case 93 reads the diagnostics where mcp --inspect nests them (`b6468d87e`)
- **scope**: case 93 reads the diagnostics where mcp --inspect nests them (`7d8cd469b`)
- **projections**: the plan test writes nothing outside the fixture root (`6ca6a2cd7`)
- **site**: a capability that joins or leaves the registry manifest gains or loses its route; a broken manifest is refused (`6791e9e16`)
- **suite**: a bounded parallel runner, exclusive cases, and MAJORDOMUS_BIN for the Rust cases (`e5de81170`)
- **cases**: a mutation case copies the site tree whole and reads the input list from --inputs (`2a73067c1`)
- **http**: replay every route's cases over a socket and time every route (`bbab30282`)
- **projections**: the plan test writes nothing outside the fixture root (`11a0acd67`)
- **http**: replay every route's cases over a socket and time every route (`82406d309`)
- **scope**: case 93 reads the diagnostics where mcp --inspect nests them (`8ec5922a9`)
- **bench**: the objects.get run has one result per declared case, not one (`14d958ebb`)
- **rust**: prove the Rust gates are wired, as three claims and case 77 (`015e55aeb`)
- **cli**: the no-share check runs inside an initialised repository (`e78732848`)
- **site**: fixtures carry every path the claims matrix names (`ff7d1a7fc`)
- **capabilities**: registry invariants, cross-projection identity, HTTP black box, cross-protocol parity, generate --check (`1e7761cb1`)
- **cli**: black-box coverage for the CLI, discovery, metadata, index, stdio MCP and extensibility (`68fde4bee`)
- **constructs**: allow the context-document scratch files in the write allow-list (`9a7adc757`)
- **context**: black-box cases for context impact and check-sync, against the contract (`62ce65ced`)
- **rules**: prove the rule DAG by mutation, dispatch `rules`, document rule objects (`c58893378`)
- **site**: mutate the state reference the scanner now resolves (`20e0bac56`)
- **plan**: capture output instead of piping it into grep -q (`a08ecde76`)
- **plan**: capture output instead of piping it into grep -q (`646308dea`)
- an empty derivation is a fault in the derivation, not an answer (`b559ce3b5`)
- **plan**: the roadmap graph gets its own mutation propagation proof (`955031ad9`)
- **site**: the committed derived artifacts are checked against this repository (`05423af46`)
- **plan**: cross-surface consistency and mutation propagation; CI gates the model (`c4c12ac21`)
- **cli**: derive the command surface from the dispatch table (`359b45e2d`)
- renumber the continuity cases to 20-26 (`c31159b9d`)
- **memory**: behavioural, negative and integration cases, wired into CI (`3b22af408`)
- lint the derived site for mobile-first and responsive constructs (`870c45ecd`)
- portable file-mode check; gate the derived site in validate (`57e85e1fb`)
- scan the tool's own source for forbidden constructs (`f3daaca36`)

### Pipeline

- **plan**: the cockpit probe is gated on an output the plan job declares (`40cf65522`)
- **pages**: the workflow's paths block follows the model again (`817ea7ba2`)
- **site**: one documentation source, two base paths (`da48cba98`)
- **cockpit**: the stylesheet is a generated artifact, so a gate proves it current (`f710d3472`)
- **suite**: the jobs that run the suite check out the whole history (`c991d86cb`)
- master runs are independent of one another; only deployments queue (`1e6edeafc`)
- **structure**: actionlint from a pinned release, not from install-action (`94926fa72`)
- the workflow parses, and actionlint is part of the lint gate (`50056aa3f`)
- one workflow over the plan; the site deploys from the run that verified it (`f0623b631`)
- **pages**: the site does not deploy from a stale registry dataset or bootstrap (`fea04fad7`)
- **pages**: the site does not deploy from a stale registry dataset or bootstrap (`b935ae090`)
- **pages**: the site does not deploy from a stale registry dataset or bootstrap (`e80ded65f`)
- **site**: the complete probe runs in validate, and master's shellcheck is green (`a27dd1cfa`)
- install site build dependencies in validate so the derived-site tests run (`e64e7af34`)

### Build

- **derive**: one entry point for every derived artifact, one read-only gate, and the commit out of the derived data (`58e2cd45b`)

### Housekeeping

- **release**: 0.3.1, the version the fixed pipeline releases (`032cc0025`)
- **derive**: the projections follow the merges (`1fd5e32e0`)
- **derive**: the projections follow the merge with master (`748de838d`)
- **derive**: the claim is an object of the index once it is tracked (`3d649ae3b`)
- **derive**: the projections follow the gate and the version (`fb5bbeb01`)
- **derive**: the projections follow the merges (`51a39cacc`)
- **derive**: the projections follow the fold (`89aec9654`)
- **derive**: the projections follow the merges (`f9a1a4482`)
- **derive**: the projections follow the merged tree (`eac7c97ba`)
- **derive**: the projections follow the merge of master (`db64b42bf`)
- **derive**: the projections follow the merge of master (`a568ee4c4`)
- **derive**: the projections follow the rebase onto master (`dfcc21f19`)
- **sessions**: record two feature/ai-artifacts episodes (`bfffc0b1f`)
- **derive**: the projections follow the merge with master (`4bca63379`)
- **derive**: every projection regenerated over the integrated tree (`c83f763fb`)
- **derive**: the projections follow the merged tree (`347c94342`)
- **sessions**: record two feature/ai-artifacts episodes (`c027ef6ea`)
- **plan**: I1003 done, with the evidence the contract asked for (`701d3780c`)
- **plan**: I1025 done, with the evidence the contract asked for (`a1fc8d8ac`)
- **plan**: I1004 done, with the evidence the contract asked for (`5b817c2eb`)
- **plan**: I1002 done, with the evidence the contract asked for (`46da8089d`)
- **derive**: the plan projection follows the two new milestones (`72544e632`)
- **derive**: the projections follow the capture fixes (`3fa38210f`)
- **plan**: I1001 done, with the evidence the contract asked for (`a4979a09b`)
- **derive**: the projections follow the merge with master (`988e928ee`)
- **derive**: every projection regenerated over the merge (`5961be109`)
- **derive**: every projection regenerated over the merge (`bb93f91d8`)
- **derive**: the site registry settles (`edba7a9c5`)
- **derive**: the projections follow the rebase onto master (`730f4b167`)
- **derive**: every projection regenerated over the merge (`5ba3d60b6`)
- **derive**: the site registry follows the tree (`1f7955968`)
- **derive**: the mirrored status page settles on the second pass (`67a17cea1`)
- **derive**: the site follows the ported schema and the use case (`33fca6129`)
- **derive**: regenerate the projections after the rebase (`305b7f743`)
- **derive**: the registry dataset over the capture fix (`5d0e628ef`)
- **derive**: the registry dataset over the objects the last commits added (`b3d05ae1d`)
- **derive**: the pages status over the inputs as they finally are (`a2bb43fca`)
- **derive**: the site dataset and the formatting the rebase left behind (`4137b87bc`)
- **derive**: the registry over the tree master merged into (`0a13b50e3`)
- **derive**: the site dataset follows the rebased tree (`c79b16c6e`)
- **derive**: the registry regenerated over the rebased tree (`32572bc9f`)
- **sessions**: the episodes that built this, closed by the events they added (`acb0c173a`)
- **derive**: regenerate after rebasing the claim onto master (`0b0c104ba`)
- **derive**: the site registry follows the tree (`c7f35c23c`)
- **derive**: the mirrored status page settles on the second pass (`ae4072bbc`)
- **derive**: the site follows the ported schema and the use case (`12668bb7b`)
- **derive**: regenerate the projections after the rebase (`42319a792`)
- **derive**: regenerate over the merge (`b46d727e7`)
- **derive**: the registry dataset over the objects the last commits added (`2af35cbc3`)
- **derive**: the pages status over the inputs as they finally are (`ced26b02c`)
- **derive**: regenerate over the merge, migrate one use case, renumber the ADR to 0016 (`df138c2e5`)
- **derive**: the projections follow the strict gate onto the merged tree (`0169c885e`)
- **derive**: regenerate the projections after the rebase (`a70fdbc34`)
- **derive**: the site dataset and the formatting the rebase left behind (`e08bfcc77`)
- **derive**: regenerate over the merged session work (`6a584e688`)
- **derive**: the projections follow the tree, so the site can publish again (`d4e0ed7fb`)
- **derive**: regenerate after the merges, so the publication path accepts the tree (`0d9c347b3`)
- **derive**: the registry over the tree master merged into (`86ce0e7d8`)
- **derive**: the site dataset follows the rebased tree (`44d2806a6`)
- **derive**: the registry regenerated over the rebased tree (`6a585031e`)
- **sessions**: the episodes that built this, closed by the events they added (`921ab6b16`)
- **derive**: regenerate over the merge (`129bfe1a8`)
- **derive**: settle the mirrored status page (`7da59b1c1`)
- **derive**: regenerate over the merged tree (`e17a73ead`)
- **derive**: regenerate over the merged tree (`d847b3226`)
- **derive**: regenerate every derived artifact after the session lifecycle landed (`c852dea59`)
- **derive**: every projection regenerated from the merged tree (`62b71b3b6`)
- **site**: the plan projection follows the two new milestones (`73e76c1f1`)
- **derive**: the second pass the index needs after the first (`4658856fe`)
- **derive**: regenerate every projection over the merged tree (`235c47106`)
- **derive**: the projections follow the web command and its reports (`56372e100`)
- **web**: the generated web root is ignored, like every other derived tree (`9bb67d64c`)
- **site**: the website follows the registry and the docs index, regenerated (`ef755d41c`)
- **cockpit**: vendor the module build the page actually loads, and the dataset (`0d26ed9c5`)
- **derive**: the site dataset follows the two assertions that moved (`eaad9f682`)
- **derive**: one registry for one tree, at the tip (`9e224d050`)
- **derive**: regenerate the projections this branch owns, from a clean tree (`53b1d1c6c`)
- **derive**: the pages-status stamp, current with the inputs it was computed over (`de48006e3`)
- **derive**: regenerate over the decision subsystem and its two use cases (`bc32cac72`)
- **derive**: the pages-status page, current with the document it projects (`eba8bdbda`)
- **derive**: regenerate for the generator and performance-doc change (`defed0c27`)
- **derive**: regenerate after integrating the prompt capture (`a0ccbc9e2`)
- **derive**: regenerate the derived artifacts over the integrated tree (`8cab3ebcf`)
- **derive**: regenerate the derived artifacts after merging the Pages fast path (`8e14274b6`)
- **derive**: regenerate the registry dataset over the tracked tree (`02b2e3e9f`)
- **derive**: regenerate for the third skill and the skills use case (`ef2136dc3`)
- **derive**: regenerate the site data and the registry dataset for the two skills (`9525f6280`)
- **site**: regenerate the derived data for the hero branch (`2cdc96310`)
- **derive**: regenerate on the merged tree (`fa9db2be4`)
- **derive**: regenerate the registry dataset after the usecase library change (`c06271f54`)
- **derive**: regenerate on the merged tree (`e90b347f0`)
- **derive**: regenerate every projection and the site data on the merged tree (`629ebf8a3`)
- **derive**: regenerate after merging PR #38 onto the scope policy (`31acaf90d`)
- **derive**: regenerate the projections, the OpenAPI document and the site data after the merges (`e31aceb75`)
- **derive**: regenerate the registry dataset and the site data after rebasing onto 4bbd726 (`55452d619`)
- **derive**: regenerate the site data and the registry dataset after merging PR #30 (`4bbd7266d`)
- **bench**: re-record the macos-aarch64-debug baseline with its host (Apple M5 Pro/18) (`5f538d4c7`)
- **derive**: regenerate the derived data after merging origin/master (`32f796175`)
- **derive**: regenerate the site data and the registry dataset after rebasing onto 0481024 (`b1be56192`)
- **site-data**: regenerate after the case 65 change (`1282a3630`)
- **derive**: regenerate the site data and the registry dataset after rebasing onto 77e28a8 (`04221ad38`)
- **derive**: regenerate the site data and the registry dataset after rebasing onto b9f1091 (`080adee37`)
- **derive**: regenerate the site data and the registry dataset after rebasing onto cb9dc62 (`5b845f33a`)
- **site**: regenerate the registry dataset after its own claim joined the index (`24bb212a2`)
- **site-data**: regenerate after the test-case change (`0bf5f316d`)
- **derive**: regenerate the derived data after integrating PR #30, #31, #32 and #33 (`5b0e3e807`)
- **derive**: regenerate the site data and the registry dataset after rebasing onto b9f1091 (`4656b0be5`)
- **derive**: regenerate the site data and the registry dataset after rebasing onto cb9dc62 (`81921eec1`)
- **site**: regenerate the registry dataset after its own claim joined the index (`5afb9bf57`)
- **bench**: re-record the macos-aarch64-debug baseline with its host (Apple M5 Pro/18) (`ff078af73`)
- **site-data**: regenerate for the case 28 probe change (`b9f109147`)
- **derive**: regenerate the site data and the registry dataset after rebasing onto cb9dc62 (`614b7b13a`)
- **site**: regenerate the registry dataset after its own claim joined the index (`1b017bad4`)
- **site-data**: regenerate the derived data after merging feature/mcp-shared-server (`cb9dc62b1`)
- **bench**: record the macos-aarch64-debug baseline from a clean tree at 1acda1d (`65ccf7835`)
- **projections**: restamp AGENTS.md and CLAUDE.md for the merged policy (`38093b339`)
- **generated**: regenerate the reference, the registry manifest and the site data after the merges (`08a235cf6`)
- **site-data**: regenerate after rebasing onto the merged Rust CLI (`da14b6414`)
- **site-data**: regenerate the plan status after the rebase onto master (`9d6339b14`)
- **site-data**: regenerate after the context claims and rule joined the inputs (`5e10fda21`)
- **rules**: a maintainer entry point for the shipped package manifest (`cbce33c79`)
- **site-data**: regenerate after the layer responsibility change (`6aba73108`)
- **rules**: wire the layer's claims into the rules that back them (`e2406d9cb`)
- **state**: close the plan-page task and start the .ai transformation task (`6d3b73b2e`)
- **state**: checkpoint that v0.1.0 is tagged (`e85269422`)
- **site-data**: regenerate source.json at the checkpoint commit (`aabee071b`)
- **state**: checkpoint the responsibilities and argv work (`31f8b7bdc`)
- **state**: record why the enforcement gap was taken outside the plan (`29a5879ef`)
- **gitignore**: ignore local scratch directories (`ca66fca22`)
- **state**: close the stale-base task and reopen at the current commit (`7ba86aeb8`)
- **site-data**: regenerate after RESPONSIBILITIES.yaml joined the inputs (`d6aca744e`)
- **state**: session records for the derived-plan-page task (`31965b1dc`)
- **site**: regenerate derived data after merging master (`07a71d563`)
- **site**: regenerate derived data after merging master (`e792ebe8a`)
- **site**: regenerate after merging the doctrine branch (`a726e3cfc`)
- **site**: regenerate derived data after merging master (`94d2180ad`)
- **site**: regenerate after the merge (`5ea2fa7ec`)
- **site**: regenerate the derived data after merging master (`8c26a8e72`)
- **plan**: the suite evidence cites the run that covers every case (`40b6f3bfd`)
- **site**: regenerate after the self-derivation and shellcheck fixes (`252a0dab2`)
- **site**: regenerate the derived data after merging the plan work (`99053b8c1`)
- **site**: regenerate derived data after the rebase (`7e88948fe`)
- **state**: checkpoint at the end of M000 (`6563fc4a3`)
- **state**: the merge brought .gitignore into this task's diff range (`ee0a12849`)
- **state**: close the stale-base task and start again at the current commit (`615c2b298`)
- **state**: widen the task scope to the projection targets and the README (`2dd5915d6`)
- **site**: regenerate after rebase (`e818a9d5e`)
- **site**: regenerate after rebase; new claims landed upstream (`a785baef2`)
- **state**: the audit task was scoped for writing, not for finding (`1555bf5b7`)
- **state**: close the worktree-ownership task; its base predates a peer's commits (`1e2bcfe50`)
- **state**: close the active task; it was gating other worktrees (`d95773ad7`)
- **state**: record the scope violation and the task that follows it (`beb7b074c`)
- **site**: regenerate derived site data after the rebase (`477239041`)

### Formatting

- **rust**: the crate is formatted as rustfmt wants it, and the projections follow (`7515a2b49`)
- **rust**: the crate is formatted, so the rust gate can fail for a reason (`5ae59e5fc`)
- **rust**: cargo fmt over the integrated tree, and the scope fixture's own limit (`1dbacfeb2`)
- **rust**: the crate is formatted as rustfmt wants it (`fead190a1`)
- **rust**: cargo fmt over the merged tree (`088ba7c86`)
- **tests**: one line rustfmt rewraps, so the rust gate can reach the rest (`6a2509c21`)

### Integration

- the rule, the case, the base-path gate and the generated route reference (`90ec4556c`)
- the contract-working hero (`99b117832`)

### Other

- **cockpit**: wip: the legibility work as the working tree held it (`643e75ea2`)
- Merge origin/master into fix/home-path-gate (`d63e2467d`)
- Merge origin/master into fix/derived-merge-driver-enforced (`e37f21a03`)

### Published

- `macos-aarch64` — majordomus-v0.3.1-aarch64-apple-darwin.tar.gz (`549269f08e9f`)
- `macos-x86_64` — majordomus-v0.3.1-x86_64-apple-darwin.tar.gz (`084a5956f06c`)
- `linux-x86_64-gnu` — majordomus-v0.3.1-x86_64-unknown-linux-gnu.tar.gz (`d4b34c5efeab`)
- `linux-x86_64-musl` — majordomus-v0.3.1-x86_64-unknown-linux-musl.tar.gz (`504f168f868c`)
- `linux-aarch64-gnu` — majordomus-v0.3.1-aarch64-unknown-linux-gnu.tar.gz (`6e7d865c6b4c`)
- `linux-aarch64-musl` — majordomus-v0.3.1-aarch64-unknown-linux-musl.tar.gz (`6ca046c0dd70`)

