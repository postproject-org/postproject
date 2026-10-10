"""Positive nominal-ID fixture; checked with the package's regular typing gate."""

from typing import assert_type
from uuid import UUID

from postproject import (
    ActivityId,
    ActivityRef,
    ActivitySpec,
    AssetId,
    AssetRef,
    ClientId,
    HistoryId,
    JobId,
    JobRef,
    LocatorId,
    MediaRootId,
    Production,
    Proposal,
    ProtocolBase,
    RepresentationId,
    RequestId,
    RequestIdentity,
    Transaction,
    parse_id,
)


def correct_kinds(
    production: Production,
    transaction: Transaction,
    asset: AssetId,
    representation: RepresentationId,
    root: MediaRootId,
    locator: LocatorId,
    job: JobId,
    activity: ActivityId,
) -> None:
    assert_type(parse_id(str(UUID(int=1)), AssetId), AssetId)
    assert_type(production.asset(asset).id, AssetId)
    assert_type(production.representation(representation).id, RepresentationId)
    assert_type(AssetRef(asset).id, AssetId)
    assert_type(transaction.add_media_root("rushes"), MediaRootId)
    transaction.set_media_root_enabled(root, False)
    assert_type(parse_id(str(locator), LocatorId), LocatorId)
    transaction.retire_locator(locator)
    assert_type(production.job(job).id, JobId)
    assert_type(JobRef(job).id, JobId)
    transaction.cancel_job(job)
    assert_type(parse_id(str(activity), ActivityId), ActivityId)
    assert_type(ActivityRef(activity).id, ActivityId)
    assert_type(
        transaction.create_activity(ActivitySpec("com.example:record", ())), ActivityId
    )


def correct_exchange(
    transaction: Transaction,
    client: ClientId,
    request: RequestId,
    history: HistoryId,
    proposal: Proposal,
    base: ProtocolBase,
) -> None:
    assert_type(transaction.prepare_proposal(client, request), Proposal)
    assert_type(proposal.identity, RequestIdentity)
    assert_type(proposal.identity.history_id, HistoryId)
    assert_type(proposal.identity.client_id, ClientId)
    assert_type(proposal.identity.request_id, RequestId)
    assert_type(parse_id(str(history), HistoryId), HistoryId)
    assert_type(base.history_id, HistoryId)
