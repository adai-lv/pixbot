#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const ROOT_ENV: &str = "PIXBOT_ARCHITECTURE_GUARD_ROOT";
const MODE_ENV: &str = "PIXBOT_ARCHITECTURE_GUARD_MODE";
const SCRIPT_ENV: &str = "PIXBOT_ARCHITECTURE_GUARD_SCRIPT";
const CARGO_ENV: &str = "PIXBOT_ARCHITECTURE_GUARD_CARGO";
const REQUIRED_CRATES: [&str; 10] = [
    "agent",
    "bootstrap",
    "cli",
    "config",
    "context",
    "contracts",
    "headless",
    "providers",
    "session",
    "tools",
];
const ALLOWED_EDGES: [(&str, &str); 16] = [
    ("agent", "contracts"),
    ("context", "contracts"),
    ("providers", "contracts"),
    ("tools", "contracts"),
    ("session", "contracts"),
    ("config", "contracts"),
    ("bootstrap", "agent"),
    ("bootstrap", "context"),
    ("bootstrap", "providers"),
    ("bootstrap", "tools"),
    ("bootstrap", "session"),
    ("bootstrap", "config"),
    ("bootstrap", "contracts"),
    ("headless", "bootstrap"),
    ("headless", "contracts"),
    ("cli", "headless"),
];

#[derive(Debug)]
struct GuardError {
    category: &'static str,
    detail: String,
}

impl GuardError {
    fn new(category: &'static str, detail: impl Into<String>) -> Self {
        Self {
            category,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for GuardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ERROR {}: {}", self.category, self.detail)
    }
}

type GuardResult<T> = Result<T, GuardError>;

#[test]
fn architecture_guard_entrypoint() {
    let mode = env::var(MODE_ENV).unwrap_or_else(|_| "ordinary".to_owned());
    let result = match mode.as_str() {
        "ordinary" => Ok(()),
        "check" => guard_from_environment(),
        "self-test" => self_test_from_environment(),
        _ => Err(GuardError::new(
            "ARGUMENT",
            format!("unsupported internal guard mode: {mode}"),
        )),
    };

    if let Err(error) = result {
        eprintln!("{error}");
        panic!("architecture guard entrypoint failed");
    }
}

fn guard_from_environment() -> GuardResult<()> {
    let root = env::var_os(ROOT_ENV).ok_or_else(|| {
        GuardError::new(
            "ARGUMENT",
            format!("missing environment variable {ROOT_ENV}"),
        )
    })?;
    let root = PathBuf::from(root);
    guard_root(&root)?;
    println!(
        "PASS ARCHITECTURE_GUARD: validated 10 crates at {}",
        root.display()
    );
    Ok(())
}

fn guard_root(root: &Path) -> GuardResult<()> {
    let apps_dir = root.join("apps");
    let crates_dir = apps_dir.join("crates");
    let root_manifest = apps_dir.join("Cargo.toml");

    require_directory(root, "ROOT_LAYOUT")?;
    require_directory(&apps_dir, "ROOT_LAYOUT")?;
    require_directory(&crates_dir, "CRATE_LAYOUT")?;
    read_text(&root_manifest, "ROOT_MANIFEST_READ")?;

    let mut manifests = BTreeMap::new();
    for crate_name in REQUIRED_CRATES {
        let crate_dir = crates_dir.join(crate_name);
        require_directory(&crate_dir, "CRATE_LAYOUT")?;
        let manifest_path = crate_dir.join("Cargo.toml");
        let manifest = read_text(&manifest_path, "MEMBER_MANIFEST_READ")?;
        check_no_private_version(&manifest_path, &manifest)?;
        manifests.insert(crate_name, manifest_path);
    }

    let metadata = load_metadata(&apps_dir)?;
    validate_metadata(&metadata, &apps_dir)?;

    for crate_name in REQUIRED_CRATES {
        let source_dir = crates_dir.join(crate_name).join("src");
        scan_public_traits(crate_name, &source_dir, 0)?;
    }

    Ok(())
}

fn require_directory(path: &Path, category: &'static str) -> GuardResult<()> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(GuardError::new(
            category,
            format!(
                "required directory is missing or not a directory: {}",
                path.display()
            ),
        ))
    }
}

