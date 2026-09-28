# ADR 0038: Image-sequence file names belong to the locator

- Status: Accepted
- Date: 2026-09-28

## Context

ADR 0003 separates content from place: a resource identifies stored content, a
locator identifies one place or route to it, and "locator changes do not change
content identity". For a single file that holds, since renaming or moving the
file changes only its locator, and resolution finds a renamed file by content.

An image sequence did not follow the rule. Its file-name pattern (prefix,
suffix, and frame-number padding) was part of the representation's sequence
descriptor, while its locator named only the directory. The sampled sequence
fingerprint hashed the prefix, suffix, and sampled file names. Resolution
looked for a moved sequence only in directories holding a file with the
recorded name of its first frame. So a sequence renamed from `shot_0001.png` to
`shot-graded_0001.png` had different content identity, could not be found, and
could not be recorded at its new place even if a user pointed at it, because
every locator of the resource had to share the one pattern.

Renaming a sequence is routine: graded plates get a suffix, a render is
renamed for delivery, a conform tool renames by shot. The Blender pilot's
success criterion was to relink a renamed image sequence by content. The same
need exists in compositors, which read plates by pattern, in editors, in
review tools, and in an OpenAssetIO Manager resolving a sequence to a templated
location.

## Decision

A sequence's file names are part of where it is, not what it is.

- **Locator.** A locator of an image-sequence resource carries a sequence
  naming: the prefix, suffix, and frame-number padding of the files in its
  directory. Every such locator has one; no other locator has one. Different
  locators of one sequence may name its files differently.
- **Descriptor.** The representation's sequence descriptor keeps the frame
  range, step, rate, and frames known to be missing. It no longer holds names.
- **Fingerprint.** `pp-blake3-sequence-sampled-members` version 2 hashes the
  frame range, step, rate, known missing frames, sampled frame numbers, and
  each sampled member's content fingerprint, and no names. The representation
  fingerprint drops the names in the same way and becomes version 2. Version 1
  values remain stored but are no longer computed, so they are foreign domains
  (ADR 0028) until the content is observed again.
- **Resolution.** A known locator is checked under its own naming. To find a
  moved or renamed sequence, the search groups the numbered files of each
  directory by prefix, suffix, and padding. A group that holds every expected
  frame is verified against the version 2 fingerprint. A candidate carries the
  naming it was found under, with file-name evidence only when that naming
  equals a recorded one. Several matching groups are ambiguous; a group missing
  an expected frame is no candidate. Without a comparable (version 2)
  fingerprint, only a group under a naming some locator records is offered,
  with `fingerprint_not_verified` evidence.
- **Confirmation.** Confirming a candidate for a sequence records its naming
  with the locator. `pp_transaction_confirm_locator(transaction, resource_id,
  uri, root_name, sequence_naming, out_error)` takes an optional root name and
  an optional `pp_sequence_naming_t`, required exactly for a sequence
  resource; a mismatch fails at commit. It replaces
  `pp_transaction_confirm_locator_under_root`. A resource's locators are unique
  by URI and naming, so one directory may be recorded under two namings.
- **Reading.** Locators and resolution candidates report their naming on every
  surface. `pp_representation_set_get_sequence` loses its prefix, suffix, and
  padding outputs; `pp_representation_set_get_locator` and
  `pp_resolution_set_get_candidate` gain a naming output, as does
  `pp_locator_query_set_get`: an `out_has_sequence_naming` flag and a borrowed
  `pp_sequence_naming_t`. Python's `Locator`
  and `ResolutionCandidate` gain `sequence_naming`, and
  `ImageSequenceDescriptor` loses its name fields. C++ changes the same way.
  C++ `Transaction::confirmLocator` takes an optional root name and naming and
  replaces `confirmLocatorUnderRoot`; Python's `confirm_locator` takes
  `media_root` and `sequence_naming` keywords and replaces
  `confirm_locator_under_root`. Revision events are unchanged: a
  `locator_added` event identifies the locator, whose naming is read from it.
  The C ABI becomes version 35.
- **Naming a sequence elsewhere.** `pp_media_source_create_image_sequence` takes
  its naming as a `pp_sequence_naming_t`; the directory and naming become the
  sequence's first locator. Verifying or observing a sequence's content at a
  directory needs the naming of its files there:
  `pp_production_verify_resource` and `pp_transaction_observe_resource_content`
  take a nullable `pp_sequence_naming_t`, where NULL means the naming recorded
  for that directory. The CLI prints namings as printf-style patterns such as
  `shot_%04d.png`, takes `--confirm-naming` to choose among candidates at one
  URI and `--sequence-naming` for `media fingerprint` and `media verify-content`.
- **Storage.** Schema 15 moves the prefix, suffix, and padding from
  `image_sequences` to a new `locator_sequence_namings` table, one row for each
  locator of every sequence resource. The locator table is rebuilt without its
  `(resource_id, uri)` uniqueness.

## Alternatives considered

- **A templated locator URI**, such as `file:///mnt/show/plates/shot_{frame}.png`, as
  OpenAssetIO presents a sequence. Braces are not valid in a URI, a template is
  not a place, and every consumer comparing locators would have to parse it.
  Hosts that need a template build it from the directory and the naming, as the
  OpenAssetIO Manager does.
- **Rewriting the descriptor on confirmation.** Keeping the names in the
  descriptor and replacing them when a renamed candidate is confirmed would
  relink the pilot's case, but every place a sequence is stored would have to
  use one naming, which a backup copy under the original names contradicts.
- **Keeping names in the fingerprint and verifying renamed candidates under
  the recorded names.** Content identity would still depend on a locator
  property, contrary to ADR 0003.

## Standards impact

None. Sequence naming is PostProject's own model. Two interchange standards
already keep names with the location, which this decision matches, checked
against the versions PostProject's integrations use:

- OpenTimelineIO 0.18.1's `ImageSequenceReference` holds `target_url_base`
  with `name_prefix`, `name_suffix`, and `frame_zero_padding` beside
  `start_frame`, `frame_step`, and `rate`.
- OpenAssetIO-MediaCreation 1.0.0a13's `LocatableContentTrait` carries a
  location marked `isTemplated` for a sequence, and the PostProject Manager
  builds that template from a directory and names.

## Consequences

A renamed or moved sequence is found by content and can be recorded under its
new names, beside locators that keep the old ones. Hosts read names from the
locator they use, not from the representation. Productions opened by schema 15
keep their sequences; their version 1 fingerprints are not comparable until the
sequence is observed again, and until then a moved sequence is offered with
`fingerprint_not_verified` evidence rather than a fingerprint match.
