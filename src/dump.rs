use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct DumpCard {
    address: String,
    bibtex: Option<String>,
    text: String,
}

struct DumpPayload {
    mapping: BTreeMap<String, String>,
    files: Vec<(String, String)>,
}

impl DumpCard {
    pub(crate) fn new(address: String, text: String) -> Self {
        Self {
            address,
            bibtex: None,
            text,
        }
    }

    pub(crate) fn literature(address: String, bibtex: String, text: String) -> Self {
        Self {
            address,
            bibtex: Some(bibtex),
            text,
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Platform {
    Windows,
    Unix,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompressionFormat {
    Zip,
    TarGz,
    TarZst,
    SevenZip,
}

impl CompressionFormat {
    const fn name(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::TarGz => "tar.gz",
            Self::TarZst => "tar.zst",
            Self::SevenZip => "7z",
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            Self::Zip => ".zip",
            Self::TarGz => ".tar.gz",
            Self::TarZst => ".tar.zst",
            Self::SevenZip => ".7z",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Backend {
    WindowsTar,
    UnixZip,
    UnixTarGzip,
    UnixTarZstd,
    SevenZip(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Candidate {
    format: CompressionFormat,
    backend: Backend,
}

#[derive(Debug, Eq, PartialEq)]
struct CommandSpec {
    program: &'static str,
    args: Vec<OsString>,
    current_dir: Option<PathBuf>,
}

impl CommandSpec {
    fn new(program: &'static str) -> Self {
        Self {
            program,
            args: Vec::new(),
            current_dir: None,
        }
    }

    fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    fn current_dir(mut self, path: &Path) -> Self {
        self.current_dir = Some(path.to_path_buf());
        self
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CompressionPlan {
    Single(CommandSpec),
    Pipeline {
        tar: CommandSpec,
        compressor: CommandSpec,
    },
}

#[cfg(test)]
impl CompressionPlan {
    fn single<I, S>(program: &'static str, args: I, current_dir: Option<&Path>) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut command = CommandSpec::new(program).args(args);
        if let Some(path) = current_dir {
            command = command.current_dir(path);
        }
        Self::Single(command)
    }

    fn pipeline<I, S, J, T>(tar_args: I, compressor: &'static str, compressor_args: J) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
        J: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        Self::Pipeline {
            tar: CommandSpec::new("tar").args(tar_args),
            compressor: CommandSpec::new(compressor).args(compressor_args),
        }
    }
}

impl Candidate {
    const fn new(format: CompressionFormat, backend: Backend) -> Self {
        Self { format, backend }
    }
}

fn compression_candidates(platform: Platform) -> Vec<Candidate> {
    match platform {
        Platform::Windows => vec![
            Candidate::new(CompressionFormat::Zip, Backend::WindowsTar),
            Candidate::new(CompressionFormat::TarGz, Backend::WindowsTar),
            Candidate::new(CompressionFormat::TarZst, Backend::WindowsTar),
            Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7z.exe")),
            Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7zz.exe")),
        ],
        Platform::Unix => vec![
            Candidate::new(CompressionFormat::Zip, Backend::UnixZip),
            Candidate::new(CompressionFormat::TarGz, Backend::UnixTarGzip),
            Candidate::new(CompressionFormat::TarZst, Backend::UnixTarZstd),
            Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7z")),
            Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7zz")),
        ],
    }
}

fn detect_with<F>(candidates: Vec<Candidate>, mut probe: F) -> Vec<Candidate>
where
    F: FnMut(Candidate) -> bool,
{
    let mut detected = Vec::new();
    for candidate in candidates {
        if detected
            .iter()
            .any(|item: &Candidate| item.format == candidate.format)
        {
            continue;
        }
        if probe(candidate) {
            detected.push(candidate);
        }
    }
    detected
}

fn current_platform() -> Platform {
    #[cfg(windows)]
    {
        Platform::Windows
    }
    #[cfg(not(windows))]
    {
        Platform::Unix
    }
}

fn require_detected_options(options: Vec<Candidate>) -> Result<Vec<Candidate>> {
    if options.is_empty() {
        bail!(
            "no supported compression option detected; supported options: zip, tar.gz, tar.zst, 7z"
        );
    }
    Ok(options)
}

struct DumpArtifacts {
    staging: PathBuf,
    archive: PathBuf,
    archive_owned: bool,
    keep_archive: bool,
}

impl DumpArtifacts {
    fn new(staging: PathBuf, archive: PathBuf) -> Self {
        Self {
            staging,
            archive,
            archive_owned: false,
            keep_archive: false,
        }
    }

    fn publish(&mut self, partial: &Path) -> Result<()> {
        let mut source = fs::File::open(partial)
            .with_context(|| format!("failed to open {}", partial.display()))?;
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.archive)
            .with_context(|| format!("archive already exists: {}", self.archive.display()))?;
        self.archive_owned = true;
        io::copy(&mut source, &mut destination)
            .with_context(|| format!("failed to write {}", self.archive.display()))?;
        destination.flush()?;
        destination.sync_all()?;
        Ok(())
    }

    fn remove_staging(&self) -> Result<()> {
        if self.staging.exists() {
            fs::remove_dir_all(&self.staging)
                .with_context(|| format!("failed to remove {}", self.staging.display()))?;
        }
        self.remove_empty_staging_root();
        Ok(())
    }

    fn remove_empty_staging_root(&self) {
        let Some(parent) = self.staging.parent() else {
            return;
        };
        if parent.file_name().is_some_and(|name| name == ".tmp") {
            fs::remove_dir(parent).ok();
        }
    }

    fn commit(mut self) -> Result<()> {
        self.remove_staging()?;
        self.keep_archive = true;
        Ok(())
    }
}

impl Drop for DumpArtifacts {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.staging).ok();
        self.remove_empty_staging_root();
        if self.archive_owned && !self.keep_archive {
            fs::remove_file(&self.archive).ok();
        }
    }
}

pub(crate) fn run(root: &Path, cards: &[DumpCard]) -> Result<()> {
    let options = require_detected_options(detect_compression_options())?;

    println!("compression options:");
    for (index, option) in options.iter().enumerate() {
        println!("  {}. {}", index + 1, option.format.name());
    }
    print!("select compression: ");
    io::stdout().flush()?;
    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim_end_matches(['\r', '\n']);
    let selected = options
        .iter()
        .enumerate()
        .find(|(index, _)| choice == (index + 1).to_string())
        .map(|(_, option)| *option)
        .context("invalid compression option")?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time is before Unix epoch")?
        .as_secs();
    let archive_name = format!("zt-archive-{timestamp}");
    let dump_root = root.join("dump");
    let staging = dump_root.join(".tmp").join(&archive_name);
    let archive_path = dump_root.join(format!("{archive_name}{}", selected.format.extension()));
    if archive_path.exists() {
        bail!("archive already exists: {}", archive_path.display());
    }

    fs::create_dir_all(&staging)
        .with_context(|| format!("failed to create {}", staging.display()))?;
    let mut artifacts = DumpArtifacts::new(staging.clone(), archive_path.clone());
    let payload = build_payload(cards)?;
    fs::write(
        staging.join("mapping.json"),
        serde_json::to_vec(&payload.mapping)?,
    )?;
    for (filename, text) in &payload.files {
        fs::write(staging.join(filename), text.as_bytes())?;
    }
    let mut entries = vec![format!("{archive_name}/mapping.json")];
    entries.extend(
        payload
            .files
            .iter()
            .map(|(filename, _)| format!("{archive_name}/{filename}")),
    );
    let partial_archive = staging.join(format!(
        ".{archive_name}.partial{}",
        selected.format.extension()
    ));
    compress(
        selected,
        staging.parent().context("dump staging parent is missing")?,
        &entries,
        &partial_archive,
    )?;
    artifacts.publish(&partial_archive)?;
    artifacts.commit()?;

    println!("dumped {} cards to {}", cards.len(), archive_path.display());
    Ok(())
}

fn build_payload(cards: &[DumpCard]) -> Result<DumpPayload> {
    build_payload_with(cards, digest_10, random_hex_10)
}

fn build_payload_with<H, S>(cards: &[DumpCard], mut hash: H, mut salt: S) -> Result<DumpPayload>
where
    H: FnMut(&[u8]) -> String,
    S: FnMut() -> Result<String>,
{
    let mut mapping = BTreeMap::new();
    let mut files = Vec::with_capacity(cards.len());
    let mut filenames = BTreeSet::new();
    let mut ordered = cards.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.address.cmp(&right.address));
    for card in &ordered {
        let mut base = Vec::with_capacity(
            card.address.len() + card.text.len() + card.bibtex.as_ref().map_or(0, String::len) + 2,
        );
        base.extend_from_slice(card.address.as_bytes());
        base.push(0);
        if let Some(bibtex) = &card.bibtex {
            base.extend_from_slice(bibtex.as_bytes());
            base.push(0);
        }
        base.extend_from_slice(card.text.as_bytes());
        let mut digest = hash(&base);
        while filenames.contains(&format!("{digest}.md")) {
            let mut salted = base.clone();
            salted.extend_from_slice(salt()?.as_bytes());
            salted.extend_from_slice(salt()?.as_bytes());
            digest = hash(&salted);
        }
        let filename = format!("{digest}.md");
        filenames.insert(filename.clone());
        mapping.insert(card.address.clone(), filename.clone());
        let markdown = match &card.bibtex {
            Some(bibtex) => literature_markdown(&card.address, bibtex, &card.text),
            None => card.text.clone(),
        };
        files.push((filename, markdown));
    }
    for card in ordered {
        if let Some(bibtex) = &card.bibtex {
            files.push((format!("{}.bib", card.address), bibtex.clone()));
        }
    }
    Ok(DumpPayload { mapping, files })
}

fn literature_markdown(citation_key: &str, bibtex: &str, text: &str) -> String {
    let trailing_newlines = bibtex
        .chars()
        .rev()
        .take_while(|ch| *ch == '\n' || *ch == '\r')
        .filter(|ch| *ch == '\n')
        .count();
    let indicator = match trailing_newlines {
        0 => "|-",
        1 => "|",
        _ => "|+",
    };
    let mut output =
        format!("---\nzt_kind: literature\ncitation_key: {citation_key}\nbibtex: {indicator}\n");
    for line in bibtex.split_inclusive('\n') {
        output.push_str("  ");
        output.push_str(line);
    }
    if !bibtex.ends_with('\n') {
        output.push('\n');
    }
    output.push_str("---\n");
    output.push_str(text);
    output
}

fn digest_10(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    let digest = format!("{:x}", hasher.finalize());
    digest[..10].to_string()
}

fn random_hex_10() -> Result<String> {
    let mut bytes = [0_u8; 5];
    getrandom::fill(&mut bytes).context("failed to read OS randomness")?;
    let mut output = String::with_capacity(10);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(output)
}

fn detect_compression_options() -> Vec<Candidate> {
    detect_with(compression_candidates(current_platform()), probe_candidate)
}

fn probe_candidate(candidate: Candidate) -> bool {
    let Ok(temp) = tempfile::tempdir() else {
        return false;
    };
    let folder = temp.path().join("zt-probe");
    if fs::create_dir_all(&folder).is_err()
        || fs::write(folder.join("mapping.json"), b"{}").is_err()
    {
        return false;
    }
    let output = temp
        .path()
        .join(format!("probe{}", candidate.format.extension()));
    let entries = ["zt-probe/mapping.json".to_string()];
    compress(candidate, temp.path(), &entries, &output).is_ok() && output.is_file()
}

fn command_plan(
    candidate: Candidate,
    source_root: &Path,
    entries: &[String],
    archive_path: &Path,
) -> CompressionPlan {
    match candidate.backend {
        Backend::WindowsTar => CompressionPlan::Single(
            CommandSpec::new("tar.exe")
                .args(["-a", "-cf"])
                .arg(archive_path)
                .arg("-C")
                .arg(source_root)
                .args(entries),
        ),
        Backend::UnixZip => CompressionPlan::Single(
            CommandSpec::new("zip")
                .arg("-q")
                .arg(archive_path)
                .args(entries)
                .current_dir(source_root),
        ),
        Backend::UnixTarGzip => CompressionPlan::Pipeline {
            tar: CommandSpec::new("tar")
                .args(["-cf", "-", "-C"])
                .arg(source_root)
                .args(entries),
            compressor: CommandSpec::new("gzip").arg("-c"),
        },
        Backend::UnixTarZstd => CompressionPlan::Pipeline {
            tar: CommandSpec::new("tar")
                .args(["-cf", "-", "-C"])
                .arg(source_root)
                .args(entries),
            compressor: CommandSpec::new("zstd").args(["-q", "-c"]),
        },
        Backend::SevenZip(program) => CompressionPlan::Single(
            CommandSpec::new(program)
                .args(["a", "-t7z", "-bd", "-y"])
                .arg(archive_path)
                .args(entries)
                .current_dir(source_root),
        ),
    }
}

fn compress(
    candidate: Candidate,
    source_root: &Path,
    entries: &[String],
    archive_path: &Path,
) -> Result<()> {
    execute_plan(
        command_plan(candidate, source_root, entries, archive_path),
        archive_path,
        candidate.format,
    )
}

fn command_from_spec(spec: &CommandSpec) -> Command {
    let program = env::var_os("PATH")
        .and_then(|path| {
            env::split_paths(&path)
                .map(|directory| directory.join(spec.program))
                .find(|candidate| candidate.is_file())
        })
        .unwrap_or_else(|| PathBuf::from(spec.program));
    let mut command = Command::new(program);
    command.args(&spec.args);
    if let Some(path) = &spec.current_dir {
        command.current_dir(path);
    }
    command
}

fn execute_plan(
    plan: CompressionPlan,
    archive_path: &Path,
    format: CompressionFormat,
) -> Result<()> {
    let CompressionPlan::Pipeline { tar, compressor } = plan else {
        let CompressionPlan::Single(spec) = plan else {
            unreachable!();
        };
        let status = command_from_spec(&spec)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .with_context(|| format!("failed to run {}", spec.program))?;
        if !status.success() {
            bail!(
                "compression command for {} failed with {status}",
                format.name()
            );
        }
        return Ok(());
    };

    let mut tar_child = command_from_spec(&tar)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to run tar")?;
    let tar_stdout = tar_child
        .stdout
        .take()
        .context("failed to capture tar output")?;
    let output = fs::File::create(archive_path)
        .with_context(|| format!("failed to create {}", archive_path.display()))?;
    let compression = command_from_spec(&compressor)
        .stdin(Stdio::from(tar_stdout))
        .stdout(Stdio::from(output))
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to run {}", compressor.program));
    let tar_status = tar_child.wait().context("failed to wait for tar")?;
    let compression_status = compression?;
    if !tar_status.success() {
        bail!("compression command tar failed with {tar_status}");
    }
    if !compression_status.success() {
        bail!(
            "compression command {} failed with {compression_status}",
            compressor.program
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    #[test]
    fn collision_retry_appends_two_fresh_salts_until_filename_is_unique() {
        let cards = [
            DumpCard::new("0/0".to_string(), "same".to_string()),
            DumpCard::new("0/1".to_string(), "same".to_string()),
        ];
        let digests = RefCell::new(VecDeque::from([
            "aaaaaaaaaa".to_string(),
            "aaaaaaaaaa".to_string(),
            "aaaaaaaaaa".to_string(),
            "bbbbbbbbbb".to_string(),
        ]));
        let salts = RefCell::new(VecDeque::from([
            "0000000000".to_string(),
            "1111111111".to_string(),
            "2222222222".to_string(),
            "3333333333".to_string(),
        ]));
        let hash_inputs = RefCell::new(Vec::new());

        let payload = build_payload_with(
            &cards,
            |input| {
                hash_inputs.borrow_mut().push(input.to_vec());
                digests.borrow_mut().pop_front().unwrap()
            },
            || Ok(salts.borrow_mut().pop_front().unwrap()),
        )
        .expect("collision resolves");

        assert_eq!(payload.mapping["0/0"], "aaaaaaaaaa.md");
        assert_eq!(payload.mapping["0/1"], "bbbbbbbbbb.md");
        assert_eq!(payload.files.len(), 2);
        assert_eq!(
            hash_inputs.borrow().as_slice(),
            [
                b"0/0\0same".as_slice(),
                b"0/1\0same".as_slice(),
                b"0/1\0same00000000001111111111".as_slice(),
                b"0/1\0same22222222223333333333".as_slice(),
            ]
        );
        assert!(
            !serde_json::to_string(&payload.mapping)
                .unwrap()
                .contains("0000000000")
        );
    }

    #[test]
    fn digest_uses_location_nul_and_exact_text() {
        assert_eq!(digest_10(b"0/0\0text"), "141284107b");
    }

    #[test]
    fn collision_salts_are_ten_lowercase_hex_characters() {
        for _ in 0..32 {
            let salt = random_hex_10().expect("OS randomness");
            assert_eq!(salt.len(), 10);
            assert!(salt.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert_eq!(salt, salt.to_ascii_lowercase());
        }
    }

    #[test]
    fn platform_candidates_match_the_locked_compressor_matrix() {
        assert_eq!(
            compression_candidates(Platform::Windows),
            vec![
                Candidate::new(CompressionFormat::Zip, Backend::WindowsTar),
                Candidate::new(CompressionFormat::TarGz, Backend::WindowsTar),
                Candidate::new(CompressionFormat::TarZst, Backend::WindowsTar),
                Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7z.exe"),),
                Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7zz.exe"),),
            ]
        );
        assert_eq!(
            compression_candidates(Platform::Unix),
            vec![
                Candidate::new(CompressionFormat::Zip, Backend::UnixZip),
                Candidate::new(CompressionFormat::TarGz, Backend::UnixTarGzip),
                Candidate::new(CompressionFormat::TarZst, Backend::UnixTarZstd),
                Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7z")),
                Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7zz")),
            ]
        );
    }

    #[test]
    fn detection_offers_only_successful_formats_and_falls_back_to_7zz() {
        let probes = RefCell::new(Vec::new());
        let detected = detect_with(compression_candidates(Platform::Windows), |candidate| {
            probes.borrow_mut().push(candidate);
            matches!(
                candidate,
                Candidate {
                    format: CompressionFormat::TarGz,
                    backend: Backend::WindowsTar,
                } | Candidate {
                    format: CompressionFormat::SevenZip,
                    backend: Backend::SevenZip("7zz.exe"),
                }
            )
        });

        assert_eq!(
            detected,
            vec![
                Candidate::new(CompressionFormat::TarGz, Backend::WindowsTar),
                Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7zz.exe"),),
            ]
        );
        assert!(probes.borrow().contains(&Candidate::new(
            CompressionFormat::SevenZip,
            Backend::SevenZip("7z.exe"),
        )));
    }

    #[test]
    fn command_plans_route_every_backend_with_the_locked_archive_path() {
        let source = Path::new("source");
        let entries = ["folder/mapping.json".to_string()];

        assert_eq!(
            command_plan(
                Candidate::new(CompressionFormat::Zip, Backend::WindowsTar),
                source,
                &entries,
                Path::new("archive.zip"),
            ),
            CompressionPlan::single(
                "tar.exe",
                [
                    "-a",
                    "-cf",
                    "archive.zip",
                    "-C",
                    "source",
                    "folder/mapping.json",
                ],
                None,
            )
        );
        assert_eq!(
            command_plan(
                Candidate::new(CompressionFormat::Zip, Backend::UnixZip),
                source,
                &entries,
                Path::new("archive.zip"),
            ),
            CompressionPlan::single(
                "zip",
                ["-q", "archive.zip", "folder/mapping.json"],
                Some(source),
            )
        );
        assert_eq!(
            command_plan(
                Candidate::new(CompressionFormat::TarGz, Backend::UnixTarGzip),
                source,
                &entries,
                Path::new("archive.tar.gz"),
            ),
            CompressionPlan::pipeline(
                ["-cf", "-", "-C", "source", "folder/mapping.json"],
                "gzip",
                ["-c"],
            )
        );
        assert_eq!(
            command_plan(
                Candidate::new(CompressionFormat::TarZst, Backend::UnixTarZstd),
                source,
                &entries,
                Path::new("archive.tar.zst"),
            ),
            CompressionPlan::pipeline(
                ["-cf", "-", "-C", "source", "folder/mapping.json"],
                "zstd",
                ["-q", "-c"],
            )
        );
        assert_eq!(
            command_plan(
                Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7zz")),
                source,
                &entries,
                Path::new("archive.7z"),
            ),
            CompressionPlan::single(
                "7zz",
                [
                    "a",
                    "-t7z",
                    "-bd",
                    "-y",
                    "archive.7z",
                    "folder/mapping.json",
                ],
                Some(source),
            )
        );
    }

    #[test]
    fn no_successful_probe_reports_all_supported_option_names() {
        let detected = detect_with(compression_candidates(Platform::Unix), |_| false);
        let error = require_detected_options(detected).unwrap_err();
        assert_eq!(
            error.to_string(),
            "no supported compression option detected; supported options: zip, tar.gz, tar.zst, 7z"
        );
    }

    #[test]
    fn artifact_guard_cleans_failures_and_keeps_only_committed_archive() {
        let temp = tempfile::tempdir().unwrap();
        let failed_staging = temp.path().join("failed-staging");
        let failed_partial = failed_staging.join("partial.zip");
        let failed_archive = temp.path().join("failed.zip");
        fs::create_dir_all(&failed_staging).unwrap();
        fs::write(&failed_partial, b"partial").unwrap();
        let mut failed = DumpArtifacts::new(failed_staging.clone(), failed_archive.clone());
        failed.publish(&failed_partial).unwrap();
        drop(failed);
        assert!(!failed_staging.exists());
        assert!(!failed_archive.exists());

        let complete_staging = temp.path().join("complete-staging");
        let complete_partial = complete_staging.join("partial.zip");
        let complete_archive = temp.path().join("complete.zip");
        fs::create_dir_all(&complete_staging).unwrap();
        fs::write(&complete_partial, b"complete").unwrap();
        let mut complete = DumpArtifacts::new(complete_staging.clone(), complete_archive.clone());
        complete.publish(&complete_partial).unwrap();
        complete.commit().unwrap();
        assert!(!complete_staging.exists());
        assert_eq!(fs::read(complete_archive).unwrap(), b"complete");
    }

    #[test]
    fn artifact_guard_never_removes_an_unowned_destination() {
        let temp = tempfile::tempdir().unwrap();
        let staging = temp.path().join("staging");
        let archive = temp.path().join("existing.zip");
        fs::create_dir_all(&staging).unwrap();
        fs::write(&archive, b"someone else's archive").unwrap();

        drop(DumpArtifacts::new(staging.clone(), archive.clone()));

        assert!(!staging.exists());
        assert_eq!(fs::read(archive).unwrap(), b"someone else's archive");
    }
}
#[test]
fn literature_yaml_round_trips_all_trailing_newline_states_and_exact_card_text() {
    let card_text = "# Title\n\nbody\n\n<!-- zt:reverse-links -->\nreverse";
    for (bibtex, indicator) in [
        ("@book{K,title={T}}", "|-"),
        ("@book{K,title={T}}\n", "|"),
        ("@book{K,title={T}}\n\n", "|+"),
    ] {
        let markdown = literature_markdown("K", bibtex, card_text);
        assert!(markdown.contains(&format!("bibtex: {indicator}\n")));
        let closing = markdown.find("\n---\n").expect("closing front matter");
        let yaml = &markdown[4..=closing];
        let metadata: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(yaml).expect("valid YAML front matter");
        assert_eq!(metadata["zt_kind"].as_str(), Some("literature"));
        assert_eq!(metadata["citation_key"].as_str(), Some("K"));
        assert_eq!(metadata["bibtex"].as_str(), Some(bibtex));
        assert_eq!(&markdown[closing + 5..], card_text);
    }
}
