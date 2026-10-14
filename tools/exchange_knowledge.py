"""Prepare bounded, exact compound-media knowledge through an installed SDK."""

from __future__ import annotations

from fractions import Fraction
from pathlib import Path

import postproject as pp


def sources(root: Path) -> tuple[pp.MediaSource, ...]:
    directory = root / "knowledge"
    directory.mkdir()
    for name, content in (
        ("first.dat", b"first"),
        ("second.dat", b"second"),
        ("sidecar.txt", b"exact unknown sidecar"),
    ):
        (directory / name).write_bytes(content)
    frames = directory / "frames"
    frames.mkdir()
    for frame in range(-1, 2):
        (frames / f"frame{frame:04}.dat").write_bytes(f"frame {frame}".encode())
    return (
        directory / "first.dat",
        pp.ImageSequenceSource(
            frames,
            pp.SequenceNaming("frame", ".dat", 4),
            -1,
            1,
            1,
            Fraction(24_000, 1_001),
        ),
        pp.OrderedPartsSource(
            (
                pp.FileResourceInput(str(directory / "first.dat"), "vendor:first"),
                pp.FileResourceInput(str(directory / "second.dat"), "vendor:second"),
            )
        ),
        pp.PackageSource(
            (
                pp.FileResourceInput(str(directory / "first.dat"), "vendor:essence"),
                pp.FileResourceInput(
                    str(directory / "sidecar.txt"), "vendor:sidecar", False
                ),
            )
        ),
    )


def exact_metadata(target: pp.ObjectReference) -> pp.MetadataValue:
    return pp.MetadataStruct(
        (
            pp.MetadataStructField(
                "ordered",
                ((("同 😀"), ("同 😀"))),
            ),
            pp.MetadataStructField("integer", pp.MetadataU64(2**64 - 1)),
            pp.MetadataStructField("decimal", pp.MetadataDecimal(-(2**127), 3)),
            pp.MetadataStructField("ratio", Fraction(-(2**63), 2**64 - 1)),
            pp.MetadataStructField("opaque", (b"\0\xff")),
            pp.MetadataStructField("signed", (-(2**63))),
            pp.MetadataStructField(
                "language", pp.MetadataLanguageString("同", "x-Exact")
            ),
            pp.MetadataStructField("boolean", (True)),
            pp.MetadataStructField("time", pp.MetadataTimestamp(-(2**63))),
            pp.MetadataStructField("uri", pp.MetadataUri("unknown:Exact%2fCase")),
            pp.MetadataStructField("reference", pp.MetadataReference(target)),
        )
    )


def prepare(
    source: pp.Production, root: Path, client: pp.ClientId, request: pp.RequestId
) -> tuple[
    pp.Proposal, pp.PreparedAsset, pp.PreparedMediaRoot, pp.PreparedRepresentation
]:
    content = sources(root)
    with source.proposal_builder(
        client, request, base=source.exchange_state.observed_base
    ) as builder:
        original = builder.stage(
            [pp.ImportMediaCommand(content[0], "Exact compound knowledge")]
        )[0]
        assert isinstance(original, pp.PreparedAsset)
        configured = builder.stage(
            [pp.AddMediaRootCommand("qualification", "Exact 同", -5)]
        )[0]
        assert isinstance(configured, pp.PreparedMediaRoot)
        temporary = builder.stage(
            [pp.AddMediaRootCommand("temporary", "Remove this root", 7)]
        )[0]
        assert isinstance(temporary, pp.PreparedMediaRoot)
        made = builder.stage(
            [
                pp.AddRepresentationCommand(
                    original.asset_id, pp.RepresentationKind.DERIVED, content[1]
                ),
                pp.AddRepresentationCommand(
                    original.asset_id, pp.RepresentationKind.OPTIMIZED, content[2]
                ),
                pp.AddRepresentationCommand(
                    original.asset_id, pp.RepresentationKind.DERIVED, content[3]
                ),
            ]
        )
        assert all(isinstance(item, pp.PreparedRepresentation) for item in made)
        sequence = made[0]
        assert isinstance(sequence, pp.PreparedRepresentation)
        builder.stage(
            [
                pp.AppendMetadataCommand(
                    pp.AssetRef(original.asset_id),
                    pp.MetadataProperty("urn:vendor:Unknown", "ordered"),
                    exact_metadata(pp.AssetRef(original.asset_id)),
                ),
                pp.AppendMetadataCommand(
                    pp.AssetRef(original.asset_id),
                    pp.MetadataProperty("urn:vendor:Unknown", "ordered"),
                    exact_metadata(pp.AssetRef(original.asset_id)),
                ),
                pp.AddIdentifierCommand(
                    pp.AssetRef(original.asset_id),
                    pp.ExternalIdentifier("urn:vendor:EXACT", "  同 😀  ", "Keep"),
                ),
                pp.AddIdentifierCommand(
                    pp.AssetRef(original.asset_id),
                    pp.ExternalIdentifier("urn:vendor:EXACT", "remove", "Temporary"),
                ),
                pp.SetMediaRootEnabledCommand(temporary.root_id, False),
                pp.RemoveMediaRootCommand(temporary.root_id),
            ]
        )
        return builder.prepare(), original, configured, sequence
