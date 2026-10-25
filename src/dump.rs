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

use crate::hub::HubLevel;
use crate::math::MathForm;
use crate::model::{Emphasis, Facet, Level};
use crate::{CardType, Workspace};

/// The archive manifest: the archive state that is not Card text.
const MANIFEST: &str = "archive.json";

/// The versioned Lock history and integrity evidence of the same captured view (R315, S13-AC04). The
/// dump writes it even when the archive holds no Lock, so a reader never has to guess whether an
/// archive was inspected for Locks.
const LOCKS: &str = "locks.json";

pub(crate) struct DumpCard {
    address: String,
    workspace: Workspace,
    card_type: CardType,
    /// The exact Term name of a Term Card, `None` for no name.
    term_name: Option<String>,
    facet: Option<Facet>,
    emphasis: Option<Emphasis>,
    level: Option<Level>,
    form: Option<MathForm>,
    hub_level: Option<HubLevel>,
    bibtex: Option<String>,
    text: String,
}

struct DumpPayload {
    mapping: BTreeMap<String, String>,
    files: Vec<(String, String)>,
}

impl DumpCard {
    pub(crate) fn new(
        address: String,
        workspace: Workspace,
        card_type: CardType,
        text: String,
    ) -> Self {
        Self {
            address,
            workspace,
            card_type,
            term_name: None,
            facet: None,
            emphasis: None,
            level: None,
            form: None,
            hub_level: None,
            bibtex: None,
            text,
        }
    }

    /// The exact term name of a Term Card.
    pub(crate) fn with_term_name(mut self, term_name: Option<String>) -> Self {
        self.term_name = term_name;
        self
    }

    /// The Model facet and emphasis of a Model part.
    pub(crate) fn with_part(mut self, facet: Option<Facet>, emphasis: Option<Emphasis>) -> Self {
        self.facet = facet;
        self.emphasis = emphasis;
        self
    }

    /// The elaboration level of a Model Card.
    pub(crate) fn with_level(mut self, level: Option<Level>) -> Self {
        self.level = level;
        self
    }

    /// The Math form of a Model Card that states a mathematical unit.
    pub(crate) fn with_form(mut self, form: Option<MathForm>) -> Self {
        self.form = form;
        self
    }

    /// The Hub level of a Hub Card.
    pub(crate) fn with_hub_level(mut self, hub_level: Option<HubLevel>) -> Self {
        self.hub_level = hub_level;
        self
    }

    pub(crate) fn literature(
        address: String,
        workspace: Workspace,
        bibtex: String,
        text: String,
    ) -> Self {
        Self {
            address,
            workspace,
            card_type: CardType::Idea,
            term_name: None,
            facet: None,
            emphasis: None,
            level: None,
            form: None,
            hub_level: None,
            bibtex: Some(bibtex),
            text,
        }
    }
}

