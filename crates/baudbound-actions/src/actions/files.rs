use std::{
    fs,
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
};

use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeContext,
};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions as CapabilityOpenOptions},
};
use serde_json::{Map, Number, Value};

use crate::actions::{bounded_io, network::send_download_request};
use crate::{
    ActionSecurityPolicy, config_bool, config_string, failed, required_string, timeout_duration,
};

mod move_file;

use move_file::move_file;

pub(crate) fn read_file_action(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
    max_read_bytes: u64,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let path = required_string(request, "path")?;
    let encoding = config_string(&request.config, "encoding").unwrap_or_else(|| "utf-8".to_owned());
    if encoding != "utf-8" {
        return failed(request, format!("unsupported file encoding {encoding}"));
    }

    let path_ref = resolve_action_path(request, context, &path, PathIntent::Existing)?;
    let metadata = path_ref
        .metadata()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to inspect {path}: {source}"),
        })?;
    if !metadata.is_file {
        return failed(request, format!("{path} is not a regular file"));
    }
    if metadata.len > max_read_bytes {
        return failed(
            request,
            format!("file exceeds the configured read limit of {max_read_bytes} bytes"),
        );
    }
    let mut file = path_ref
        .open_read()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to read {path}: {source}"),
        })?;
    let bytes = bounded_io::read_to_end(&mut file, max_read_bytes).map_err(|source| {
        RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to read {path}: {source}"),
        }
    })?;
    let content =
        String::from_utf8(bytes.clone()).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("{path} is not valid UTF-8: {source}"),
        })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("path".to_owned(), Value::String(path)),
            ("content".to_owned(), Value::String(content)),
            ("bytes".to_owned(), Value::Number(Number::from(bytes.len()))),
        ]),
    })
}
pub(crate) fn download_file_action(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
    max_download_bytes: u64,
    policy: &ActionSecurityPolicy,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let url = required_string(request, "url")?;
    let destination_path = required_string(request, "destinationPath")?;
    let overwrite = config_bool(&request.config, "overwrite", false);
    let destination =
        resolve_action_path(request, context, &destination_path, PathIntent::Destination)?;
    ensure_destination_available(request, &destination, overwrite)?;
    ensure_parent_directory(request, &destination)?;

    let mut response = send_download_request(request, &url, timeout_duration(request)?, policy)?;
    let status = response.status();
    if !status.is_success() {
        return failed(
            request,
            format!("download request {url} returned {}", status.as_u16()),
        );
    }
    if response
        .content_length()
        .is_some_and(|length| length > max_download_bytes)
    {
        return failed(
            request,
            format!("download exceeds the configured limit of {max_download_bytes} bytes"),
        );
    }

    let (temporary, mut temporary_file) =
        create_temporary_sibling(&destination).map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to create temporary download file: {source}"),
        })?;
    let download_result = (|| {
        let bytes = bounded_io::copy(&mut response, &mut temporary_file, max_download_bytes)
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed to download {url}: {source}"),
            })?;
        temporary_file
            .sync_all()
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed to flush temporary download file: {source}"),
            })?;
        drop(temporary_file);
        temporary
            .move_to(&destination, overwrite)
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!(
                    "failed to replace download destination {destination_path}: {source}"
                ),
            })?;
        Ok(bytes)
    })();
    if download_result.is_err() {
        let _ = temporary.remove_file();
    }
    let bytes = download_result?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("path".to_owned(), Value::String(destination_path)),
            ("url".to_owned(), Value::String(url)),
            ("bytes".to_owned(), Value::Number(Number::from(bytes))),
        ]),
    })
}