fn read_text(path: &Path, category: &'static str) -> GuardResult<String> {
    fs::read_to_string(path).map_err(|error| {
        GuardError::new(category, format!("cannot read {}: {error}", path.display()))
    })
}

fn load_metadata(apps_dir: &Path) -> GuardResult<Value> {
    let cargo = env::var_os(CARGO_ENV).unwrap_or_else(|| "cargo".into());
    let output = Command::new(&cargo)
        .args(["metadata", "--no-deps", "--format-version=1"])
        .current_dir(apps_dir)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .map_err(|error| {
            GuardError::new(
                "METADATA_COMMAND_SPAWN",
                format!(
                    "cannot execute {:?} for {}: {error}",
                    cargo,
                    apps_dir.display()
                ),
            )
        })?;

    if !output.status.success() {
        return Err(GuardError::new(
            "METADATA_COMMAND",
            format!(
                "cargo metadata exited {} for {}: {}",
                display_status(&output),
                apps_dir.display(),
                bounded_diagnostic(&output.stderr)
            ),
        ));
    }

    serde_json::from_slice(&output.stdout).map_err(|error| {
        GuardError::new(
            "METADATA_PARSE",
            format!(
                "invalid cargo metadata JSON for {}: {error}; stdout: {}",
                apps_dir.display(),
                bounded_diagnostic(&output.stdout)
            ),
        )
    })
}

fn display_status(output: &Output) -> String {
    output
        .status
        .code()
        .map_or_else(|| "due to signal".to_owned(), |code| code.to_string())
}

fn bounded_diagnostic(bytes: &[u8]) -> String {
    const LIMIT: usize = 2_000;
    let text = String::from_utf8_lossy(bytes);
    if text.len() <= LIMIT {
        return text.trim().to_owned();
    }
    let mut boundary = LIMIT;
    while !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}…", text[..boundary].trim())
}

fn validate_metadata(metadata: &Value, _apps_dir: &Path) -> GuardResult<()> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| GuardError::new("METADATA_SCHEMA", "packages must be an array"))?;
    let member_ids = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or_else(|| GuardError::new("METADATA_SCHEMA", "workspace_members must be an array"))?;

    if member_ids.len() != REQUIRED_CRATES.len() || packages.len() != REQUIRED_CRATES.len() {
        return Err(GuardError::new(
            "MEMBER_SET",
            format!(
                "expected exactly 10 workspace packages, got {} members and {} packages",
                member_ids.len(),
                packages.len()
            ),
        ));
    }

    let required: BTreeSet<&str> = REQUIRED_CRATES.into_iter().collect();
    let allowed: BTreeSet<(&str, &str)> = ALLOWED_EDGES.into_iter().collect();
    let mut packages_by_id = BTreeMap::new();
    let mut actual_names = BTreeSet::new();

    for package in packages {
        let id = package
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| GuardError::new("METADATA_SCHEMA", "package id must be a string"))?;
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| GuardError::new("METADATA_SCHEMA", "package name must be a string"))?;
        if !actual_names.insert(name) || packages_by_id.insert(id, package).is_some() {
            return Err(GuardError::new(
                "MEMBER_SET",
                format!("duplicate workspace package name or id: {name} / {id}"),
            ));
        }
    }

    if actual_names != required {
        return Err(GuardError::new(
            "MEMBER_SET",
            format!("workspace crate set differs from required set: {actual_names:?}"),
        ));
    }

    for member_id in member_ids {
        let member_id = member_id.as_str().ok_or_else(|| {
            GuardError::new("METADATA_SCHEMA", "workspace member id must be a string")
        })?;
        if !packages_by_id.contains_key(member_id) {
            return Err(GuardError::new(
                "MEMBER_SET",
                format!("workspace member has no package record: {member_id}"),
            ));
        }
    }

    let mut production_edges = BTreeSet::new();
    for package in packages {
        let source = package
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| GuardError::new("METADATA_SCHEMA", "package name must be a string"))?;
        let manifest_path = package
            .get("manifest_path")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                GuardError::new("METADATA_SCHEMA", "package manifest_path must be a string")
            })?;
        let dependencies = package
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GuardError::new(
                    "METADATA_SCHEMA",
                    format!("dependencies must be an array for {source}"),
                )
            })?;

        for dependency in dependencies {
            let is_production = dependency.get("kind").map(Value::is_null).unwrap_or(false);
            if !is_production {
                continue;
            }
            let target = dependency
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    GuardError::new(
                        "METADATA_SCHEMA",
                        format!("dependency name must be a string for {source}"),
                    )
                })?;
            if !required.contains(target) {
                continue;
            }
            if !allowed.contains(&(source, target)) {
                let target_scope = dependency
                    .get("target")
                    .and_then(Value::as_str)
                    .unwrap_or("all targets");
                return Err(GuardError::new(
                    "DEPENDENCY_EDGE",
                    format!(
                        "illegal internal production edge {source} -> {target} at {manifest_path} [{target_scope}]"
                    ),
                ));
            }
            production_edges.insert((source, target));
        }
    }

    Ok(())
}