/// `archive.json`: everything beyond the Card files that a dump needs to
/// replicate the archive. `workspaces` maps every address in `mapping.json`
/// to its Workspace, `types` maps every Event, Finding, Model, Hub, and Term Card to its Card
/// type (Idea Cards are left out), `term_names` maps every named Term Card and Term part to its
/// exact term name (a Card with no name is omitted, exactly as an Idea Card is omitted from
/// `types`), `facets` maps every Model part to its Model
/// facet, `emphasis` maps every core or aside part to its emphasis, `levels`
/// maps every Model Card with an elaboration level to it, `forms` maps every
/// Model Card that states a mathematical unit to its Math form, `hub_levels` maps
/// every Hub Card to its Hub level, and `next_topic_id`
/// keeps deleted Topic numbers from being reused.
fn manifest(cards: &[DumpCard], next_topic_id: u64) -> Result<String> {
    let workspaces = cards
        .iter()
        .map(|card| (card.address.clone(), card.workspace.id()))
        .collect::<BTreeMap<_, _>>();
    let types = cards
        .iter()
        .filter(|card| card.card_type != CardType::Idea)
        .map(|card| (card.address.clone(), card.card_type.id()))
        .collect::<BTreeMap<_, _>>();
    // The term-name map sits beside the type map: an absent name is omitted, exactly as an Idea
    // Card's type is omitted from `types` (S07 #149; S17 completes dump/load compatibility).
    let term_names = cards
        .iter()
        .filter_map(|card| {
            card.term_name
                .as_ref()
                .map(|term| (card.address.clone(), term.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let facets = cards
        .iter()
        .filter_map(|card| {
            card.facet
                .as_ref()
                .map(|facet| (card.address.clone(), facet.id()))
        })
        .collect::<BTreeMap<_, _>>();
    let emphasis = cards
        .iter()
        .filter_map(|card| {
            card.emphasis
                .map(|emphasis| (card.address.clone(), emphasis.id()))
        })
        .collect::<BTreeMap<_, _>>();
    let levels = cards
        .iter()
        .filter_map(|card| card.level.map(|level| (card.address.clone(), level.id())))
        .collect::<BTreeMap<_, _>>();
    let forms = cards
        .iter()
        .filter_map(|card| card.form.map(|form| (card.address.clone(), form.id())))
        .collect::<BTreeMap<_, _>>();
    let hub_levels = cards
        .iter()
        .filter_map(|card| {
            card.hub_level
                .map(|level| (card.address.clone(), level.id()))
        })
        .collect::<BTreeMap<_, _>>();
    // serde_json sorts object keys, so the documented key order is written out.
    Ok(format!(
        r#"{{"next_topic_id":{next_topic_id},"workspaces":{},"types":{},"term_names":{},"facets":{},"emphasis":{},"levels":{},"forms":{},"hub_levels":{}}}"#,
        serde_json::to_string(&workspaces)?,
        serde_json::to_string(&types)?,
        serde_json::to_string(&term_names)?,
        serde_json::to_string(&facets)?,
        serde_json::to_string(&emphasis)?,
        serde_json::to_string(&levels)?,
        serde_json::to_string(&forms)?,
        serde_json::to_string(&hub_levels)?
    ))
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

/// One way to write one format. `program` pins the executable that detection proved
/// can write the format; `None` leaves the backend's command name to the system.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Candidate {
    format: CompressionFormat,
    backend: Backend,
    program: Option<PathBuf>,
}

#[derive(Debug, Eq, PartialEq)]
struct CommandSpec {
    program: PathBuf,
    args: Vec<OsString>,
    current_dir: Option<PathBuf>,
}

impl CommandSpec {
    fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
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
        Self {
            format,
            backend,
            program: None,
        }
    }

    fn pinned(&self, program: PathBuf) -> Self {
        Self {
            program: Some(program),
            ..self.clone()
        }
    }

    fn name(&self, default: &'static str) -> PathBuf {
        self.program
            .clone()
            .unwrap_or_else(|| PathBuf::from(default))
    }
}

impl Backend {
    /// The single executable that writes the archive, if the backend is one command.
    /// A host may hold several executables of that name (Git for Windows puts GNU tar
    /// ahead of Windows' bsdtar on PATH), so detection tries each of them.
    const fn program(self) -> Option<&'static str> {
        match self {
            Self::WindowsTar => Some("tar.exe"),
            Self::UnixZip => Some("zip"),
            Self::SevenZip(program) => Some(program),
            Self::UnixTarGzip | Self::UnixTarZstd => None,
        }
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

/// Replace each single-command candidate by one candidate per executable of that name,
/// in search order, so a later executable can stand in when an earlier one cannot write
/// the format. A command found nowhere is left to the system to resolve.
fn expand_with<L>(candidates: Vec<Candidate>, mut locate: L) -> Vec<Candidate>
where
    L: FnMut(&str) -> Vec<PathBuf>,
{
    let mut expanded = Vec::new();
    for candidate in candidates {
        let found = candidate
            .backend
            .program()
            .map(&mut locate)
            .unwrap_or_default();
        if found.is_empty() {
            expanded.push(candidate);
        } else {
            expanded.extend(found.into_iter().map(|program| candidate.pinned(program)));
        }
    }
    expanded
}

/// Every executable named `program` on PATH, in PATH order, without duplicates.
fn locate_on_path(program: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    let directories = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    for directory in directories {
        let candidate = directory.join(program);
        if !candidate.is_file() {
            continue;
        }
        let key = fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
        if seen.insert(key) {
            found.push(candidate);
        }
    }
    found
}

fn detect_with<F>(candidates: Vec<Candidate>, mut probe: F) -> Vec<Candidate>
where
    F: FnMut(&Candidate) -> bool,
{
    let mut detected = Vec::new();
    for candidate in candidates {
        if detected
            .iter()
            .any(|item: &Candidate| item.format == candidate.format)
        {
            continue;
        }
        if probe(&candidate) {
            detected.push(candidate);
        }
    }
    detected
}

/// Whether the file at `path` starts with the signature of `format`. A tool that accepts
/// the command but writes another format (GNU tar writes a plain tar named `.zip`) fails.
fn written_as(path: &Path, format: CompressionFormat) -> bool {
    let mut head = Vec::with_capacity(6);
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    if io::Read::read_to_end(&mut io::Read::take(file, 6), &mut head).is_err() {
        return false;
    }
    match format {
        CompressionFormat::Zip => {
            head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06")
        }
        CompressionFormat::TarGz => head.starts_with(&[0x1f, 0x8b]),
        CompressionFormat::TarZst => head.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]),
        CompressionFormat::SevenZip => head.starts_with(&[b'7', b'z', 0xbc, 0xaf, 0x27, 0x1c]),
    }
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

pub(crate) fn run(
    root: &Path,
    cards: &[DumpCard],
    next_topic_id: u64,
    locks: &serde_json::Value,
) -> Result<()> {
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
        .map(|(_, option)| option.clone())
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
    fs::write(staging.join(MANIFEST), manifest(cards, next_topic_id)?)?;
    // The Lock history and the integrity evidence of the same captured view travel with the Cards,
    // immediately after the manifest, so a reader can inspect every Lock promise after old roots
    // disappear (R315/R316, S13-AC02/S13-AC04).
    fs::write(staging.join(LOCKS), serde_json::to_vec(locks)?)?;
    for (filename, text) in &payload.files {
        fs::write(staging.join(filename), text.as_bytes())?;
    }
    let mut entries = vec![
        format!("{archive_name}/mapping.json"),
        format!("{archive_name}/{MANIFEST}"),
        format!("{archive_name}/{LOCKS}"),
    ];
    entries.extend(
        payload
            .files
            .iter()
            .map(|(filename, _)| format!("{archive_name}/{filename}")),
    );
    // The figures the Cards show travel with them, under their content names.
    let shown = cards
        .iter()
        .flat_map(|card| crate::artifact::references(&card.text))
        .collect::<BTreeSet<_>>();
    for digest in &shown {
        let Some(source) = crate::artifact::path(root, digest) else {
            continue;
        };
        let name = source
            .file_name()
            .and_then(|name| name.to_str())
            .context("artifact file name is not UTF-8")?
            .to_string();
        let folder = staging.join("artifacts");
        fs::create_dir_all(&folder)?;
        fs::copy(&source, folder.join(&name))
            .with_context(|| format!("failed to copy {}", source.display()))?;
        entries.push(format!("{archive_name}/artifacts/{name}"));
    }
    let partial_archive = staging.join(format!(
        ".{archive_name}.partial{}",
        selected.format.extension()
    ));
    compress(
        &selected,
        staging.parent().context("dump staging parent is missing")?,
        &entries,
        &partial_archive,
    )?;
    if !written_as(&partial_archive, selected.format) {
        bail!(
            "compression command for {} did not write a {} archive",
            selected.format.name(),
            selected.format.name()
        );
    }
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
    detect_with(
        expand_with(compression_candidates(current_platform()), locate_on_path),
        probe_candidate,
    )
}

/// A candidate is usable only if it really writes its format: it compresses a small probe
/// folder, and the result must carry the format's signature.
fn probe_candidate(candidate: &Candidate) -> bool {
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
    compress(candidate, temp.path(), &entries, &output).is_ok()
        && written_as(&output, candidate.format)
}

fn command_plan(
    candidate: &Candidate,
    source_root: &Path,
    entries: &[String],
    archive_path: &Path,
) -> CompressionPlan {
    match candidate.backend {
        Backend::WindowsTar => {
            // The archive is named from inside its own folder: a GNU tar on PATH reads
            // `C:\...` as a remote host and stalls before it fails.
            let folder = archive_path
                .parent()
                .filter(|folder| !folder.as_os_str().is_empty());
            let mut command = CommandSpec::new(candidate.name("tar.exe"))
                .args(["-a", "-cf"])
                .arg(match folder {
                    Some(_) => archive_path
                        .file_name()
                        .map_or_else(|| archive_path.as_os_str().to_owned(), ToOwned::to_owned),
                    None => archive_path.as_os_str().to_owned(),
                })
                .arg("-C")
                .arg(source_root)
                .args(entries);
            if let Some(folder) = folder {
                command = command.current_dir(folder);
            }
            CompressionPlan::Single(command)
        }
        Backend::UnixZip => CompressionPlan::Single(
            CommandSpec::new(candidate.name("zip"))
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
            CommandSpec::new(candidate.name(program))
                .args(["a", "-t7z", "-bd", "-y"])
                .arg(archive_path)
                .args(entries)
                .current_dir(source_root),
        ),
    }
}

fn compress(
    candidate: &Candidate,
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
    // A pinned executable runs as is; a bare name runs as the first match on PATH.
    let program = if spec.program.components().count() > 1 {
        spec.program.clone()
    } else {
        locate_on_path(&spec.program.to_string_lossy())
            .into_iter()
            .next()
            .unwrap_or_else(|| spec.program.clone())
    };
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
            .with_context(|| format!("failed to run {}", spec.program.display()))?;
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
        .with_context(|| format!("failed to run {}", compressor.program.display()));
    let tar_status = tar_child.wait().context("failed to wait for tar")?;
    let compression_status = compression?;
    if !tar_status.success() {
        bail!("compression command tar failed with {tar_status}");
    }
    if !compression_status.success() {
        bail!(
            "compression command {} failed with {compression_status}",
            compressor.program.display()
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
            DumpCard::new(
                "0/0".to_string(),
                Workspace::Human,
                CardType::Idea,
                "same".to_string(),
            ),
            DumpCard::new(
                "0/1".to_string(),
                Workspace::Agent,
                CardType::Idea,
                "same".to_string(),
            ),
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
    fn manifest_records_every_workspace_the_non_idea_types_the_facets_and_the_next_topic_id() {
        let cards = [
            DumpCard::new(
                "0/0".to_string(),
                Workspace::Human,
                CardType::Idea,
                "a".to_string(),
            ),
            DumpCard::new(
                "0/1".to_string(),
                Workspace::Human,
                CardType::Model,
                "m".to_string(),
            )
            .with_level(Some(Level::Formal))
            .with_form(Some(MathForm::Theorem)),
            DumpCard::new(
                "0/1|a".to_string(),
                Workspace::Human,
                CardType::Model,
                "p".to_string(),
            )
            .with_part(
                Some(Facet::parse("assumptions").unwrap()),
                Some(Emphasis::Core),
            ),
            DumpCard::new(
                "1/0".to_string(),
                Workspace::Agent,
                CardType::Idea,
                "b".to_string(),
            ),
            DumpCard::new(
                "1/1|a".to_string(),
                Workspace::Agent,
                CardType::Event,
                "e".to_string(),
            ),
            DumpCard::new(
                "1/1|b".to_string(),
                Workspace::Agent,
                CardType::Finding,
                "f".to_string(),
            ),
            DumpCard::new(
                "1/1|c".to_string(),
                Workspace::Agent,
                CardType::Hub,
                "h".to_string(),
            )
            .with_hub_level(Some(HubLevel::Topic)),
            DumpCard::new(
                "1/1|d".to_string(),
                Workspace::Agent,
                CardType::Term,
                "t".to_string(),
            )
            .with_term_name(Some("均衡".to_string())),
            DumpCard::literature(
                "Smith2024".to_string(),
                Workspace::Agent,
                "@book{Smith2024,title={T}}".to_string(),
                "c".to_string(),
            ),
        ];
        assert_eq!(
            manifest(&cards, 7).unwrap(),
            r#"{"next_topic_id":7,"workspaces":{"0/0":"human","0/1":"human","0/1|a":"human","1/0":"agent","1/1|a":"agent","1/1|b":"agent","1/1|c":"agent","1/1|d":"agent","Smith2024":"agent"},"types":{"0/1":"model","0/1|a":"model","1/1|a":"event","1/1|b":"finding","1/1|c":"hub","1/1|d":"term"},"term_names":{"1/1|d":"均衡"},"facets":{"0/1|a":"assumptions"},"emphasis":{"0/1|a":"core"},"levels":{"0/1":"formal"},"forms":{"0/1":"theorem"},"hub_levels":{"1/1|c":"topic"}}"#
        );
        assert_eq!(
            manifest(&cards[..1], 1).unwrap(),
            r#"{"next_topic_id":1,"workspaces":{"0/0":"human"},"types":{},"term_names":{},"facets":{},"emphasis":{},"levels":{},"forms":{},"hub_levels":{}}"#
        );
        assert_eq!(
            manifest(&[], 0).unwrap(),
            r#"{"next_topic_id":0,"workspaces":{},"types":{},"term_names":{},"facets":{},"emphasis":{},"levels":{},"forms":{},"hub_levels":{}}"#
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
            probes.borrow_mut().push(candidate.clone());
            matches!(
                (candidate.format, candidate.backend),
                (CompressionFormat::TarGz, Backend::WindowsTar)
                    | (CompressionFormat::SevenZip, Backend::SevenZip("7zz.exe"))
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
    fn detection_tries_every_executable_of_a_name_and_pins_the_first_that_writes_the_format() {
        let gnu = PathBuf::from(r"C:\Git\usr\bin\tar.exe");
        let bsd = PathBuf::from(r"C:\Windows\System32\tar.exe");
        let tar = |format| Candidate::new(format, Backend::WindowsTar);
        let expanded = expand_with(compression_candidates(Platform::Windows), |program| {
            if program == "tar.exe" {
                vec![gnu.clone(), bsd.clone()]
            } else {
                Vec::new()
            }
        });
        assert_eq!(
            expanded,
            vec![
                tar(CompressionFormat::Zip).pinned(gnu.clone()),
                tar(CompressionFormat::Zip).pinned(bsd.clone()),
                tar(CompressionFormat::TarGz).pinned(gnu.clone()),
                tar(CompressionFormat::TarGz).pinned(bsd.clone()),
                tar(CompressionFormat::TarZst).pinned(gnu.clone()),
                tar(CompressionFormat::TarZst).pinned(bsd.clone()),
                Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7z.exe")),
                Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7zz.exe")),
            ]
        );
        // GNU tar cannot write the archive formats; bsdtar can, so bsdtar is pinned.
        let detected = detect_with(expanded, |candidate| {
            candidate.program.as_deref() == Some(bsd.as_path())
        });
        assert_eq!(
            detected,
            vec![
                tar(CompressionFormat::Zip).pinned(bsd.clone()),
                tar(CompressionFormat::TarGz).pinned(bsd.clone()),
                tar(CompressionFormat::TarZst).pinned(bsd.clone()),
            ]
        );
        assert_eq!(
            command_plan(
                &detected[0],
                Path::new("source"),
                &["folder/mapping.json".to_string()],
                Path::new("archive.zip"),
            ),
            CompressionPlan::Single(CommandSpec::new(bsd).args([
                "-a",
                "-cf",
                "archive.zip",
                "-C",
                "source",
                "folder/mapping.json",
            ]))
        );
    }

    #[test]
    fn windows_tar_names_the_archive_from_inside_its_folder() {
        let folder = Path::new(r"C:\archive\dump\.tmp\zt-archive-1");
        assert_eq!(
            command_plan(
                &Candidate::new(CompressionFormat::Zip, Backend::WindowsTar),
                Path::new(r"C:\archive\dump\.tmp"),
                &["zt-archive-1/mapping.json".to_string()],
                &folder.join(".zt-archive-1.partial.zip"),
            ),
            CompressionPlan::single(
                "tar.exe",
                [
                    "-a",
                    "-cf",
                    ".zt-archive-1.partial.zip",
                    "-C",
                    r"C:\archive\dump\.tmp",
                    "zt-archive-1/mapping.json",
                ],
                Some(folder),
            )
        );
    }

    #[test]
    fn only_a_file_with_the_format_signature_counts_as_written() {
        let temp = tempfile::tempdir().unwrap();
        let cases: [(CompressionFormat, &[u8]); 5] = [
            (CompressionFormat::Zip, b"PK\x03\x04rest"),
            (CompressionFormat::Zip, b"PK\x05\x06"),
            (CompressionFormat::TarGz, &[0x1f, 0x8b, 8]),
            (CompressionFormat::TarZst, &[0x28, 0xb5, 0x2f, 0xfd, 0]),
            (
                CompressionFormat::SevenZip,
                &[b'7', b'z', 0xbc, 0xaf, 0x27, 0x1c, 0],
            ),
        ];
        for (index, (format, bytes)) in cases.into_iter().enumerate() {
            let path = temp.path().join(format!("good{index}"));
            fs::write(&path, bytes).unwrap();
            assert!(written_as(&path, format), "{format:?}");
        }
        // A plain tar archive named .zip, as GNU tar writes for `-a -cf x.zip`.
        let tar = temp.path().join("tar.zip");
        let mut header = vec![0_u8; 512];
        header[257..262].copy_from_slice(b"ustar");
        fs::write(&tar, header).unwrap();
        assert!(!written_as(&tar, CompressionFormat::Zip));
        assert!(!written_as(
            &temp.path().join("missing.zip"),
            CompressionFormat::Zip
        ));
        let empty = temp.path().join("empty.7z");
        fs::write(&empty, b"").unwrap();
        assert!(!written_as(&empty, CompressionFormat::SevenZip));
    }

    #[test]
    fn command_plans_route_every_backend_with_the_locked_archive_path() {
        let source = Path::new("source");
        let entries = ["folder/mapping.json".to_string()];

        assert_eq!(
            command_plan(
                &Candidate::new(CompressionFormat::Zip, Backend::WindowsTar),
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
                &Candidate::new(CompressionFormat::Zip, Backend::UnixZip),
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
                &Candidate::new(CompressionFormat::TarGz, Backend::UnixTarGzip),
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
                &Candidate::new(CompressionFormat::TarZst, Backend::UnixTarZstd),
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
                &Candidate::new(CompressionFormat::SevenZip, Backend::SevenZip("7zz")),
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

    /// R314, S13-AC01: the dump module is the serializer/compressor. It receives the lock export with
    /// its Cards and next Topic id and writes the entry; it never opens a connection of its own to
    /// discover Locks later, and it owns the entry name and its place after the manifest.
    #[test]
    fn the_dump_module_serializes_its_input_and_opens_no_database_of_its_own() {
        let production = include_str!("dump.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("dump.rs holds its unit tests after the module code");
        for forbidden in ["rusqlite", "Connection", "open_db", "tree_lock"] {
            assert!(
                !production.contains(forbidden),
                "the dump module must not reach the database through `{forbidden}`: it receives the \
                 lock export from the caller's captured view"
            );
        }
        assert_eq!(LOCKS, "locks.json");
    }
}