pub(crate) fn write_file_action(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let path = required_string(request, "path")?;
    let content = config_string(&request.config, "content").unwrap_or_default();
    let mode = config_string(&request.config, "mode").unwrap_or_else(|| "overwrite".to_owned());
    let path_buf = resolve_action_path(request, context, &path, PathIntent::Destination)?;
    ensure_parent_directory(request, &path_buf)?;

    let write_mode = match mode.as_str() {
        "append" => WriteMode::Append,
        "overwrite" => WriteMode::Overwrite,
        other => return failed(request, format!("unsupported file write mode {other}")),
    };
    let mut file =
        path_buf
            .open_write(write_mode, false)
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed to open {path} for writing: {source}"),
            })?;
    file.write_all(content.as_bytes())
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to write {path}: {source}"),
        })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("path".to_owned(), Value::String(path)),
            ("mode".to_owned(), Value::String(mode)),
            (
                "bytes".to_owned(),
                Value::Number(Number::from(content.len())),
            ),
        ]),
    })
}

pub(crate) fn copy_file_action(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let source_path = required_string(request, "sourcePath")?;
    let destination_path = required_string(request, "destinationPath")?;
    let overwrite = config_bool(&request.config, "overwrite", false);
    let source = resolve_action_path(request, context, &source_path, PathIntent::Existing)?;
    let destination =
        resolve_action_path(request, context, &destination_path, PathIntent::Destination)?;

    ensure_regular_source(request, &source)?;
    ensure_distinct_paths(request, &source, &destination)?;
    ensure_destination_available(request, &destination, overwrite)?;
    ensure_parent_directory(request, &destination)?;

    let bytes =
        source
            .copy_to(&destination, overwrite)
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!("failed to copy {source_path} to {destination_path}: {source}"),
            })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("source_path".to_owned(), Value::String(source_path)),
            (
                "destination_path".to_owned(),
                Value::String(destination_path),
            ),
            ("bytes".to_owned(), Value::Number(Number::from(bytes))),
        ]),
    })
}

pub(crate) fn move_file_action(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let source_path = required_string(request, "sourcePath")?;
    let destination_path = required_string(request, "destinationPath")?;
    let overwrite = config_bool(&request.config, "overwrite", false);
    let source = resolve_action_path(request, context, &source_path, PathIntent::Existing)?;
    let destination =
        resolve_action_path(request, context, &destination_path, PathIntent::Destination)?;

    ensure_regular_source(request, &source)?;
    ensure_distinct_paths(request, &source, &destination)?;
    ensure_destination_available(request, &destination, overwrite)?;
    ensure_parent_directory(request, &destination)?;
    source
        .move_to(&destination, overwrite)
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to move {source_path} to {destination_path}: {source}"),
        })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("source_path".to_owned(), Value::String(source_path)),
            (
                "destination_path".to_owned(),
                Value::String(destination_path),
            ),
        ]),
    })
}

pub(crate) fn delete_file_action(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let path = required_string(request, "path")?;
    let path_buf = resolve_action_path(request, context, &path, PathIntent::Existing)?;
    let metadata = path_buf
        .metadata()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to inspect {path}: {source}"),
        })?;
    if !metadata.is_file {
        return failed(request, format!("{path} is not a regular file"));
    }

    path_buf
        .remove_file()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!("failed to delete {path}: {source}"),
        })?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([("path".to_owned(), Value::String(path))]),
    })
}

#[derive(Clone, Copy)]
enum PathIntent {
    Destination,
    Existing,
}

enum ActionPath {
    Ambient(PathBuf),
    Limited {
        directory: Dir,
        relative: PathBuf,
        display: PathBuf,
    },
}

struct ActionMetadata {
    is_file: bool,
    len: u64,
}

enum ActionFile {
    Ambient(fs::File),
    Limited(cap_std::fs::File),
}

#[derive(Clone, Copy)]
enum WriteMode {
    Append,
    Overwrite,
}

