# Exchange implementation inventory

Baseline inspected on 2026-10-08: tag `v0.7.0-alpha.1` resolves to
`e14d92fbcd7de55626b7da3a42aee4e8d3a32ba6`; development starts at
`663a8cf83b3062eb74636a4655c3882f1f607cdd`. Package/Python/ABI/schema are
0.7.0-alpha.1 / 0.7.0a1 / 51 (322 exports) / 19. GitHub release metadata confirms
publication at 18:11:25 UTC. The tag contains an SSH signature; local signature
verification is unavailable without `gpg.ssh.allowedSignersFile`.

The development delta changes installation/Flatpak documentation and version
guards only. Preserve those corrections. The workspace instructions apply to
all 13 active GitHub repositories; no nested instructions were found. The
historical Shotcut scaffold has no remote and is outside qualification.
[Repository inputs](exchange-repositories.json) record exact starting commits
and preexisting untracked builds. Candidate qualification has **not** run.

Development schema 20 adds one persistent history generation and a deterministic
genesis/migration-floor anchor. Existing revisions remain intact. The codec now
covers strict framing, every metadata value kind and metadata proposal intent;
complete mutation/effect coverage and the other exchange workflows remain pending.

## Mutations and effect requirements

All transaction methods and actual schema-19 tables/columns are classified in
[the machine-readable inventory](exchange-inventory.json).
`python3 tools/check_exchange_inventory.py` rejects unreviewed additions. Pending
coverage is explicit; classification alone does not establish replay correctness.

| Mutation | Guard/merge | Required captured facts |
|---|---|---|
| Original import / add representation | Additive; exact uniqueness/references | Assigned aggregate IDs, structures, ordered memberships, resources, locators, initial fingerprints |
| Add / retire locator | Locator-set key; retirement needs base | Exact locator/naming, root evidence, observation, removal |
| Add / enable / remove root | Root-name/root-ID keys; destructive base | Complete root and enabled/removal fact; no machine mapping |
| Add / remove identifier | Exact attachment key; removal base | Target, scheme/value/qualifier verbatim; no physical row ID |
| Append metadata | Additive; advances property version | Exact ordered assertion/value |
| Replace / remove metadata | Property key and required base | Complete ordered replacement or removal, including empty set |
| Create activity | Additive; references and cycle checks | Activity/edges plus fingerprint and dependency snapshots captured now |
| Record dependency set | Required base; complete-set key | Ordered set, explicit empty observation, extraction and recorded boundary |
| Record resource fingerprint | Required base; fingerprint-domain key | Current and superseded values/boundaries; affected recomputation markers |
| Record file facts | Required base; resource-file-facts key | Exact measured size/time and observation event |
| Record representation fingerprint | Required base; fingerprint-domain key | Current/history boundaries and cleared recomputation marker |
| Request job | Additive; exact referenced inputs | Request, canonical inputs, output and target root |
| Claim / renew / release / fail | Authority state/time/claim fencing | Public attribution/expiry/state; never bearer claim UUID |
| Complete lease | Input guards and atomic publication | Output aggregate, activity/snapshots, terminal job and input boundary |
| Cancel job | State-guarded coordinator action | Terminal cancellation and public event |
| Context / commit / rollback | Terminal lifecycle; own receipt | Assigned transaction/revision/time/context, ordered events, conflict versions |

`confirm_locator` and host bindings reduce to locator/identifier mutations.
Media import/adoption/content observation prepare facts in the media adapter;
storage persists them through these native transaction operations. Job completion
calls aggregate/activity persistence in the same transaction. No general entity
deletion is offered. Native capture must cover each of these indirect paths.

## Portable state and exclusions

Portable tables include current domain values, fingerprint history and
recomputation, activity evidence, observation revisions/events and conflict
versions/migration floors. Public ordering uses domain positions or section
order, never physical row IDs. Activity/path foreign row IDs are remapped to
edge/path positions; fingerprint histories retain their original ordered facts.
Production schema/singleton fields are local storage bookkeeping.

`jobs.claim_id` is a bearer secret and excluded even for a claimed observation.
Keep attribution/expiry faithful using an inert mirror representation. Claim
input boundaries derive from retained claim events and conflict history. Job
parameters are ordinary metadata. Local root mappings are caller values, not
persisted tables. Absolute locators and legacy root URIs are retained historical
knowledge, without a reachability claim.

Rebuild `unresolved_memberships`, `media_root_representations`,
`activity_output_keys` and `revision_event_kinds` from portable facts. Exclude
`schema_migrations` and operational `job_clock`. The production header is created
outside ordinary edits; failed lease paths may preserve clock high-water after
rollback. Those private writes do not produce portable revisions.

## Evidence status

Inventory/specification are development records. Codec, capture, outcomes,
checkpoint/mirror, conformance, projections, consumer qualification and package
gates remain incomplete. No new SDK artifact, host run, commercial acceptance
or release publication is claimed by this inventory.