fn check_no_private_version(path: &Path, manifest: &str) -> GuardResult<()> {
    for (index, line) in manifest.lines().enumerate() {
        if matches_a2_version_pattern(line) {
            return Err(GuardError::new(
                "DEPENDENCY_VERSION",
                format!(
                    "private version assignment at {}:{}; versions must use workspace inheritance",
                    path.display(),
                    index + 1
                ),
            ));
        }
    }
    Ok(())
}

fn matches_a2_version_pattern(line: &str) -> bool {
    let bytes = line.as_bytes();
    for start in 0..bytes.len().saturating_sub(6) {
        if &bytes[start..start + 7] != b"version" {
            continue;
        }
        let mut cursor = start + 7;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) == Some(&b'"') {
            return true;
        }
    }
    false
}

fn scan_public_traits(crate_name: &str, directory: &Path, depth: usize) -> GuardResult<()> {
    if depth > 64 {
        return Err(GuardError::new(
            "PORT_SCAN",
            format!("source directory depth exceeds 64: {}", directory.display()),
        ));
    }
    let entries = fs::read_dir(directory).map_err(|error| {
        GuardError::new(
            "PORT_SCAN",
            format!("cannot read directory {}: {error}", directory.display()),
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            GuardError::new(
                "PORT_SCAN",
                format!(
                    "cannot read an entry under {}: {error}",
                    directory.display()
                ),
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            GuardError::new(
                "PORT_SCAN",
                format!("cannot inspect {}: {error}", path.display()),
            )
        })?;
        if file_type.is_dir() {
            scan_public_traits(crate_name, &path, depth + 1)?;
        } else if file_type.is_file() && path.extension().is_some_and(|value| value == "rs") {
            let source = read_text(&path, "PORT_SCAN")?;
            if crate_name != "contracts" {
                for (index, line) in source.lines().enumerate() {
                    if is_public_trait_declaration(line) {
                        return Err(GuardError::new(
                            "PORT_LOCATION",
                            format!(
                                "public trait outside contracts at {}:{}",
                                path.display(),
                                index + 1
                            ),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn is_public_trait_declaration(line: &str) -> bool {
    let line = line.trim_start();
    let Some(after_pub) = line.strip_prefix("pub") else {
        return false;
    };
    if !after_pub.starts_with(char::is_whitespace) {
        return false;
    }
    let after_pub = after_pub.trim_start();
    after_pub
        .strip_prefix("trait")
        .is_some_and(|rest| rest.starts_with(char::is_whitespace))
}

fn self_test_from_environment() -> GuardResult<()> {
    let root = PathBuf::from(env::var_os(ROOT_ENV).ok_or_else(|| {
        GuardError::new(
            "SELF_TEST",
            format!("missing environment variable {ROOT_ENV}"),
        )
    })?);
    let script = PathBuf::from(env::var_os(SCRIPT_ENV).ok_or_else(|| {
        GuardError::new(
            "SELF_TEST",
            format!("missing environment variable {SCRIPT_ENV}"),
        )
    })?);

    expect_success(&script, &root, "real-workspace")?;
    let legal = legal_fixture("legal")?;
    expect_success(&script, &legal.root, "legal-fixture")?;

    let illegal_edges = [
        ("agent-context", "agent", "context"),
        ("providers-tools", "providers", "tools"),
        ("agent-providers", "agent", "providers"),
        ("contracts-agent", "contracts", "agent"),
        ("headless-agent", "headless", "agent"),
        ("headless-providers", "headless", "providers"),
        ("cli-bootstrap", "cli", "bootstrap"),
        ("cli-contracts", "cli", "contracts"),
    ];
    for (label, source, target) in illegal_edges {
        let fixture = legal_fixture(label)?;
        append_dependency(&fixture, source, target, false)?;
        expect_rejection(
            &script,
            &fixture.root,
            label,
            "DEPENDENCY_EDGE",
            &[source, target],
            None,
        )?;
    }

    let target_specific = legal_fixture("target-specific-agent-context")?;
    append_dependency(&target_specific, "agent", "context", true)?;
    expect_rejection(
        &script,
        &target_specific.root,
        "target-specific-agent-context",
        "DEPENDENCY_EDGE",
        &["agent", "context", "cfg(windows)"],
        None,
    )?;

    let port = legal_fixture("port-location")?;
    fs::write(
        port.root.join("apps/crates/agent/src/lib.rs"),
        "pub trait OutOfPlacePort {}\n",
    )
    .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
    expect_rejection(
        &script,
        &port.root,
        "port-location",
        "PORT_LOCATION",
        &["agent/src/lib.rs"],
        None,
    )?;

    let version = legal_fixture("private-version")?;
    let manifest_path = version.root.join("apps/crates/providers/Cargo.toml");
    let manifest = read_text(&manifest_path, "SELF_TEST")?;
    let changed = manifest.replace(
        "contracts.workspace = true",
        "contracts = { workspace = true, version = \"0.1.0\" }",
    );
    fs::write(&manifest_path, changed)
        .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
    expect_rejection(
        &script,
        &version.root,
        "private-version",
        "DEPENDENCY_VERSION",
        &["providers/Cargo.toml"],
        None,
    )?;

    run_fault_cases(&script)?;
    println!(
        "SELF_TEST PASS: real=1 legal=1 invalid=11 faults=9; every child status and diagnostic was asserted"
    );
    Ok(())
}

fn run_fault_cases(script: &Path) -> GuardResult<()> {
    let missing_root = TempDir::new("missing-root")?;
    let absent_root = missing_root.path.join("absent-repository");
    expect_rejection(
        script,
        &absent_root,
        "missing-root",
        "ROOT_LAYOUT",
        &["absent-repository"],
        None,
    )?;

    let missing_crates = legal_fixture("missing-crates-dir")?;
    fs::remove_dir_all(missing_crates.root.join("apps/crates"))
        .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
    expect_rejection(
        script,
        &missing_crates.root,
        "missing-crates-dir",
        "CRATE_LAYOUT",
        &["apps/crates"],
        None,
    )?;

    let missing_crate = legal_fixture("missing-crate-dir")?;
    fs::remove_dir_all(missing_crate.root.join("apps/crates/agent"))
        .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
    expect_rejection(
        script,
        &missing_crate.root,
        "missing-crate-dir",
        "CRATE_LAYOUT",
        &["crates/agent"],
        None,
    )?;

    let missing_root_manifest = legal_fixture("missing-root-manifest")?;
    fs::remove_file(missing_root_manifest.root.join("apps/Cargo.toml"))
        .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
    expect_rejection(
        script,
        &missing_root_manifest.root,
        "missing-root-manifest",
        "ROOT_MANIFEST_READ",
        &["apps/Cargo.toml"],
        None,
    )?;

    let missing_member_manifest = legal_fixture("missing-member-manifest")?;
    fs::remove_file(
        missing_member_manifest
            .root
            .join("apps/crates/agent/Cargo.toml"),
    )
    .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
    expect_rejection(
        script,
        &missing_member_manifest.root,
        "missing-member-manifest",
        "MEMBER_MANIFEST_READ",
        &["agent/Cargo.toml"],
        None,
    )?;

    let malformed_root = legal_fixture("malformed-root-manifest")?;
    fs::write(malformed_root.root.join("apps/Cargo.toml"), "[workspace\n")
        .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
    expect_rejection(
        script,
        &malformed_root.root,
        "malformed-root-manifest",
        "METADATA_COMMAND",
        &["apps"],
        None,
    )?;

    let malformed_member = legal_fixture("malformed-member-manifest")?;
    fs::write(
        malformed_member.root.join("apps/crates/agent/Cargo.toml"),
        "[package\n",
    )
    .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
    expect_rejection(
        script,
        &malformed_member.root,
        "malformed-member-manifest",
        "METADATA_COMMAND",
        &["agent/Cargo.toml"],
        None,
    )?;

    let malformed_metadata = legal_fixture("malformed-metadata")?;
    let malformed_runner = create_fake_cargo(
        &malformed_metadata,
        "malformed-metadata-cargo",
        "#!/bin/sh\nprintf '%s\\n' '{not-json'\n",
    )?;
    expect_rejection(
        script,
        &malformed_metadata.root,
        "malformed-metadata",
        "METADATA_PARSE",
        &["invalid cargo metadata JSON"],
        Some(&malformed_runner),
    )?;

    let failed_command = legal_fixture("failed-command")?;
    let failed_runner = create_fake_cargo(
        &failed_command,
        "failed-command-cargo",
        "#!/bin/sh\nprintf '%s\\n' 'simulated metadata command failure' >&2\nexit 42\n",
    )?;
    expect_rejection(
        script,
        &failed_command.root,
        "failed-command",
        "METADATA_COMMAND",
        &["42", "simulated metadata command failure"],
        Some(&failed_runner),
    )?;
    Ok(())
}

fn expect_success(script: &Path, root: &Path, label: &str) -> GuardResult<()> {
    let output = run_child_guard(script, root, None)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(GuardError::new(
            "SELF_TEST",
            format!(
                "{label} unexpectedly exited {}: {}",
                display_status(&output),
                bounded_diagnostic(&output.stderr)
            ),
        ))
    }
}

fn expect_rejection(
    script: &Path,
    root: &Path,
    label: &str,
    category: &str,
    needles: &[&str],
    cargo: Option<&Path>,
) -> GuardResult<()> {
    let output = run_child_guard(script, root, cargo)?;
    if output.status.success() {
        return Err(GuardError::new(
            "SELF_TEST",
            format!("{label} was not rejected"),
        ));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_category = format!("ERROR {category}:");
    if !stderr.contains(&expected_category) {
        return Err(GuardError::new(
            "SELF_TEST",
            format!(
                "{label} exited {} without {expected_category}: {}",
                display_status(&output),
                bounded_diagnostic(&output.stderr)
            ),
        ));
    }
    for needle in needles {
        if !stderr.contains(needle) {
            return Err(GuardError::new(
                "SELF_TEST",
                format!(
                    "{label} diagnostic lacks {needle:?}: {}",
                    bounded_diagnostic(&output.stderr)
                ),
            ));
        }
    }
    if stderr.contains("PASS ARCHITECTURE_GUARD") || stdout.contains("PASS ARCHITECTURE_GUARD") {
        return Err(GuardError::new(
            "SELF_TEST",
            format!("{label} emitted a success conclusion while failing"),
        ));
    }
    Ok(())
}

fn run_child_guard(script: &Path, root: &Path, cargo: Option<&Path>) -> GuardResult<Output> {
    let mut command = Command::new(script);
    command.arg("--root").arg(root);
    if let Some(cargo) = cargo {
        command.env(CARGO_ENV, cargo);
    } else {
        command.env_remove(CARGO_ENV);
    }
    command.output().map_err(|error| {
        GuardError::new(
            "SELF_TEST",
            format!("cannot run child guard {}: {error}", script.display()),
        )
    })
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> GuardResult<Self> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?
            .as_nanos();
        for sequence in 0..1_000_u32 {
            let path = env::temp_dir().join(format!(
                "pixbot-architecture-guard-{}-{timestamp}-{sequence}-{label}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(GuardError::new(
                        "SELF_TEST",
                        format!("cannot create {}: {error}", path.display()),
                    ));
                }
            }
        }
        Err(GuardError::new(
            "SELF_TEST",
            format!("cannot allocate a unique temporary directory for {label}"),
        ))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best-effort cleanup must not panic during another test failure.
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
}

fn legal_fixture(label: &str) -> GuardResult<Fixture> {
    let temp = TempDir::new(label)?;
    let root = temp.path.join("repository");
    let crates_dir = root.join("apps/crates");
    fs::create_dir_all(&crates_dir)
        .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;

    let workspace_dependencies = REQUIRED_CRATES
        .iter()
        .filter(|name| **name != "cli")
        .map(|name| format!("{name} = {{ path = \"crates/{name}\" }}\n"))
        .collect::<String>();
    let root_manifest = format!(
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.93\"\n\n[workspace.dependencies]\n{workspace_dependencies}"
    );
    fs::write(root.join("apps/Cargo.toml"), root_manifest)
        .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;

    for source in REQUIRED_CRATES {
        let crate_dir = crates_dir.join(source);
        fs::create_dir_all(crate_dir.join("src"))
            .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
        let mut manifest = format!(
            "[package]\nname = \"{source}\"\nversion.workspace = true\nedition.workspace = true\nrust-version.workspace = true\n\n[dependencies]\n"
        );
        for (allowed_source, target) in ALLOWED_EDGES {
            if allowed_source == source && !(source == "headless" && target == "contracts") {
                manifest.push_str(&format!("{target}.workspace = true\n"));
            }
        }
        if source == "headless" {
            manifest.push_str("\n[target.'cfg(unix)'.dependencies]\ncontracts.workspace = true\n");
        }
        fs::write(crate_dir.join("Cargo.toml"), manifest)
            .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
        fs::write(
            crate_dir.join("src/lib.rs"),
            format!("//! Temporary legal fixture for {source}.\n"),
        )
        .map_err(|error| GuardError::new("SELF_TEST", error.to_string()))?;
    }

    Ok(Fixture { _temp: temp, root })
}

fn append_dependency(
    fixture: &Fixture,
    source: &str,
    target: &str,
    target_specific: bool,
) -> GuardResult<()> {
    let path = fixture
        .root
        .join("apps/crates")
        .join(source)
        .join("Cargo.toml");
    let mut manifest = read_text(&path, "SELF_TEST")?;
    if target_specific {
        manifest.push_str(&format!(
            "\n[target.'cfg(windows)'.dependencies]\n{target}.workspace = true\n"
        ));
    } else {
        manifest.push_str(&format!("{target}.workspace = true\n"));
    }
    fs::write(&path, manifest).map_err(|error| {
        GuardError::new(
            "SELF_TEST",
            format!("cannot modify {}: {error}", path.display()),
        )
    })
}

fn create_fake_cargo(fixture: &Fixture, label: &str, body: &str) -> GuardResult<PathBuf> {
    let path = fixture.root.join(label);
    fs::write(&path, body).map_err(|error| {
        GuardError::new(
            "SELF_TEST",
            format!("cannot create fake cargo {}: {error}", path.display()),
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(|error| {
            GuardError::new(
                "SELF_TEST",
                format!(
                    "cannot make fake cargo executable {}: {error}",
                    path.display()
                ),
            )
        })?;
    }
    Ok(path)
}

#[test]
fn metadata_guard_accepts_complete_legal_graph_and_target_specific_edges() {
    let metadata = legal_metadata();
    validate_metadata(&metadata, Path::new("/fixture/apps"))
        .expect("complete legal metadata must pass");
}

#[test]
fn metadata_guard_rejects_illegal_edge_with_manifest_diagnostic() {
    let mut metadata = legal_metadata();
    metadata["packages"][0]["dependencies"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "context", "kind": null, "target": null}));

    let error = validate_metadata(&metadata, Path::new("/fixture/apps")).unwrap_err();
    assert_eq!(error.category, "DEPENDENCY_EDGE");
    assert!(error.detail.contains("agent -> context"));
    assert!(error.detail.contains("agent/Cargo.toml"));
}

#[test]
fn metadata_guard_ignores_dev_and_build_edges_for_direction_only() {
    let mut metadata = legal_metadata();
    let dependencies = metadata["packages"][0]["dependencies"]
        .as_array_mut()
        .unwrap();
    dependencies.push(json!({"name": "context", "kind": "dev", "target": null}));
    dependencies.push(json!({"name": "providers", "kind": "build", "target": null}));

    validate_metadata(&metadata, Path::new("/fixture/apps"))
        .expect("dev/build edges are outside direction guard scope");
}

#[test]
fn raw_version_guard_matches_a2_semantics_and_allows_workspace_inheritance() {
    assert!(matches_a2_version_pattern("serde = { version = \"1\" }"));
    assert!(matches_a2_version_pattern("version\t = \t\"1\""));
    assert!(matches_a2_version_pattern("notversion = \"1\""));
    assert!(!matches_a2_version_pattern("version.workspace = true"));
    assert!(!matches_a2_version_pattern("serde.workspace = true"));
}

#[test]
fn public_trait_scanner_accepts_private_traits_and_comments() {
    assert!(is_public_trait_declaration("pub trait Port: Send {}"));
    assert!(is_public_trait_declaration("  pub\ttrait Port {}"));
    assert!(!is_public_trait_declaration("pub(crate) trait Internal {}"));
    assert!(!is_public_trait_declaration("trait Private {}"));
    assert!(!is_public_trait_declaration(
        "/// pub trait DocumentedExample {}"
    ));
}

fn legal_metadata() -> Value {
    let packages = REQUIRED_CRATES
        .iter()
        .map(|source| {
            let dependencies = ALLOWED_EDGES
                .iter()
                .filter(|(allowed_source, _)| allowed_source == source)
                .map(|(_, target)| {
                    let target_scope = if *source == "headless" && *target == "contracts" {
                        "cfg(unix)"
                    } else {
                        "all targets"
                    };
                    json!({"name": target, "kind": null, "target": target_scope})
                })
                .collect::<Vec<_>>();
            json!({
                "id": format!("path+file:///fixture/apps/crates/{source}#{source}@0.1.0"),
                "name": source,
                "manifest_path": format!("/fixture/apps/crates/{source}/Cargo.toml"),
                "dependencies": dependencies,
            })
        })
        .collect::<Vec<_>>();
    let workspace_members = packages
        .iter()
        .map(|package| package["id"].clone())
        .collect::<Vec<_>>();
    json!({"packages": packages, "workspace_members": workspace_members})
}
