//! End-to-end tests of the paginated domain-query commands.

use std::{fs, path::Path};

use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

const TRANSCODE_KIND: &str = "org.postproject:transcode";
const CONFORM_KIND: &str = "org.example:conform";

fn run_json(arguments: &[&str]) -> Value {
    let assertion = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(arguments)
        .assert()
        .success();
    serde_json::from_slice(&assertion.get_output().stdout).expect("command emits valid JSON")
}

fn run_text(arguments: &[&str]) -> String {
    let assertion = cargo_bin_cmd!("postproject")
        .args(arguments)
        .assert()
        .success();
    String::from_utf8(assertion.get_output().stdout.clone()).expect("UTF-8 text output")
}

fn run_failure(arguments: &[&str]) -> String {
    let assertion = cargo_bin_cmd!("postproject")
        .arg("--json")
        .args(arguments)
        .assert()
        .failure();
    String::from_utf8(assertion.get_output().stderr.clone()).expect("UTF-8 error output")
}

fn import(production: &str, directory: &Path, name: &str, content: &[u8]) -> Value {
    let path = directory.join(name);
    fs::write(&path, content).expect("write media fixture");
    run_json(&[
        "media",
        "add",
        production,
        path.to_str().expect("UTF-8 media path"),
        "--name",
        name,
    ])
}

fn items(page: &Value) -> &[Value] {
    page["items"].as_array().expect("page items")
}

/// Follows every continuation of a query and returns the collected items.
fn collect_pages(arguments: &[&str]) -> Vec<Value> {
    let mut collected = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut page_arguments = arguments.to_vec();
        page_arguments.extend(["--limit", "1"]);
        if let Some(cursor) = cursor.as_deref() {
            page_arguments.extend(["--cursor", cursor]);
        }
        let page = run_json(&page_arguments);
        assert!(items(&page).len() <= 1);
        collected.extend(items(&page).iter().cloned());
        match page["next_cursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => return collected,
        }
    }
}

#[test]
fn pages_media_structure() {
    let directory = tempfile::tempdir().expect("create test directory");
    let production = directory.path().join("structure.pproj");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    let first = import(production, directory.path(), "first.mov", b"first fixture");
    let second = import(
        production,
        directory.path(),
        "second.mov",
        b"second fixture",
    );

    let legacy = run_json(&["media", "list", production]);
    assert_eq!(legacy.as_array().expect("legacy asset array").len(), 2);

    let first_page = run_json(&["media", "list", production, "--limit", "1"]);
    assert_eq!(items(&first_page).len(), 1);
    assert_eq!(items(&first_page)[0]["representation_count"], 1);
    assert_eq!(first_page["traversal_truncated"], false);
    let assets = collect_pages(&["media", "list", production]);
    let asset_ids: Vec<_> = assets.iter().map(|asset| asset["id"].clone()).collect();
    assert_eq!(
        asset_ids,
        [first["asset_id"].clone(), second["asset_id"].clone()]
    );

    let asset_id = first["asset_id"].as_str().expect("asset ID");
    let representations = run_json(&["representation", "list", production, asset_id]);
    assert_eq!(items(&representations).len(), 1);
    let representation = &items(&representations)[0];
    assert_eq!(representation["id"], first["representation_id"]);
    assert_eq!(representation["asset_id"], asset_id);
    assert_eq!(representation["kind"], "original");
    assert!(representations["next_cursor"].is_null());

    let representation_id = first["representation_id"].as_str().expect("rep ID");
    let resources = run_json(&["representation", "resources", production, representation_id]);
    assert_eq!(items(&resources).len(), 1);
    let resource_id = items(&resources)[0]["id"].as_str().expect("resource ID");
    assert!(items(&resources)[0].get("locators").is_none());

    let shown = run_json(&["representation", "show", production, representation_id]);
    assert_eq!(&shown, representation);
    let users = run_json(&["representation", "using-resource", production, resource_id]);
    assert_eq!(items(&users), &vec![representation.clone()]);
    assert!(users["next_cursor"].is_null());
    let absent = "00000000-0000-0000-0000-000000000001";
    let error = run_failure(&["representation", "show", production, absent]);
    assert!(error.contains("does not exist"), "{error}");
    let error = run_failure(&["media", "show", production, absent]);
    assert!(error.contains("does not exist"), "{error}");

    let locators = run_json(&["locator", "list", production, resource_id]);
    assert_eq!(items(&locators).len(), 1);
    assert_eq!(items(&locators)[0]["id"], first["locator_id"]);
    assert_eq!(items(&locators)[0]["resource_id"], resource_id);
    assert!(items(&locators)[0]["media_root"].is_null());

    let text = run_text(&["media", "list", production, "--limit", "1"]);
    let lines: Vec<_> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[1].starts_with("next_cursor\t"));

    let cursor = first_page["next_cursor"].as_str().expect("asset cursor");
    let error = run_failure(&[
        "representation",
        "list",
        production,
        asset_id,
        "--cursor",
        cursor,
    ]);
    assert!(error.contains("cursor"), "{error}");
}

