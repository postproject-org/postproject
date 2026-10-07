# 0056: Decision-bound content observations

Status: accepted for development.

## Decision

Explicit resource fingerprints, representation fingerprints and file facts
require a decision base, including first and unchanged observations. Initial
facts stored with a new imported resource/representation remain part of the
additive import. A fingerprint observation replaces a current value in its
domain; adding a previously unseen domain is still an explicit observation.

Changed size/modification-time facts emit `resource_file_facts_observed` and
advance a resource file-facts conflict key. An unchanged value emits nothing.
Unchanged observations still check their conflict key at commit.
Facts remain cheap discovery filters, not identity evidence, and retain no
history. A successful facts-only edit has its own revision/receipt. Content
observation commits resource facts, fingerprints and recomputed aggregate
fingerprints together. A stale conflicting edit rolls all of them back.

Validate the base at the storage and native staging boundaries before work;
early rejection leaves the edit open. Import remains available without a base.
Decision keys protect mutated observations, not every membership or read fact;
this does not promise full read-set serializability.

## Migration and standards impact

Use a read session's edit for explicit observations. CLI `media fingerprint`
requires a scoped `--decision-base`. Existing IDs and fingerprint bytes retain
their meanings. Schema 18 permits the additional journal event and preserves
the old journal, event-kind index and query indexes during migration. C ABI
47 gains additive event/conflict tags; signatures and public layouts do not
change. Use matching development projections to recognize the new tags.

Reviewed against the standards policy: this changes concurrency and journal
attribution, without new fingerprint algorithms, metadata mappings or standards
normalization.
