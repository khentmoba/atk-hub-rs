//! Tauri commands for ATK headsets.

use atk_hid::headset::{self, Bolutek, BolutekState, HeadsetInfo, KtMicro, KtState, PeqPoint};
use serde::Deserialize;

async fn blocking<R: Send + 'static>(
    work: impl FnOnce() -> Result<R, atk_hid::HidError> + Send + 'static,
) -> Result<R, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

/// Bolutek settings to write; absent fields are untouched.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BolutekPatch {
    eq_mode: Option<u8>,
    surround: Option<bool>,
    spatial: Option<bool>,
    auto_power_off: Option<u8>,
    sidetone: Option<bool>,
    sidetone_volume: Option<u8>,
    prompt_volume: Option<u8>,
    light_mode: Option<u8>,
    light_speed: Option<u16>,
}

#[tauri::command]
pub async fn headset_list() -> Result<Vec<HeadsetInfo>, String> {
    blocking(headset::list_headsets).await
}

#[tauri::command]
pub async fn headset_bolutek_state(path: String) -> Result<BolutekState, String> {
    blocking(move || Bolutek::open(&path)?.state()).await
}

#[tauri::command]
pub async fn headset_bolutek_apply(
    path: String,
    patch: BolutekPatch,
) -> Result<BolutekState, String> {
    blocking(move || {
        let mut driver = Bolutek::open(&path)?;
        if let Some(mode) = patch.eq_mode {
            driver.set_eq_mode(mode)?;
        }
        if let Some(on) = patch.surround {
            driver.set_surround(on)?;
        }
        if let Some(on) = patch.spatial {
            driver.set_spatial(on)?;
        }
        if let Some(level) = patch.auto_power_off {
            driver.set_auto_power_off(level)?;
        }
        if patch.sidetone.is_some() || patch.sidetone_volume.is_some() {
            driver.set_sidetone(
                patch.sidetone.unwrap_or(true),
                patch.sidetone_volume.unwrap_or(0),
            )?;
        }
        if let Some(volume) = patch.prompt_volume {
            driver.set_prompt_volume(volume)?;
        }
        if let Some(mode) = patch.light_mode {
            driver.set_lighting(mode, patch.light_speed.unwrap_or(50))?;
        }
        driver.state()
    })
    .await
}

#[tauri::command]
pub async fn headset_bolutek_peq(path: String) -> Result<Vec<PeqPoint>, String> {
    blocking(move || Bolutek::open(&path)?.peq()).await
}

#[tauri::command]
pub async fn headset_bolutek_set_peq(path: String, points: Vec<PeqPoint>) -> Result<(), String> {
    blocking(move || {
        let mut driver = Bolutek::open(&path)?;
        driver.set_eq_mode(2)?;
        driver.set_peq(&points)
    })
    .await
}

#[tauri::command]
pub async fn headset_kt_state(path: String) -> Result<KtState, String> {
    blocking(move || KtMicro::open(&path)?.state()).await
}

#[tauri::command]
pub async fn headset_kt_apply(path: String, state: KtState) -> Result<KtState, String> {
    blocking(move || {
        let mut driver = KtMicro::open(&path)?;
        driver.apply(&state)?;
        driver.state()
    })
    .await
}

#[tauri::command]
pub async fn headset_factory_reset(path: String, family: headset::Family) -> Result<(), String> {
    blocking(move || match family {
        headset::Family::Bolutek => Bolutek::open(&path)?.factory_reset(),
        headset::Family::Ktmicro => KtMicro::open(&path)?.factory_reset(),
    })
    .await
}