fn resolve_action_path(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
    configured_path: &str,
    intent: PathIntent,
) -> Result<ActionPath, RuntimeActionError> {
    let path = Path::new(configured_path);
    if path.is_absolute() || contains_parent_component(path) {
        return Ok(ActionPath::Ambient(path.to_path_buf()));
    }
    let Some(workspace) = script_workspace(context) else {
        return Ok(ActionPath::Ambient(path.to_path_buf()));
    };
    fs::create_dir_all(&workspace).map_err(|source| RuntimeActionError::Failed {
        action_type: request.action_type.clone(),
        message: format!(
            "failed to create script workspace {}: {source}",
            workspace.display()
        ),
    })?;
    let directory = Dir::open_ambient_dir(&workspace, ambient_authority()).map_err(|source| {
        RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!(
                "failed to open script workspace {}: {source}",
                workspace.display()
            ),
        }
    })?;
    if matches!(intent, PathIntent::Existing) {
        directory
            .metadata(path)
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!(
                    "failed to resolve limited file path {}: {source}",
                    workspace.join(path).display()
                ),
            })?;
    }
    Ok(ActionPath::Limited {
        directory,
        relative: path.to_path_buf(),
        display: workspace.join(path),
    })
}

fn contains_parent_component(path: &Path) -> bool {
    path.components()
        .any(|component| component == Component::ParentDir)
        || path
            .to_string_lossy()
            .replace('\\', "/")
            .split('/')
            .any(|component| component == "..")
}

fn script_workspace(context: &RuntimeContext) -> Option<PathBuf> {
    let package_path = context.package_path.as_ref()?;
    let scripts_directory = package_path.parent()?;
    let runner_home = scripts_directory.parent()?;
    Some(
        runner_home
            .join("workspaces")
            .join(&context.identity.script_id),
    )
}

fn ensure_destination_available(
    request: &RuntimeActionRequest,
    destination: &ActionPath,
    overwrite: bool,
) -> Result<(), RuntimeActionError> {
    let exists = destination
        .try_exists()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!(
                "failed to inspect destination {}: {source}",
                destination.display().display()
            ),
        })?;
    if exists && !overwrite {
        return failed(
            request,
            format!(
                "destination {} already exists and overwrite is disabled",
                destination.display().display()
            ),
        );
    }
    if exists
        && !destination
            .metadata()
            .map_err(|source| RuntimeActionError::Failed {
                action_type: request.action_type.clone(),
                message: format!(
                    "failed to inspect destination {}: {source}",
                    destination.display().display()
                ),
            })?
            .is_file
    {
        return failed(
            request,
            format!(
                "destination {} is not a regular file",
                destination.display().display()
            ),
        );
    }
    Ok(())
}

fn ensure_regular_source(
    request: &RuntimeActionRequest,
    source: &ActionPath,
) -> Result<(), RuntimeActionError> {
    let metadata = source
        .metadata()
        .map_err(|source_error| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!(
                "failed to inspect source {}: {source_error}",
                source.display().display()
            ),
        })?;
    if !metadata.is_file {
        return failed(
            request,
            format!(
                "source {} is not a regular file",
                source.display().display()
            ),
        );
    }
    Ok(())
}

fn ensure_distinct_paths(
    request: &RuntimeActionRequest,
    source: &ActionPath,
    destination: &ActionPath,
) -> Result<(), RuntimeActionError> {
    if !source.try_exists().unwrap_or(false) || !destination.try_exists().unwrap_or(false) {
        return Ok(());
    }

    let source_path = source
        .canonicalize_for_comparison()
        .map_err(|source_error| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!(
                "failed to resolve source path {}: {source_error}",
                source.display().display()
            ),
        })?;
    let destination_path = destination
        .canonicalize_for_comparison()
        .map_err(|source_error| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!(
                "failed to resolve destination path {}: {source_error}",
                destination.display().display()
            ),
        })?;
    if source_path == destination_path {
        return failed(
            request,
            format!(
                "source and destination resolve to the same file: {}",
                source.display().display()
            ),
        );
    }
    Ok(())
}

