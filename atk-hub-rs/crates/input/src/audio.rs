//! Windows Core Audio master volume, mute and endpoint enumeration
//! (`audioSystemManage` / `window.loudness`).

use crate::system::SystemError;
use serde::Serialize;
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, STGM_READ,
};

/// An active Windows audio output device.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioEndpoint {
    /// Windows MMDevice endpoint ID.
    pub id: String,
    /// Human-readable device name.
    pub name: String,
    /// Whether this is the current default playback device.
    pub is_default: bool,
}

/// Snapshot of system master volume, mute state and active playback devices.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAudioState {
    /// Master volume percentage (0-100).
    pub volume: u32,
    /// Whether master output is muted.
    pub muted: bool,
    /// Name of the default playback device.
    pub default_device: Option<String>,
    /// Active audio output devices.
    pub devices: Vec<AudioEndpoint>,
}

struct ComGuard(bool);

impl ComGuard {
    fn init() -> Self {
        // SAFETY: COM init on current thread; balanced by Drop when S_OK/S_FALSE.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self(hr.is_ok())
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: Matches the successful CoInitializeEx call above.
            unsafe { CoUninitialize() };
        }
    }
}

fn default_endpoint_volume() -> Result<IAudioEndpointVolume, SystemError> {
    // SAFETY: Standard Core Audio COM activation on the default render endpoint.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device: IMMDevice = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        Ok(volume)
    }
}

fn device_name(device: &IMMDevice) -> String {
    // SAFETY: Opens read-only property store and frees allocated PWSTR.
    unsafe {
        let Ok(store) = device.OpenPropertyStore(STGM_READ) else {
            return "Audio Device".into();
        };
        let Ok(prop) = store.GetValue(&PKEY_Device_FriendlyName) else {
            return "Audio Device".into();
        };
        let Ok(pwstr) = PropVariantToStringAlloc(&prop) else {
            return "Audio Device".into();
        };
        let name = pwstr.to_string().unwrap_or_else(|_| "Audio Device".into());
        CoTaskMemFree(Some(pwstr.0.cast()));
        name
    }
}

fn device_id(device: &IMMDevice) -> String {
    // SAFETY: GetId returns a CoTaskMem-allocated string.
    unsafe {
        let Ok(pwstr) = device.GetId() else {
            return String::new();
        };
        let id = pwstr.to_string().unwrap_or_default();
        CoTaskMemFree(Some(pwstr.0.cast()));
        id
    }
}

/// Reads the current master volume (0-100).
///
/// # Errors
///
/// Returns a Core Audio error when no default output endpoint is available.
pub fn get_master_volume() -> Result<u32, SystemError> {
    let _com = ComGuard::init();
    let vol = default_endpoint_volume()?;
    // SAFETY: GetMasterVolumeLevelScalar returns a normalized 0.0..=1.0 float.
    let scalar = unsafe { vol.GetMasterVolumeLevelScalar()? };
    Ok((scalar.clamp(0.0, 1.0) * 100.0).round() as u32)
}

/// Sets the master volume (0-100).
///
/// # Errors
///
/// Returns a Core Audio error when the endpoint rejects the update.
pub fn set_master_volume(percent: u32) -> Result<(), SystemError> {
    let _com = ComGuard::init();
    let vol = default_endpoint_volume()?;
    let scalar = (percent.min(100) as f32) / 100.0;
    // SAFETY: SetMasterVolumeLevelScalar takes a 0.0..=1.0 float and null context GUID.
    unsafe { vol.SetMasterVolumeLevelScalar(scalar, std::ptr::null())? };
    Ok(())
}

/// Reads the current master mute state.
///
/// # Errors
///
/// Returns a Core Audio error when no default output endpoint is available.
pub fn get_muted() -> Result<bool, SystemError> {
    let _com = ComGuard::init();
    let vol = default_endpoint_volume()?;
    // SAFETY: GetMute returns a Win32 BOOL.
    let muted = unsafe { vol.GetMute()? };
    Ok(muted.as_bool())
}

/// Sets the master mute state.
///
/// # Errors
///
/// Returns a Core Audio error when the endpoint rejects the update.
pub fn set_muted(muted: bool) -> Result<(), SystemError> {
    let _com = ComGuard::init();
    let vol = default_endpoint_volume()?;
    // SAFETY: SetMute takes a bool and null event context GUID.
    unsafe { vol.SetMute(muted, std::ptr::null())? };
    Ok(())
}

/// Lists active Windows audio playback endpoints.
///
/// # Errors
///
/// Returns a Core Audio error if enumeration fails.
pub fn list_output_devices() -> Result<Vec<AudioEndpoint>, SystemError> {
    let _com = ComGuard::init();
    // SAFETY: Standard Core Audio endpoint enumeration.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let default_id = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .ok()
            .map(|d| device_id(&d))
            .unwrap_or_default();
        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let count = collection.GetCount()?;
        let mut out = Vec::with_capacity(count as usize);
        for index in 0..count {
            if let Ok(device) = collection.Item(index) {
                let id = device_id(&device);
                let is_default = !default_id.is_empty() && id == default_id;
                out.push(AudioEndpoint {
                    id,
                    name: device_name(&device),
                    is_default,
                });
            }
        }
        Ok(out)
    }
}

/// Reads master volume, mute state and output devices in one call.
///
/// # Errors
///
/// Returns a Core Audio error when volume or mute cannot be read.
pub fn system_audio_state() -> Result<SystemAudioState, SystemError> {
    let volume = get_master_volume()?;
    let muted = get_muted()?;
    let devices = list_output_devices().unwrap_or_default();
    let default_device = devices
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.name.clone());
    Ok(SystemAudioState {
        volume,
        muted,
        default_device,
        devices,
    })
}