#[test]
fn finds_every_known_media_candidate_by_locator_and_fingerprint() {
    let directory = tempfile::tempdir().expect("create test directory");
    let production = directory.path().join("known-media.pproj");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    let first = import(production, directory.path(), "shared.mov", b"same bytes");
    let second = import(production, directory.path(), "shared.mov", b"same bytes");
    let uri = first["uri"].as_str().expect("locator URI");

    let by_locator = collect_pages(&["media", "find-by-locator", production, uri]);
    assert_eq!(by_locator.len(), 2);
    for imported in [&first, &second] {
        assert!(by_locator.iter().any(|candidate| {
            candidate["asset_id"] == imported["asset_id"]
                && candidate["representation_id"] == imported["representation_id"]
                && candidate["resource_id"] == imported["resource_id"]
        }));
    }

    let shown = run_json(&[
        "media",
        "show",
        production,
        first["asset_id"].as_str().expect("asset ID"),
    ]);
    let fingerprint = &shown["representations"][0]["resources"][0]["fingerprints"][0];
    let version = fingerprint["version"]
        .as_u64()
        .expect("fingerprint version");
    let version = version.to_string();
    let by_fingerprint = collect_pages(&[
        "media",
        "find-by-fingerprint",
        production,
        fingerprint["algorithm"].as_str().expect("algorithm"),
        &version,
        fingerprint["value_hex"]
            .as_str()
            .expect("fingerprint value"),
    ]);
    assert_eq!(by_fingerprint, by_locator);
}

#[test]
fn queries_media_knowledge_by_root_and_resolution() {
    let directory = tempfile::tempdir().expect("create test directory");
    let production = directory.path().join("roots.pproj");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    let original = directory.path().join("original.mov");
    fs::write(&original, b"root query fixture").expect("write fixture");
    let imported = run_json(&[
        "media",
        "add",
        production,
        original.to_str().expect("UTF-8 media path"),
    ]);
    let asset_id = imported["asset_id"].as_str().expect("asset ID");
    let representation_id = imported["representation_id"].as_str().expect("rep ID");
    assert_eq!(
        items(&run_json(&["media", "unresolved", production])),
        Vec::<Value>::new()
    );

    let inspection = run_json(&["inspect", production]);
    let base = inspection["decision_base"].as_str().unwrap();
    run_json(&[
        "--decision-base",
        base,
        "locator",
        "retire",
        production,
        imported["locator_id"].as_str().expect("locator ID"),
    ]);
    fs::remove_file(&original).expect("remove original");
    let unresolved = run_json(&["media", "unresolved", production]);
    assert_eq!(items(&unresolved).len(), 1);
    assert_eq!(
        items(&unresolved)[0]["representation_id"],
        representation_id
    );

    let relocated = directory.path().join("relocated");
    fs::create_dir(&relocated).expect("create relocated root");
    fs::write(relocated.join("original.mov"), b"root query fixture").expect("relocate fixture");
    run_json(&["root", "add", production, "archive"]);
    assert_eq!(
        items(&run_json(&["media", "under-root", production, "archive"])),
        Vec::<Value>::new()
    );
    let mapping = format!("archive={}", relocated.to_str().expect("UTF-8 root"));
    let resolved = run_json(&[
        "media",
        "resolve",
        production,
        asset_id,
        "--root-map",
        &mapping,
    ]);
    let uri = resolved["resolutions"][0]["resources"][0]["candidates"][0]["uri"]
        .as_str()
        .expect("candidate URI");
    run_json(&[
        "media",
        "resolve",
        production,
        asset_id,
        "--root-map",
        &mapping,
        "--confirm",
        uri,
    ]);

    let resource_id = imported["resource_id"].as_str().expect("resource ID");
    let locators = run_json(&["locator", "list", production, resource_id]);
    let confirmed = items(&locators)
        .iter()
        .find(|locator| locator["uri"] == uri)
        .expect("confirmed locator");
    assert_eq!(confirmed["media_root"], "archive");

    let under_root = run_json(&["media", "under-root", production, "archive"]);
    assert_eq!(items(&under_root).len(), 1);
    assert_eq!(items(&under_root)[0]["id"], representation_id);
    assert_eq!(
        items(&run_json(&["media", "unresolved", production])),
        Vec::<Value>::new()
    );
    let error = run_failure(&["media", "under-root", production, "missing"]);
    assert!(error.starts_with("error:"), "{error}");
}