fn ensure_parent_directory(
    request: &RuntimeActionRequest,
    destination: &ActionPath,
) -> Result<(), RuntimeActionError> {
    destination
        .create_parent_dir_all()
        .map_err(|source| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: format!(
                "failed to create parent directory for {}: {source}",
                destination.display().display()
            ),
        })
}

impl ActionPath {
    fn display(&self) -> &Path {
        match self {
            Self::Ambient(path) => path,
            Self::Limited { display, .. } => display,
        }
    }

    fn metadata(&self) -> io::Result<ActionMetadata> {
        match self {
            Self::Ambient(path) => {
                let metadata = fs::metadata(path)?;
                Ok(ActionMetadata {
                    is_file: metadata.is_file(),
                    len: metadata.len(),
                })
            }
            Self::Limited {
                directory,
                relative,
                ..
            } => {
                let metadata = directory.metadata(relative)?;
                Ok(ActionMetadata {
                    is_file: metadata.is_file(),
                    len: metadata.len(),
                })
            }
        }
    }

    fn try_exists(&self) -> io::Result<bool> {
        match self {
            Self::Ambient(path) => path.try_exists(),
            Self::Limited {
                directory,
                relative,
                ..
            } => directory.try_exists(relative),
        }
    }

    fn open_read(&self) -> io::Result<ActionFile> {
        match self {
            Self::Ambient(path) => fs::File::open(path).map(ActionFile::Ambient),
            Self::Limited {
                directory,
                relative,
                ..
            } => directory.open(relative).map(ActionFile::Limited),
        }
    }

    fn open_write(&self, mode: WriteMode, create_new: bool) -> io::Result<ActionFile> {
        match self {
            Self::Ambient(path) => {
                let mut options = fs::OpenOptions::new();
                configure_write_options(&mut options, mode, create_new);
                options.open(path).map(ActionFile::Ambient)
            }
            Self::Limited {
                directory,
                relative,
                ..
            } => {
                let mut options = CapabilityOpenOptions::new();
                configure_write_options(&mut options, mode, create_new);
                directory
                    .open_with(relative, &options)
                    .map(ActionFile::Limited)
            }
        }
    }

    fn create_parent_dir_all(&self) -> io::Result<()> {
        let Some(parent) = self
            .relative_path()
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        else {
            return Ok(());
        };
        match self {
            Self::Ambient(_) => fs::create_dir_all(parent),
            Self::Limited { directory, .. } => directory.create_dir_all(parent),
        }
    }

    fn copy_to(&self, destination: &Self, overwrite: bool) -> io::Result<u64> {
        if !overwrite {
            let mut source = self.open_read()?;
            let mut destination = destination.open_write(WriteMode::Overwrite, true)?;
            return io::copy(&mut source, &mut destination);
        }
        match (self, destination) {
            (Self::Ambient(source), Self::Ambient(destination)) => fs::copy(source, destination),
            (
                Self::Limited {
                    directory: source_directory,
                    relative: source,
                    ..
                },
                Self::Limited {
                    directory: destination_directory,
                    relative: destination,
                    ..
                },
            ) => source_directory.copy(source, destination_directory, destination),
            _ => {
                let mut source = self.open_read()?;
                let mut destination = destination.open_write(WriteMode::Overwrite, false)?;
                io::copy(&mut source, &mut destination)
            }
        }
    }

    fn move_to(&self, destination: &Self, overwrite: bool) -> io::Result<()> {
        if !overwrite {
            self.copy_to(destination, false)?;
            return self.remove_file();
        }
        match (self, destination) {
            (Self::Ambient(source), Self::Ambient(destination)) => {
                move_file(source, destination, overwrite)
            }
            (
                Self::Limited {
                    directory: source_directory,
                    relative: source,
                    ..
                },
                Self::Limited {
                    directory: destination_directory,
                    relative: destination,
                    ..
                },
            ) => source_directory.rename(source, destination_directory, destination),
            _ => {
                self.copy_to(destination, true)?;
                self.remove_file()
            }
        }
    }

