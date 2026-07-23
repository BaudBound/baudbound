use std::{io::BufReader, time::Duration};

use baudbound_runtime::{
    RuntimeActionError, RuntimeActionRequest, RuntimeActionResult, RuntimeContext,
};
use baudbound_script::read_package_asset;
use rodio::{Decoder, DeviceSinkBuilder, Player, Source, source::SineWave};
use serde_json::{Map, Number, Value};

use super::config::{config_string, failed_error, required_string};

const MIN_BEEP_FREQUENCY_HZ: f64 = 20.0;
const MAX_BEEP_FREQUENCY_HZ: f64 = 20_000.0;
const MIN_BEEP_DURATION_MS: f64 = 10.0;
const MAX_BEEP_DURATION_MS: f64 = 5_000.0;
const BEEP_AMPLITUDE: f32 = 0.2;
const PLAYBACK_CANCELLATION_INTERVAL: Duration = Duration::from_millis(25);

pub(super) fn run_beep(
    request: &RuntimeActionRequest,
    context: &RuntimeContext,
) -> Result<RuntimeActionResult, RuntimeActionError> {
    let (frequency_hz, duration_ms) = beep_config(request)?;
    let device_sink = DeviceSinkBuilder::open_default_sink().map_err(|source| {
        failed_error(request, format!("failed to open audio output: {source}"))
    })?;
    let player = Player::connect_new(device_sink.mixer());
    let tone = SineWave::new(frequency_hz as f32)
        .take_duration(Duration::from_secs_f64(duration_ms / 1_000.0))
        .amplify(BEEP_AMPLITUDE);
    player.append(tone);
    wait_for_playback(&player, &context.cancellation)?;

    Ok(RuntimeActionResult {
        output_data: Map::from_iter([
            ("frequency_hz".to_owned(), Value::from(frequency_hz)),
            ("duration_ms".to_owned(), Value::from(duration_ms)),
        ]),
    })
}

pub(super) fn beep_config(
    request: &RuntimeActionRequest,
) -> Result<(f64, f64), RuntimeActionError> {
    let frequency_hz = bounded_number(
        request,
        "frequencyHz",
        800.0,
        MIN_BEEP_FREQUENCY_HZ,
        MAX_BEEP_FREQUENCY_HZ,
    )?;
    let duration_ms = bounded_number(
        request,
        "durationMs",
        200.0,
        MIN_BEEP_DURATION_MS,
        MAX_BEEP_DURATION_MS,
    )?;
    Ok((frequency_hz, duration_ms))
}

fn bounded_number(
    request: &RuntimeActionRequest,
    key: &str,
    default: f64,
    minimum: f64,
    maximum: f64,
) -> Result<f64, RuntimeActionError> {
    let value = config_string(request, key)
        .map(|value| value.trim().parse::<f64>())
        .transpose()
        .map_err(|source| failed_error(request, format!("invalid {key}: {source}")))?
        .unwrap_or(default);
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return Err(failed_error(
            request,
            format!("{key} must be between {minimum} and {maximum}"),
        ));
    }
    Ok(value)
}

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
        play_audio_source(
            request,
            std::io::BufReader::new(file),
            &context.cancellation,
        )?;

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
    play_audio_source(
        request,
        std::io::Cursor::new(asset.bytes.clone()),
        &context.cancellation,
    )?;

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
    cancellation: &baudbound_runtime::RuntimeCancellationToken,
) -> Result<(), RuntimeActionError>
where
    R: std::io::Read + std::io::Seek + Send + Sync + 'static,
{
    let device_sink = DeviceSinkBuilder::open_default_sink().map_err(|source| {
        failed_error(request, format!("failed to open audio output: {source}"))
    })?;
    let player = Player::connect_new(device_sink.mixer());
    let decoded = Decoder::try_from(BufReader::new(source))
        .map_err(|source| failed_error(request, format!("failed to decode audio: {source}")))?;
    player.append(decoded);
    wait_for_playback(&player, cancellation)
}

trait PlaybackSink {
    fn is_empty(&self) -> bool;
    fn stop(&self);
}

impl PlaybackSink for Player {
    fn is_empty(&self) -> bool {
        self.empty()
    }

    fn stop(&self) {
        Player::stop(self);
    }
}

fn wait_for_playback(
    sink: &impl PlaybackSink,
    cancellation: &baudbound_runtime::RuntimeCancellationToken,
) -> Result<(), RuntimeActionError> {
    loop {
        if cancellation.is_cancelled() {
            sink.stop();
            return Err(RuntimeActionError::Cancelled);
        }
        if sink.is_empty() {
            return Ok(());
        }
        if cancellation.wait_for(PLAYBACK_CANCELLATION_INTERVAL) {
            sink.stop();
            return Err(RuntimeActionError::Cancelled);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::{Duration, Instant},
    };

    use baudbound_runtime::{RuntimeActionError, RuntimeCancellationToken};

    use super::{PlaybackSink, wait_for_playback};

    #[derive(Default)]
    struct TestSink {
        empty: AtomicBool,
        stopped: AtomicBool,
    }

    impl PlaybackSink for TestSink {
        fn is_empty(&self) -> bool {
            self.empty.load(Ordering::Acquire)
        }

        fn stop(&self) {
            self.stopped.store(true, Ordering::Release);
        }
    }

    #[test]
    fn completed_playback_returns_without_stopping_the_sink() {
        let sink = TestSink {
            empty: AtomicBool::new(true),
            stopped: AtomicBool::new(false),
        };

        wait_for_playback(&sink, &RuntimeCancellationToken::new())
            .expect("completed playback should succeed");

        assert!(!sink.stopped.load(Ordering::Acquire));
    }

    #[test]
    fn cancellation_stops_active_playback_promptly() {
        let sink = TestSink::default();
        let cancellation = RuntimeCancellationToken::new();
        let signal = cancellation.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(30));
            signal.cancel();
        });
        let started = Instant::now();

        let error = wait_for_playback(&sink, &cancellation)
            .expect_err("cancelled playback should return cancellation");
        canceller.join().expect("canceller should finish");

        assert!(matches!(error, RuntimeActionError::Cancelled));
        assert!(sink.stopped.load(Ordering::Acquire));
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}