#[test]
fn queries_metadata_by_exact_scalar_value() {
    let directory = tempfile::tempdir().expect("create test directory");
    let production = directory.path().join("metadata.pproj");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    let mut asset_ids = Vec::new();
    for (name, scene) in [("a.mov", "12"), ("b.mov", "12"), ("c.mov", "14")] {
        let imported = import(production, directory.path(), name, name.as_bytes());
        let asset_id = imported["asset_id"].as_str().expect("asset ID").to_owned();
        run_json(&[
            "metadata",
            "add-text",
            production,
            "asset",
            &asset_id,
            "com.example.slate",
            "scene",
            scene,
        ]);
        asset_ids.push(asset_id);
    }

    let all = collect_pages(&["metadata", "find", production, "com.example.slate", "scene"]);
    assert_eq!(all.len(), 3);

    let value_file = directory.path().join("scene.json");
    fs::write(&value_file, br#"{"type": "string", "value": "12"}"#).expect("write predicate");
    let value_file = value_file.to_str().expect("UTF-8 predicate path");
    let matching = collect_pages(&[
        "metadata",
        "find",
        production,
        "com.example.slate",
        "scene",
        "--value-file",
        value_file,
    ]);
    let mut matched: Vec<_> = matching
        .iter()
        .map(|item| item["target_id"].as_str().expect("target ID").to_owned())
        .collect();
    matched.sort();
    let mut expected = asset_ids[..2].to_vec();
    expected.sort();
    assert_eq!(matched, expected);
    assert!(matching.iter().all(|item| item["value"]["value"] == "12"));

    let list_file = directory.path().join("list.json");
    fs::write(
        &list_file,
        br#"{"type": "list", "values": [{"type": "string", "value": "12"}]}"#,
    )
    .expect("write list predicate");
    let error = run_failure(&[
        "metadata",
        "find",
        production,
        "com.example.slate",
        "scene",
        "--value-file",
        list_file.to_str().expect("UTF-8 list path"),
    ]);
    assert!(error.contains("scalar"), "{error}");
}

fn add_activity(production: &str, kind: &str, input: &str, output: &str, tool: &[&str]) -> Value {
    let mut arguments = vec![
        "activity", "add", production, kind, "--input", input, "--output", output,
    ];
    arguments.extend(tool);
    run_json(&arguments)
}

#[test]
fn queries_bounded_provenance_and_outputs() {
    let directory = tempfile::tempdir().expect("create test directory");
    let production = directory.path().join("provenance.pproj");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    let ids: Vec<String> = ["source.mov", "proxy.mov", "conform.mov"]
        .iter()
        .map(|name| {
            import(production, directory.path(), name, name.as_bytes())["representation_id"]
                .as_str()
                .expect("rep ID")
                .to_owned()
        })
        .collect();
    let transcode = add_activity(
        production,
        TRANSCODE_KIND,
        &ids[0],
        &ids[1],
        &["--tool-name", "FFmpeg", "--tool-version", "8.0"],
    );
    add_activity(
        production,
        CONFORM_KIND,
        &ids[1],
        &ids[2],
        &["--tool-name", "FFmpeg"],
    );

    let producing = run_json(&["activity", "producing", production, &ids[1]]);
    assert_eq!(items(&producing).len(), 1);
    assert_eq!(items(&producing)[0]["id"], transcode["id"]);
    let consuming = run_json(&["activity", "consuming", production, &ids[0]]);
    assert_eq!(items(&consuming)[0]["id"], transcode["id"]);

    let ancestors = run_json(&["activity", "ancestors", production, &ids[2]]);
    let mut depths: Vec<_> = items(&ancestors)
        .iter()
        .map(|item| {
            (
                item["representation_id"].as_str().expect("ID").to_owned(),
                item["depth"].as_u64().expect("depth"),
            )
        })
        .collect();
    depths.sort_by_key(|(_, depth)| *depth);
    assert_eq!(depths, [(ids[1].clone(), 1), (ids[0].clone(), 2)]);
    assert_eq!(ancestors["traversal_truncated"], false);

    let bounded = run_json(&[
        "activity",
        "ancestors",
        production,
        &ids[2],
        "--max-depth",
        "1",
    ]);
    assert_eq!(items(&bounded).len(), 1);
    assert_eq!(items(&bounded)[0]["representation_id"], ids[1]);
    assert_eq!(bounded["traversal_truncated"], true);
    let text = run_text(&[
        "activity",
        "ancestors",
        production,
        &ids[2],
        "--max-depth",
        "1",
    ]);
    assert_eq!(text, format!("{}\t1\ntraversal_truncated\ttrue\n", ids[1]));

    let descendants = collect_pages(&["activity", "descendants", production, &ids[0]]);
    assert_eq!(descendants.len(), 2);
    let first_page = run_json(&[
        "activity",
        "descendants",
        production,
        &ids[0],
        "--limit",
        "1",
    ]);
    let cursor = first_page["next_cursor"].as_str().expect("cursor");
    let error = run_failure(&[
        "activity",
        "descendants",
        production,
        &ids[0],
        "--max-depth",
        "2",
        "--cursor",
        cursor,
    ]);
    assert!(error.contains("cursor"), "{error}");

    assert_activity_outputs(production, &ids);
}

fn assert_activity_outputs(production: &str, ids: &[String]) {
    let by_kind = run_json(&["activity", "outputs", production, "--kind", TRANSCODE_KIND]);
    assert_eq!(items(&by_kind).len(), 1);
    assert_eq!(items(&by_kind)[0]["representation_id"], ids[1]);
    let by_tool = run_json(&["activity", "outputs", production, "--tool-name", "FFmpeg"]);
    assert_eq!(items(&by_tool).len(), 1);
    assert_eq!(items(&by_tool)[0]["representation_id"], ids[2]);
    let by_versioned_tool = run_json(&[
        "activity",
        "outputs",
        production,
        "--tool-name",
        "FFmpeg",
        "--tool-version",
        "8.0",
    ]);
    assert_eq!(items(&by_versioned_tool)[0]["representation_id"], ids[1]);
    cargo_bin_cmd!("postproject")
        .args(["activity", "outputs", production])
        .assert()
        .failure();
    cargo_bin_cmd!("postproject")
        .args([
            "activity",
            "outputs",
            production,
            "--kind",
            TRANSCODE_KIND,
            "--tool-name",
            "FFmpeg",
        ])
        .assert()
        .failure();
}

#[test]
fn queries_stale_artifacts_and_changed_objects() {
    let directory = tempfile::tempdir().expect("create test directory");
    let production = directory.path().join("stale.pproj");
    let production = production.to_str().expect("UTF-8 production path");
    run_json(&["init", production]);
    let source = import(
        production,
        directory.path(),
        "source.mov",
        b"source fixture",
    );
    let unrelated = import(production, directory.path(), "other.mov", b"other fixture");
    let proxy = import(production, directory.path(), "proxy.mov", b"proxy fixture");
    let other_proxy = import(
        production,
        directory.path(),
        "other-proxy.mov",
        b"other proxy",
    );
    let rep = |value: &Value| {
        value["representation_id"]
            .as_str()
            .expect("rep ID")
            .to_owned()
    };
    add_activity(production, TRANSCODE_KIND, &rep(&source), &rep(&proxy), &[]);
    add_activity(
        production,
        TRANSCODE_KIND,
        &rep(&unrelated),
        &rep(&other_proxy),
        &[],
    );
    assert_eq!(
        items(&run_json(&["artifact", "stale", production])),
        Vec::<Value>::new()
    );
    let latest = run_json(&["revisions", "latest", production]);
    let sequence = latest["sequence"].as_u64().expect("sequence").to_string();

    for (imported, name, content) in [
        (&source, "source.mov", b"changed source".as_slice()),
        (&unrelated, "other.mov", b"changed other".as_slice()),
    ] {
        let path = directory.path().join(name);
        fs::write(&path, content).expect("change fixture");
        let resource_id = items(&run_json(&[
            "representation",
            "resources",
            production,
            &rep(imported),
        ]))[0]["id"]
            .as_str()
            .expect("resource ID")
            .to_owned();
        run_json(&[
            "media",
            "fingerprint",
            production,
            "--decision-base",
            run_json(&["inspect", production])["decision_base"]
                .as_str()
                .expect("decision base"),
            &resource_id,
            path.to_str().expect("UTF-8 path"),
        ]);
    }

    let stale = collect_pages(&["artifact", "stale", production]);
    let mut stale_ids: Vec<_> = stale
        .iter()
        .map(|item| item["representation_id"].as_str().expect("ID").to_owned())
        .collect();
    stale_ids.sort();
    let mut expected = vec![rep(&proxy), rep(&other_proxy)];
    expected.sort();
    assert_eq!(stale_ids, expected);
    let scoped = run_json(&["artifact", "stale", production, "--source", &rep(&source)]);
    assert_eq!(items(&scoped).len(), 1);
    assert_eq!(items(&scoped)[0]["representation_id"], rep(&proxy));
    assert_eq!(scoped["traversal_truncated"], false);
    let text = run_text(&["artifact", "stale", production, "--source", &rep(&source)]);
    assert_eq!(
        text,
        format!("{}\ntraversal_truncated\tfalse\n", rep(&proxy))
    );

    assert_changed_objects(production, &sequence, &source, &unrelated);
}

fn assert_changed_objects(production: &str, sequence: &str, source: &Value, unrelated: &Value) {
    let rep = |value: &Value| {
        value["representation_id"]
            .as_str()
            .expect("rep ID")
            .to_owned()
    };
    let changed = collect_pages(&["revisions", "changed", production, "--after", sequence]);
    let mut changed: Vec<_> = changed
        .iter()
        .map(|item| {
            (
                item["kind"].as_str().expect("kind").to_owned(),
                item["id"].as_str().expect("ID").to_owned(),
            )
        })
        .collect();
    changed.sort();
    let mut expected_changed = Vec::new();
    for imported in [source, unrelated] {
        expected_changed.push(("representation".to_owned(), rep(imported)));
        expected_changed.push((
            "resource".to_owned(),
            imported["resource_id"]
                .as_str()
                .expect("resource ID")
                .to_owned(),
        ));
    }
    expected_changed.sort();
    assert_eq!(changed, expected_changed);
    let everything = collect_pages(&["revisions", "changed", production]);
    assert!(
        everything
            .iter()
            .any(|item| item["id"] == source["asset_id"])
    );
    assert!(everything.len() > changed.len());
}
