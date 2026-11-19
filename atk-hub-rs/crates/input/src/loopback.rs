//! WASAPI loopback capture of whatever the default speakers are playing,
//! used by music rhythm lighting and audio-reactive wallpapers.

use crate::system::SystemError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient, IAudioClient,
    IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, STGM_READ,
};

const SILENT: u32 = 2;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

struct Com;

impl Com {
    fn init() -> Self {
        // SAFETY: balanced by `CoUninitialize` in `Drop` on the same thread.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: paired with the `CoInitializeEx` in `Com::init`.
        unsafe { CoUninitialize() };
    }
}

fn default_device() -> Result<IMMDevice, SystemError> {
    // SAFETY: standard COM activation of the system device enumerator.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        Ok(enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?)
    }
}

/// Friendly name of the default playback device.
#[must_use]
pub fn default_output_name() -> Option<String> {
    let _com = Com::init();
    let device = default_device().ok()?;
    // SAFETY: reads one string property from the device's property store.
    unsafe {
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let value = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        Some(value.to_string())
    }
}

/// Captures the default output until `stop` is set, handing `sink` blocks of
/// left and right samples (-1.0..1.0) with the sample rate.
///
/// # Errors
///
/// Returns an error when the device cannot be opened or disappears (for
/// example when the default output changes); callers may simply retry.
pub fn capture(
    stop: &AtomicBool,
    mut sink: impl FnMut(&[f32], &[f32], u32),
) -> Result<(), SystemError> {
    let _com = Com::init();
    let device = default_device()?;
    // SAFETY: COM calls on interfaces owned by this function. The mix format
    // pointer is freed with `CoTaskMemFree`, and capture buffers are only read
    // between `GetBuffer` and `ReleaseBuffer` for the frame count returned.
    unsafe {
        let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;
        let format = client.GetMixFormat()?;
        let fmt = *format;
        let channels = usize::from(fmt.nChannels.max(1));
        let bits = fmt.wBitsPerSample;
        let tag = fmt.wFormatTag;
        let rate = fmt.nSamplesPerSec;
        let float = tag == WAVE_FORMAT_IEEE_FLOAT || (tag == WAVE_FORMAT_EXTENSIBLE && bits == 32);
        let init = client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            10_000_000,
            0,
            format,
            None,
        );
        CoTaskMemFree(Some(format.cast()));
        init?;
        let capture: IAudioCaptureClient = client.GetService()?;
        client.Start()?;
        let mut left = Vec::new();
        let mut right = Vec::new();
        let result = loop {
            if stop.load(Ordering::Relaxed) {
                break Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
            left.clear();
            right.clear();
            let mut failed = None;
            loop {
                match capture.GetNextPacketSize() {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(error) => {
                        failed = Some(error);
                        break;
                    }
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                if let Err(error) =
                    capture.GetBuffer(&raw mut data, &raw mut frames, &raw mut flags, None, None)
                {
                    failed = Some(error);
                    break;
                }
                let count = frames as usize;
                if flags & SILENT != 0 || data.is_null() {
                    left.extend(std::iter::repeat_n(0.0, count));
                    right.extend(std::iter::repeat_n(0.0, count));
                } else if float {
                    let samples = std::slice::from_raw_parts(data.cast::<f32>(), count * channels);
                    for frame in samples.chunks_exact(channels) {
                        left.push(frame[0]);
                        right.push(frame[usize::from(channels > 1)]);
                    }
                } else if bits == 16 {
                    let samples = std::slice::from_raw_parts(data.cast::<i16>(), count * channels);
                    for frame in samples.chunks_exact(channels) {
                        left.push(f32::from(frame[0]) / 32768.0);
                        right.push(f32::from(frame[usize::from(channels > 1)]) / 32768.0);
                    }
                }
                let _ = capture.ReleaseBuffer(frames);
            }
            if !left.is_empty() {
                sink(&left, &right, rate);
            }
            if let Some(error) = failed {
                break Err(error.into());
            }
        };
        let _ = client.Stop();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a real playback device; run with `--ignored`.
    #[test]
    #[ignore = "uses the machine's audio device"]
    fn captures_from_default_output() {
        let stop = AtomicBool::new(false);
        let mut blocks = 0;
        let result = std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(400));
                stop.store(true, Ordering::Relaxed);
            });
            capture(&stop, |l, r, rate| {
                assert_eq!(l.len(), r.len());
                assert!(rate >= 8000);
                blocks += 1;
            })
        });
        // Silence produces no packets, so only require a clean start.
        assert!(result.is_ok(), "{result:?}");
        println!("device={:?} blocks={blocks}", default_output_name());
    }
}