    fn remove_file(&self) -> io::Result<()> {
        match self {
            Self::Ambient(path) => fs::remove_file(path),
            Self::Limited {
                directory,
                relative,
                ..
            } => directory.remove_file(relative),
        }
    }

    fn canonicalize_for_comparison(&self) -> io::Result<PathBuf> {
        match self {
            Self::Ambient(path) => path.canonicalize(),
            Self::Limited {
                directory,
                relative,
                ..
            } => directory.canonicalize(relative),
        }
    }

    fn relative_path(&self) -> &Path {
        match self {
            Self::Ambient(path) => path,
            Self::Limited { relative, .. } => relative,
        }
    }

    fn sibling(&self, file_name: &str) -> io::Result<Self> {
        match self {
            Self::Ambient(path) => Ok(Self::Ambient(
                path.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(file_name),
            )),
            Self::Limited {
                directory,
                relative,
                display,
            } => {
                let relative = relative
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .join(file_name);
                let display = display
                    .parent()
                    .unwrap_or_else(|| Path::new(""))
                    .join(file_name);
                Ok(Self::Limited {
                    directory: directory.try_clone()?,
                    relative,
                    display,
                })
            }
        }
    }
}

fn configure_write_options<T: WriteOptions>(options: &mut T, mode: WriteMode, create_new: bool) {
    options.write(true);
    options.create(!create_new);
    options.create_new(create_new);
    match mode {
        WriteMode::Append => options.append(true),
        WriteMode::Overwrite => options.truncate(true),
    };
}

trait WriteOptions {
    fn write(&mut self, enabled: bool);
    fn append(&mut self, enabled: bool);
    fn truncate(&mut self, enabled: bool);
    fn create(&mut self, enabled: bool);
    fn create_new(&mut self, enabled: bool);
}

impl WriteOptions for fs::OpenOptions {
    fn write(&mut self, enabled: bool) {
        self.write(enabled);
    }

    fn append(&mut self, enabled: bool) {
        self.append(enabled);
    }

    fn truncate(&mut self, enabled: bool) {
        self.truncate(enabled);
    }

    fn create(&mut self, enabled: bool) {
        self.create(enabled);
    }

    fn create_new(&mut self, enabled: bool) {
        self.create_new(enabled);
    }
}

impl WriteOptions for CapabilityOpenOptions {
    fn write(&mut self, enabled: bool) {
        self.write(enabled);
    }

    fn append(&mut self, enabled: bool) {
        self.append(enabled);
    }

    fn truncate(&mut self, enabled: bool) {
        self.truncate(enabled);
    }

    fn create(&mut self, enabled: bool) {
        self.create(enabled);
    }

    fn create_new(&mut self, enabled: bool) {
        self.create_new(enabled);
    }
}

impl Read for ActionFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Ambient(file) => file.read(buffer),
            Self::Limited(file) => file.read(buffer),
        }
    }
}

impl Write for ActionFile {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self {
            Self::Ambient(file) => file.write(buffer),
            Self::Limited(file) => file.write(buffer),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Ambient(file) => file.flush(),
            Self::Limited(file) => file.flush(),
        }
    }
}

impl ActionFile {
    fn sync_all(&self) -> io::Result<()> {
        match self {
            Self::Ambient(file) => file.sync_all(),
            Self::Limited(file) => file.sync_all(),
        }
    }
}

fn create_temporary_sibling(destination: &ActionPath) -> io::Result<(ActionPath, ActionFile)> {
    for _ in 0..32 {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random)
            .map_err(|error| io::Error::other(format!("random generator failed: {error}")))?;
        let name = format!(
            ".baudbound-download-{}.tmp",
            random
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        let temporary = destination.sibling(&name)?;
        match temporary.open_write(WriteMode::Overwrite, true) {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique temporary download file",
    ))
}
