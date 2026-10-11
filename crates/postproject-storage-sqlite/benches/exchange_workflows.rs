//! Isolated streaming exchange phases for time, RSS and closed-file measurements.

#[path = "exchange_workflows/fixtures.rs"]
mod fixtures;

use std::{env, fs::File, io::BufWriter, path::Path, time::Instant};

use postproject_storage_sqlite::{CheckpointLimits, ReplayLimits, SqliteProduction, exchange_file};

fn main() {
    let args: Vec<_> = env::args().collect();
    assert_eq!(args.len(), 4, "exchange_workflows phase input output");
    let input = Path::new(&args[2]);
    let output = Path::new(&args[3]);
    let started = Instant::now();
    match args[1].as_str() {
        "checkpoint" => {
            let source = SqliteProduction::open(input).unwrap();
            let writer = BufWriter::new(File::create_new(output).unwrap());
            let manifest = exchange_file::write_checkpoint(&source, writer).unwrap();
            println!("head_sequence\t{}", manifest.head().sequence());
        }
        "import" => {
            let mirror = exchange_file::import_checkpoint(
                output,
                File::open(input).unwrap(),
                exchange_file::FileLimits::default(),
                CheckpointLimits::default(),
            )
            .unwrap();
            println!(
                "head_sequence\t{}",
                mirror.exchange_head().unwrap().sequence()
            );
        }
        "changes" => {
            let source = SqliteProduction::open(input).unwrap();
            let from = source.record_reader(1).unwrap().manifest().head().unwrap();
            let writer = BufWriter::new(File::create_new(output).unwrap());
            let through = exchange_file::write_changes(&source, from, writer).unwrap();
            println!("records\t{}", through.sequence() - from.sequence());
        }
        "apply" => {
            let mut mirror = SqliteProduction::open(input).unwrap();
            let before = mirror.exchange_head().unwrap().sequence();
            let head = exchange_file::apply_changes(
                &mut mirror,
                File::open(output).unwrap(),
                exchange_file::FileLimits::default(),
                ReplayLimits::default(),
            )
            .unwrap();
            assert_eq!(mirror.exchange_head().unwrap(), head);
            println!("records_applied\t{}", head.sequence() - before);
        }
        "large" => fixtures::large(input),
        "verify-large" => fixtures::verify_large(input),
        "outcomes" => fixtures::outcomes(input),
        _ => panic!("unknown exchange workload"),
    }
    println!("elapsed_ns\t{}", started.elapsed().as_nanos());
    for (name, path) in [("input_bytes", input), ("output_bytes", output)] {
        if path.exists() {
            println!("{name}\t{}", path.metadata().unwrap().len());
        }
    }
}
