//! Small actual Git/tar/Cargo controls. No full release graph is packaged here.

use super::*;
use super::super::{
    CargoMetadata, IsolatedRegistry, check_selected_versions,
    inject_registry_deps_into_crate, served_metadata_output, sha256_hex,
    sparse_index_file, validate_served_metadata, write_registry_config,
    write_sparse_index_entry,
};
use serde_json::{Value, json};

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=Release fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false"])
        .args(args).current_dir(root).output().expect("git fixture command");
    assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
}

fn source_fixture() -> TempDir {
    let fixture = tempfile::Builder::new().prefix("adze source λ with spaces-")
        .tempdir().expect("source fixture");
    let root = fixture.path();
    fs::create_dir(root.join("pkg")).unwrap();
    fs::write(root.join("pkg/Cargo.toml"), "[package]\nname='fixture'\nversion='1.0.0'\n").unwrap();
    fs::write(root.join(".gitignore"), "ignored-output\n").unwrap();
    git(root, &["init", "-q"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-qm", "source fixture"]);
    fixture
}

fn new_evidence(source: &Path, parent: &Path) -> Provenance {
    Provenance::create(source, Some(&parent.join("evidence")), "1.0.0", &["fixture".into()]).unwrap()
}

fn archive_fixture() -> TempDir {
    let fixture = tempfile::Builder::new().prefix("adze archive λ with spaces-")
        .tempdir().unwrap();
    let root = fixture.path().join("fixture-1.0.0");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname='fixture'\nversion='1.0.0'\nedition='2021'\n[dependencies]\nalias={package='adze',version='0.10.0'}\n").unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn value() -> u8 { 42 }\n").unwrap();
    fs::write(root.join("binary.dat"), [0, 255, 128, 13, 10, 195, 169, 0, 254]).unwrap();
    pack_fixture(fixture.path(), &fixture.path().join("input.crate"), &["fixture-1.0.0"]);
    fixture
}

fn pack_fixture(parent: &Path, archive: &Path, members: &[&str]) {
    let status = Command::new("tar").arg("-czf").arg(archive)
        .arg("-C").arg(parent).args(members).status().unwrap();
    assert!(status.success(), "tar fixture creation");
}

fn expected() -> BTreeMap<String, String> {
    BTreeMap::from([("adze".into(), "0.10.0".into())])
}

#[test]
fn test_provenance_hashes_arbitrary_binary_bytes_without_text_conversion() {
    let bytes = [0, 255, 128, 13, 10, 195, 169, 0, 254];
    let expected = "9cb93d260e3f6316401e427d9e51620936be415a86135e7689bd84027e09d12d";
    assert_eq!(sha256_hex(&bytes).unwrap(), expected);
    let file = tempfile::NamedTempFile::new().unwrap();
    fs::write(file.path(), bytes).unwrap();
    assert_eq!(sha256_hex_file(file.path()).unwrap(), expected);
}

#[test]
fn test_provenance_rejects_dirty_untracked_and_wrong_version_sources() {
    let fixture = source_fixture();
    let root = fixture.path();
    clean_source_identity(root).unwrap();
    let manifest = fs::read(root.join("pkg/Cargo.toml")).unwrap();
    fs::write(root.join("pkg/Cargo.toml"), "changed").unwrap();
    assert!(clean_source_identity(root).unwrap_err().to_string().contains("requires clean"));
    fs::write(root.join("pkg/Cargo.toml"), manifest).unwrap();
    fs::write(root.join("untracked.rs"), "untracked source").unwrap();
    assert!(clean_source_identity(root).unwrap_err().to_string().contains("requires clean"));
    let versions = BTreeMap::from([("first".into(), "1.0.0".into()), ("last".into(), "1.0.1".into())]);
    let error = check_selected_versions(&versions, "1.0.0").unwrap_err();
    assert!(error.to_string().contains("`last` version `1.0.1`"));
    assert!(check_selected_versions(&versions, "^1.0").is_err());
}

#[test]
fn test_provenance_snapshot_is_owned_and_detects_source_snapshot_archive_changes() {
    let fixture = source_fixture();
    let parent = tempfile::tempdir().unwrap();
    fs::write(fixture.path().join("ignored-output"), "must not enter export").unwrap();
    let mut evidence = new_evidence(fixture.path(), parent.path());
    let snapshot = evidence.snapshot().unwrap();
    assert!(!snapshot.starts_with(fixture.path()));
    assert!(!snapshot.join("ignored-output").exists());
    assert_eq!(evidence.source_commit().unwrap(), git_text(fixture.path(), &["rev-parse", "HEAD"]).unwrap());
    evidence.verify_source(&snapshot).unwrap();
    fs::write(fixture.path().join("new-source.rs"), "changed").unwrap();
    assert!(evidence.verify_source(&snapshot).unwrap_err().to_string().contains("requires clean"));
    fs::remove_file(fixture.path().join("new-source.rs")).unwrap();
    let manifest = snapshot.join("pkg/Cargo.toml");
    let original = fs::read(&manifest).unwrap();
    fs::write(&manifest, "different snapshot").unwrap();
    assert!(evidence.verify_source(&snapshot).unwrap_err().to_string().contains("snapshot changed"));
    fs::write(&manifest, original).unwrap();
    fs::write(evidence.root().join("source.tar"), "replaced export").unwrap();
    assert!(evidence.verify_source(&snapshot).unwrap_err().to_string().contains("source archive changed"));
}

#[test]
fn test_provenance_gitlinks_are_recorded_and_checked_against_selected_roots() {
    let fixture = source_fixture();
    let root = fixture.path();
    let commit = git_text(root, &["rev-parse", "HEAD"]).unwrap();
    fs::create_dir_all(root.join("fixtures/module")).unwrap();
    let cacheinfo = format!("160000,{commit},fixtures/module");
    git(root, &["update-index", "--add", "--cacheinfo", &cacheinfo]);
    git(root, &["commit", "-qm", "record fixture gitlink"]);
    let parent = tempfile::tempdir().unwrap();
    let mut evidence = new_evidence(root, parent.path());
    let snapshot = evidence.snapshot().unwrap();
    let roots = BTreeMap::from([("fixture".into(), snapshot.join("pkg"))]);
    evidence.check_package_roots(&snapshot, &roots).unwrap();
    let source = evidence.receipt.source.as_ref().unwrap();
    assert_eq!(source.gitlinks.len(), 1);
    assert_eq!(source.gitlinks[0].commit, commit);
    assert_eq!(source.gitlinks[0].path, "fixtures/module");
    assert!(source.gitlinks_outside_selected_packages);
    let roots = BTreeMap::from([("fixture".into(), snapshot.join("fixtures"))]);
    assert!(evidence.check_package_roots(&snapshot, &roots).unwrap_err().to_string().contains("unexported gitlink"));
    assert!(!evidence.receipt.source.as_ref().unwrap().gitlinks_outside_selected_packages);
}

#[test]
fn test_provenance_artifact_directory_is_outside_source_and_exclusive() {
    let fixture = source_fixture();
    assert!(Provenance::create(fixture.path(), Some(&fixture.path().join("artifacts")), "1.0.0", &[]).is_err());
    assert!(!fixture.path().join("artifacts").exists());
    let parent = tempfile::tempdir().unwrap();
    fs::create_dir(parent.path().join("evidence")).unwrap();
    fs::write(parent.path().join("evidence/sentinel"), "existing user bytes").unwrap();
    assert!(Provenance::create(fixture.path(), Some(&parent.path().join("evidence")), "1.0.0", &[]).is_err());
    assert_eq!(fs::read_to_string(parent.path().join("evidence/sentinel")).unwrap(), "existing user bytes");
}

#[test]
fn test_provenance_original_and_served_archives_survive_success_and_owner_drop() {
    let fixture = archive_fixture();
    let parent = tempfile::tempdir().unwrap();
    let mut evidence = new_evidence(fixture.path(), parent.path());
    let original_bytes = fs::read(fixture.path().join("input.crate")).unwrap();
    let prepared = evidence.prepare_archive("fixture", "1.0.0", &fixture.path().join("input.crate"), |path| {
        inject_registry_deps_into_crate(path, &expected())
    }).unwrap();
    let record = &evidence.receipt.archives[0];
    assert_eq!(record.changes.len(), 1);
    assert_eq!(record.changes[0].member, "Cargo.toml");
    assert_eq!(fs::read(evidence.root().join(&record.original.as_ref().unwrap().path)).unwrap(), original_bytes);
    assert_eq!(fs::read(prepared.extracted.path().join("fixture-1.0.0/binary.dat")).unwrap(), [0, 255, 128, 13, 10, 195, 169, 0, 254]);
    assert_ne!(record.original.as_ref().unwrap().sha256, record.served.as_ref().unwrap().sha256);
    evidence.finished().unwrap();
    let root = evidence.root().to_path_buf();
    drop(prepared);
    drop(evidence);
    assert_eq!(fs::read(root.join("original/fixture-1.0.0.crate")).unwrap(), original_bytes);
    assert!(root.join("registry/crates/fixture/1.0.0/download").is_file());
    let receipt: Value = serde_json::from_slice(&fs::read(root.join(RECEIPT)).unwrap()).unwrap();
    assert_eq!(receipt["harness_status"], "completed_unqualified");
    assert_eq!(receipt["package_first_qualified"], false);
    assert_eq!(receipt["archive_preparation_complete"], false);
    assert_eq!(receipt["archives"][0]["original"]["path"], "original/fixture-1.0.0.crate");
}

#[test]
fn test_provenance_unapproved_changes_fail_and_keep_partial_evidence() {
    for mutation in ["source", "add", "remove", "lock"] {
        let fixture = archive_fixture();
        let parent = tempfile::tempdir().unwrap();
        let mut evidence = new_evidence(fixture.path(), parent.path());
        let original_bytes = fs::read(fixture.path().join("input.crate")).unwrap();
        let result = evidence.prepare_archive("fixture", "1.0.0", &fixture.path().join("input.crate"), |path| {
            let root = fixture.path().join("fixture-1.0.0");
            match mutation {
                "source" => fs::write(root.join("src/lib.rs"), "changed source")?,
                "add" => fs::write(root.join("extra.txt"), "new member")?,
                "remove" => fs::remove_file(root.join("binary.dat"))?,
                "lock" => fs::write(root.join("Cargo.lock"), "unproved lock rewrite")?,
                _ => unreachable!(),
            }
            pack_fixture(fixture.path(), path, &["fixture-1.0.0"]);
            Ok(())
        });
        let error = match result { Ok(_) => panic!("accepted {mutation}"), Err(error) => error };
        assert!(error.to_string().contains("transformation"), "{error:#}");
        evidence.failed(&error).unwrap();
        let root = evidence.root().to_path_buf();
        drop(evidence);
        assert_eq!(fs::read(root.join("original/fixture-1.0.0.crate")).unwrap(), original_bytes);
        assert!(root.join("registry/crates/fixture/1.0.0/download").is_file());
        let receipt: Value = serde_json::from_slice(&fs::read(root.join(RECEIPT)).unwrap()).unwrap();
        assert_eq!(receipt["harness_status"], "failed");
        assert_eq!(receipt["stage"], "archive:fixture:transform");
        assert_eq!(receipt["package_first_qualified"], false);
        assert_eq!(receipt["archive_preparation_complete"], false);
    }
}

#[test]
fn test_provenance_replaced_original_served_manifest_and_index_are_rejected() {
    let fixture = archive_fixture();
    let parent = tempfile::tempdir().unwrap();
    let mut evidence = new_evidence(fixture.path(), parent.path());
    let prepared = evidence.prepare_archive("fixture", "1.0.0", &fixture.path().join("input.crate"), |path| {
        inject_registry_deps_into_crate(path, &expected())
    }).unwrap();
    let index = evidence.root().join("registry/index/fixture");
    fs::create_dir_all(index.parent().unwrap()).unwrap();
    fs::write(&index, "verified index bytes\n").unwrap();
    evidence.record_index(prepared.slot, &index).unwrap();
    let record = &evidence.receipt.archives[0];
    let paths = [record.original.as_ref().unwrap(), record.served.as_ref().unwrap(),
        record.served_manifest.as_ref().unwrap(), record.sparse_index.as_ref().unwrap()]
        .map(|a| evidence.root().join(&a.path));
    for path in paths {
        evidence.verify_archive_artifacts().unwrap();
        let original = fs::read(&path).unwrap();
        fs::write(&path, "replaced retained artifact").unwrap();
        assert!(evidence.verify_archive_artifacts().unwrap_err().to_string().contains("artifact changed"));
        fs::write(path, original).unwrap();
    }
    prepared.verify_extraction().unwrap();
    fs::write(&prepared.manifest_path, "replaced served extraction").unwrap();
    assert!(prepared.verify_extraction().unwrap_err().to_string().contains("extraction changed"));
}

#[test]
fn test_provenance_archive_manifest_is_exact_root_not_first_nested_match() {
    let fixture = tempfile::tempdir().unwrap();
    let package = fixture.path().join("fixture-1.0.0");
    fs::create_dir_all(package.join("nested")).unwrap();
    fs::write(package.join("nested/Cargo.toml"), "nested only").unwrap();
    let archive = fixture.path().join("input.crate");
    pack_fixture(fixture.path(), &archive, &["fixture-1.0.0"]);
    assert!(archive_manifest_member(&archive).unwrap_err().to_string().contains("exact root Cargo.toml"));
    fs::write(package.join("Cargo.toml"), "root manifest").unwrap();
    pack_fixture(fixture.path(), &archive, &["fixture-1.0.0/nested", "fixture-1.0.0/Cargo.toml"]);
    assert_eq!(archive_manifest_member(&archive).unwrap(), "fixture-1.0.0/Cargo.toml");
    pack_fixture(fixture.path(), &archive, &["fixture-1.0.0", "fixture-1.0.0"]);
    assert!(archive_manifest_member(&archive).unwrap_err().to_string().contains("repeats member"));
}

fn metadata_fixture(manifest_text: &str) -> (TempDir, IsolatedRegistry, PathBuf, String) {
    let fixture = tempfile::Builder::new().prefix("adze metadata λ with spaces-")
        .tempdir().unwrap();
    fs::create_dir(fixture.path().join("src")).unwrap();
    fs::write(fixture.path().join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();
    let manifest = fixture.path().join("Cargo.toml");
    fs::write(&manifest, manifest_text).unwrap();
    let registry = IsolatedRegistry::new().unwrap();
    // --no-deps must interpret this manifest without resolving/fetching this index.
    let index_url = "sparse+http://127.0.0.1:1/index/".to_string();
    write_registry_config(&registry.cargo_home, &index_url, &registry.index_dir, "http://127.0.0.1:1/crates/").unwrap();
    (fixture, registry, manifest, index_url)
}

#[test]
fn test_provenance_served_metadata_retains_alias_target_flags_and_features2_version() {
    let (fixture, registry, manifest, index_url) = metadata_fixture(
        "[package]\nname='fixture'\nversion='1.0.0'\nedition='2021'\n[workspace]\n[dependencies]\nalias={package='adze',version='=0.10.0',registry='adze-local',optional=true,default-features=false}\n[target.'cfg(unix)'.build-dependencies]\nbuilder={package='adze',version='=0.10.0',registry='adze-local',default-features=false}\n[features]\nselected=['dep:alias']\n",
    );
    let output = served_metadata_output(&manifest, registry.cargo_home.path(), registry.target_dir.path()).unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let package = validate_served_metadata(&output.stdout, &manifest, "fixture", "1.0.0", &expected(), &index_url).unwrap();
    assert_eq!(package.dependencies.len(), 2);
    assert!(package.dependencies.iter().all(|d| !d.default_features));
    write_sparse_index_entry(&registry.index_dir, &package, &expected(), "archive-sha-fixture").unwrap();
    let index: Value = serde_json::from_slice(&fs::read(sparse_index_file(&registry.index_dir, "fixture")).unwrap()).unwrap();
    assert_eq!(index["v"], 2);
    assert_eq!(index["features2"]["selected"], json!(["dep:alias"]));
    let deps = index["deps"].as_array().unwrap();
    let alias = deps.iter().find(|d| d["name"] == "alias").unwrap();
    let builder = deps.iter().find(|d| d["name"] == "builder").unwrap();
    for dep in [alias, builder] {
        assert_eq!(dep["package"], "adze");
        assert_eq!(dep["registry"], Value::Null);
        assert_eq!(dep["req"], "=0.10.0");
        assert_eq!(dep["default_features"], false);
    }
    assert_eq!(alias["optional"], true);
    assert_eq!(alias["kind"], "normal");
    assert_eq!(alias["target"], Value::Null);
    assert_eq!(builder["optional"], false);
    assert_eq!(builder["kind"], "build");
    assert_eq!(builder["target"], "cfg(unix)");
    let mut wrong: Value = serde_json::from_slice(&output.stdout).unwrap();
    wrong["workspace_members"] = json!([]);
    assert!(validate_served_metadata(&serde_json::to_vec(&wrong).unwrap(), &manifest, "fixture", "1.0.0", &expected(), &index_url).is_err());
    assert!(validate_served_metadata(&output.stdout, &manifest, "fixture", "1.0.1", &expected(), &index_url).is_err());
    let other = fixture.path().join("other.toml");
    fs::write(&other, "different manifest").unwrap();
    assert!(validate_served_metadata(&output.stdout, &other, "fixture", "1.0.0", &expected(), &index_url).is_err());
}

#[test]
fn test_provenance_native_cargo_package_removed_dev_edge_is_absent_from_served_index() {
    let (fixture, registry, manifest, index_url) = metadata_fixture(
        "[package]\nname='fixture'\nversion='1.0.0'\nedition='2021'\n[workspace]\n[dev-dependencies]\nlocal-dev={path='dev'}\n[features]\nts-compat=['local-dev/ts-compat']\n",
    );
    fs::create_dir_all(fixture.path().join("dev/src")).unwrap();
    fs::write(fixture.path().join("dev/src/lib.rs"), "").unwrap();
    fs::write(fixture.path().join("dev/Cargo.toml"), "[package]\nname='local-dev'\nversion='1.0.0'\nedition='2021'\n[features]\nts-compat=[]\n").unwrap();
    let before = served_metadata_output(&manifest, registry.cargo_home.path(), registry.target_dir.path()).unwrap();
    assert!(before.status.success(), "{}", String::from_utf8_lossy(&before.stderr));
    let source: CargoMetadata = serde_json::from_slice(&before.stdout).unwrap();
    let source_package = source.packages.iter().find(|p| p.name == "fixture").unwrap();
    assert_eq!(source_package.dependencies.len(), 1);
    assert_eq!(source_package.features["ts-compat"], vec!["local-dev/ts-compat".to_string()]);
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let packaged = Command::new(cargo).args(["package", "-p", "fixture", "--offline", "--allow-dirty", "--no-verify", "--manifest-path"])
        .arg(&manifest).current_dir(fixture.path()).env("CARGO_HOME", registry.cargo_home.path())
        .env("CARGO_TARGET_DIR", registry.target_dir.path()).output().unwrap();
    assert!(packaged.status.success(), "{}", String::from_utf8_lossy(&packaged.stderr));
    let parent = tempfile::tempdir().unwrap();
    let mut evidence = new_evidence(fixture.path(), parent.path());
    let archive = registry.target_dir.path().join("package/fixture-1.0.0.crate");
    let prepared = evidence.prepare_archive("fixture", "1.0.0", &archive, |path| {
        inject_registry_deps_into_crate(path, &BTreeMap::new())
    }).unwrap();
    let output = served_metadata_output(&prepared.manifest_path, registry.cargo_home.path(), registry.target_dir.path()).unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let served = validate_served_metadata(&output.stdout, &prepared.manifest_path, "fixture", "1.0.0", &BTreeMap::new(), &index_url).unwrap();
    assert!(served.dependencies.is_empty());
    assert_eq!(served.features["ts-compat"], Vec::<String>::new());
    write_sparse_index_entry(&registry.index_dir, &served, &BTreeMap::new(), &prepared.checksum).unwrap();
    let index: Value = serde_json::from_slice(&fs::read(sparse_index_file(&registry.index_dir, "fixture")).unwrap()).unwrap();
    assert_eq!(index["deps"], json!([]));
    assert_eq!(index["features"]["ts-compat"], json!([]));
    assert_eq!(index["cksum"], prepared.checksum);
}

#[test]
fn test_provenance_native_metadata_foreign_git_and_path_origins_fail_closed() {
    for dependency in [
        "foreign={version='1',registry='foreign'}",
        "foreign={git='https://example.invalid/foreign',version='1'}",
        "foreign={path='foreign',version='1'}",
    ] {
        let text = format!("[package]\nname='fixture'\nversion='1.0.0'\nedition='2021'\n[workspace]\n[dependencies]\n{dependency}\n");
        let (fixture, registry, manifest, index_url) = metadata_fixture(&text);
        let config = registry.cargo_home.path().join("config.toml");
        let mut file = fs::OpenOptions::new().append(true).open(config).unwrap();
        writeln!(file, "\n[registries.foreign]\nindex='sparse+https://example.invalid/index/'").unwrap();
        fs::create_dir_all(fixture.path().join("foreign/src")).unwrap();
        fs::write(fixture.path().join("foreign/src/lib.rs"), "").unwrap();
        fs::write(fixture.path().join("foreign/Cargo.toml"), "[package]\nname='foreign'\nversion='1.0.0'\nedition='2021'\n").unwrap();
        // Exclude the path fixture so even that case has one independently parsed root.
        let text = fs::read_to_string(&manifest).unwrap().replace("[workspace]\n", "[workspace]\nexclude=['foreign']\n");
        fs::write(&manifest, text).unwrap();
        let output = served_metadata_output(&manifest, registry.cargo_home.path(), registry.target_dir.path()).unwrap();
        assert!(output.status.success(), "{dependency}: {}", String::from_utf8_lossy(&output.stderr));
        let error = validate_served_metadata(&output.stdout, &manifest, "fixture", "1.0.0", &BTreeMap::new(), &index_url).unwrap_err();
        assert!(error.to_string().contains("origin") || error.to_string().contains("path dependency"), "{dependency}: {error:#}");
    }
}

#[test]
fn test_provenance_late_harness_failure_remains_failed_after_registry_drop() {
    let source = source_fixture();
    let parent = tempfile::tempdir().unwrap();
    let root = parent.path().join("evidence");
    let mut registry = IsolatedRegistry::new_for_release(source.path(), Some(&root), "1.0.0", &["fixture".into()]).unwrap();
    registry.evidence_stage("cargo_install_unqualified").unwrap();
    let error = registry.finish_evidence(Err(anyhow::anyhow!("original installer failure: status 23"))).unwrap_err();
    assert_eq!(error.to_string(), "original installer failure: status 23");
    drop(registry);
    let receipt: Value = serde_json::from_slice(&fs::read(root.join(RECEIPT)).unwrap()).unwrap();
    assert_eq!(receipt["harness_status"], "failed");
    assert_eq!(receipt["stage"], "cargo_install_unqualified");
    assert_eq!(receipt["error"], "original installer failure: status 23");
    assert_eq!(receipt["package_first_qualified"], false);
}
