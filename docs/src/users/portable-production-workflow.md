# Portable production workflow

This walkthrough shows the central PostProject idea with the CLI: create a production, add media, move that media somewhere else, and resolve the same production identities in the new environment.

## 1. Create a production and a logical media root

```sh
postproject init documentary.pproj --name "Documentary"
postproject root add documentary.pproj originals --label "Camera originals"
postproject media add documentary.pproj /media/A001.mov --name "A001" --inspect
```

The `.pproj` file stores production knowledge. The root named `originals` is a logical storage concept; it is deliberately different from one machine's absolute path.

Adding the media creates durable production objects for the logical asset and its stored representation. `--inspect` adds technical observations when `ffprobe` is available; without it the import still succeeds and reports the inspection as unavailable.

## 2. Import real and compound media

A representation does not have to be one file. For example, an image sequence can be imported as one meaningful representation:

```sh
postproject media add documentary.pproj /media/plates/shot010 \
  --name "Shot 010 plates" --sequence-rate 24/1
postproject media add documentary.pproj /media/CARD_001 --name "Card 001"
```

This matters because a sequence or camera package should remain one production concept even when it is made of many filesystem entries.

## 3. Move the storage

Suppose the production moves to another workstation or the originals are mounted somewhere else. Do not create replacement asset identities just because paths changed.

Map the logical root for the current environment and resolve the existing asset:

```sh
postproject media resolve documentary.pproj MOV_ASSET_ID \
  --root-map originals=/mnt/documentary
```

The root mapping is machine-specific. The production's identity model is not.

If resolution produces one supported result, the host can continue with that resource. If several candidates remain plausible, ambiguity should be shown to the user or handled explicitly by the host rather than guessed away. Resolution never changes the production; `--confirm URI` records the chosen candidate as a new locator:

```sh
postproject media resolve documentary.pproj MOV_ASSET_ID \
  --root-map originals=/mnt/documentary \
  --confirm file:///mnt/documentary/A001.mov
```

Media that is not under any configured root can be searched with `--search-dir PATH`; such directories are used for this call only and never recorded in the production.

## 4. Detect damage and inspect the inventory

Resolution and verification are separate concerns. Verification lets you ask whether the resource found at a location still matches the evidence recorded for it.

```sh
postproject media resolve documentary.pproj SEQUENCE_ASSET_ID \
  --root-map originals=/mnt/documentary --verify --json
postproject media inventory documentary.pproj \
  --root-map originals=/mnt/documentary \
  --cache /var/tmp/documentary-inventory.json --json
```

An inventory is useful for discovering resources in a particular environment. It should be treated as machine-local operational data, not as the permanent identity of the production.

For compound media, availability can be richer than a simple yes/no. A representation may be online, partial, offline, ambiguous, or in an error state depending on the availability of its required resources.

## 5. What remains stable

After the move, the important production meaning should still be the same:

```text
asset identity
  └── representation identity
      └── resource identity
          └── new or confirmed locator
```

That stable chain is what lets application project files store a PostProject identity instead of treating an absolute path as the only truth.

## 6. Where to go next

- For image sequences, spans, and package media, read {doc}`image-sequences-and-spanned-media`.
- For inspection and production history, read {doc}`metadata-and-provenance`.
- For application integration and confirmation of resolution candidates, read {doc}`../integrators/media-resolution`.
