use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicI64, Ordering},
};

use postproject_core::{
    Activity, ActivityId, ActivityInput, ActivityKind, ActivityOutput, Asset, AssetId,
    ContentStructure, Error, ErrorKind, Job, JobId, JobKind, Locator, LocatorAvailability,
    LocatorId, OriginalMediaImport, Representation, RepresentationId, RepresentationImport,
    RepresentationKind, RequestedJobOutput, Resource, ResourceId, Result, RevisionContext,
    Timestamp,
};
use postproject_protocol::{ClientId, Command, Extensions, Proposal, RequestId};

use crate::{SqliteProduction, job_clock::JobClock};

#[derive(Debug)]
pub(super) struct Clock {
    now: AtomicI64,
    unavailable: AtomicBool,
}

impl Clock {
    pub fn set(&self, now: i64) {
        self.now.store(now, Ordering::SeqCst);
    }
    pub fn disable(&self) {
        self.unavailable.store(true, Ordering::SeqCst);
    }
}

impl JobClock for Clock {
    fn now(&self) -> Result<Timestamp> {
        if self.unavailable.load(Ordering::SeqCst) {
            return Err(Error::new(
                ErrorKind::Internal,
                "test authority clock unavailable",
            ));
        }
        Ok(Timestamp::from_unix_micros(self.now.load(Ordering::SeqCst)))
    }
}

pub(super) struct Fixture {
    directory: tempfile::TempDir,
    pub source: SqliteProduction,
    pub clock: Arc<Clock>,
    pub job: Job,
    pub output: RepresentationImport,
    pub activity: Activity,
}

fn media(asset: AssetId, role: RepresentationKind) -> RepresentationImport {
    let resource = ResourceId::new();
    RepresentationImport::new(
        Representation::new(
            RepresentationId::new(),
            asset,
            role,
            ContentStructure::single_resource(resource),
            vec![],
        ),
        vec![Resource::new(resource, vec![], None)],
        vec![
            Locator::new(
                LocatorId::new(),
                resource,
                "file:///missing/media.mov",
                None,
                LocatorAvailability::Offline,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let mut source =
            SqliteProduction::create(directory.path().join("source.pproj"), None).unwrap();
        let clock = Arc::new(Clock {
            now: AtomicI64::new(100),
            unavailable: AtomicBool::new(false),
        });
        source.job_clock = clock.clone();
        let asset = Asset::new(AssetId::new(), Timestamp::from_unix_micros(0), None, None);
        let (representation, resources, locators) =
            media(asset.id(), RepresentationKind::Original).into_parts();
        let input = representation.id();
        let original =
            OriginalMediaImport::new(asset.clone(), representation, resources, locators).unwrap();
        let job = Job::new(
            JobId::new(),
            JobKind::new("unknown:Work").unwrap(),
            vec![input],
            RequestedJobOutput::new(asset.id(), RepresentationKind::Proxy, None).unwrap(),
        )
        .unwrap();
        let output = media(asset.id(), RepresentationKind::Proxy);
        let activity = Activity::new(
            ActivityId::new(),
            ActivityKind::new("unknown:Build").unwrap(),
            vec![ActivityInput::new(input, None)],
            vec![ActivityOutput::new(output.representation().id(), None)],
        )
        .unwrap();
        let mut fixture = Self {
            directory,
            source,
            clock,
            job,
            output,
            activity,
        };
        let request = fixture.proposal(vec![
            Command::ImportOriginal(original),
            Command::RequestJob(fixture.job.clone()),
        ]);
        fixture.source.submit_proposal(&request).unwrap();
        fixture
    }

    pub fn proposal(&self, commands: Vec<Command>) -> Proposal {
        Proposal::new(
            self.source.exchange_scope().unwrap(),
            ClientId::new(),
            RequestId::new(),
            None,
            RevisionContext::default(),
            commands,
            Extensions::default(),
        )
        .unwrap()
    }

    pub fn reopen(&mut self) {
        self.source = SqliteProduction::open(self.directory.path().join("source.pproj")).unwrap();
        self.source.job_clock = self.clock.clone();
    }

    pub fn completion(&self) -> Proposal {
        self.proposal(vec![Command::CompleteJob {
            job_id: self.job.id(),
            output: Box::new(self.output.clone()),
            activity: Box::new(self.activity.clone()),
        }])
    }
}
