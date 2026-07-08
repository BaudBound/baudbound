use std::io::BufReader;

use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeContext,
};
use baudbound_script::read_package_asset;
use rodio::{Decoder, OutputStream, Sink};
use serde_json::{Map, Number, Value};

use super::config::{config_string, failed_error, required_string};

pub(super) fn run_sound_play(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let source = config_string(request, "source").unwrap_or_else(|| "asset".to_owned());
    if source.trim() == "file_path" {
        let file_path = required_string(request, "filePath")?;
        let file = std::fs::File::open(&file_path).map_err(|source| {
            failed_error(
                request,
                format!("failed to open audio file {file_path:?}: {source}"),
            )
        })?;
        play_audio_source(request, std::io::BufReader::new(file))?;

        return Ok(RuntimeActionResult {
            output_data: Map::from_iter([
                ("file_path".to_owned(), Value::String(file_path)),
                ("source".to_owned(), Value::String("file_path".to_owned())),
            ]),
        });
    }

    let asset_path = required_string(request, "assetPath")?;
    let package_path = context
        .package_path
        .as_ref()
        .ok_or_else(|| RuntimeActionError::Failed {
            action_type: request.action_type.clone(),
            message: "asset sound playback requires an installed package context".to_owned(),
        })?;
    let asset = read_package_asset(package_path, &asset_path).map_err(|source| {
        failed_error(
            request,
            format!("failed to read package audio asset {asset_path:?}: {source}"),
        )
    })?;
    play_audio_source(request, std::io::Cursor::new(asset.bytes.clone()))?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("asset_path".to_owned(), Value::String(asset.path)),
            (
                "bytes".to_owned(),
                Value::Number(Number::from(asset.bytes.len())),
            ),
            ("media_type".to_owned(), Value::String(asset.media_type)),
            ("source".to_owned(), Value::String("asset".to_owned())),
        ]),
    })
}

pub(super) fn play_audio_source<R>(
    request: &RuntimeActionRequest,
    source: R,
) -> Result<(), RuntimeActionError>
where
    R: std::io::Read + std::io::Seek + Send + Sync + 'static,
{
    let (_stream, handle) = OutputStream::try_default().map_err(|source| {
        failed_error(request, format!("failed to open audio output: {source}"))
    })?;
    let sink = Sink::try_new(&handle).map_err(|source| {
        failed_error(request, format!("failed to create audio sink: {source}"))
    })?;
    let decoded = Decoder::new(BufReader::new(source))
        .map_err(|source| failed_error(request, format!("failed to decode audio: {source}")))?;
    sink.append(decoded);
    sink.sleep_until_end();
    Ok(())
}
