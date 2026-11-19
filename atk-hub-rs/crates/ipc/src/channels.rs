//! Static channel inventory recovered from `dist-electron/main/index.js`.

use super::{Category, Channel};
use std::collections::BTreeMap;

/// All literal channels, in the order they appear in the main bundle.
pub static CHANNELS: &[Channel] = &[
    Channel::new("dotScreen:status"),
    Channel::new("dotScreen:stop"),
    Channel::new("dotScreen:start"),
    Channel::new("musicRhythm_getDevice"),
    Channel::new("musicRhythm_stopCollecting"),
    Channel::new("musicRhythm_startCollecting"),
    Channel::new("musicRhythm_updateVolume"),
    Channel::new("musicRhythm_createApp"),
    Channel::new("musicRhythm_destroyApp"),
    Channel::new("musicRhythm_changeState"),
    Channel::new("musicRhythm_deviceUpdate"),
    Channel::new("musicRhythm_syncEffects"),
    Channel::new("musicRhythm_getEffects"),
    Channel::new("gameEngine_getAppInfo"),
    Channel::new("gameEngine_selectExecutable"),
    Channel::new("atk-open-target:pick"),
    Channel::new("atk-open-target:applications"),
    Channel::new("atk-rust-agent:request"),
    Channel::new("audio-diagnostic"),
    Channel::new("sls-log"),
    Channel::new("atk-main-process-message"),
    Channel::new("getHeadsetAudioOperation"),
    Channel::new("dismissHeadsetAudioOperation"),
    Channel::new("headsetDriverLog"),
    Channel::new("woodenFish:locale"),
    Channel::new("woodenFish:sound-data"),
    Channel::new("woodenFish:set-sound"),
    Channel::new("woodenFish:get"),
    Channel::new("woodenFish:enable"),
    Channel::new("woodenFish:options"),
    Channel::new("woodenFish:drag"),
    Channel::new("woodenFish:tap"),
    Channel::new("music-practice:input"),
    Channel::new("hoopsPet:rename"),
    Channel::new("hoopsPet:get"),
    Channel::new("hoopsPet:sprite"),
    Channel::new("hoopsPet:upload-custom"),
    Channel::new("hoopsPet:ready"),
    Channel::new("hoopsPet:load-error"),
    Channel::new("hoopsPet:options"),
    Channel::new("hoopsPet:test-level-up"),
    Channel::new("hoopsPet:reset-position"),
    Channel::new("hoopsPet:locale"),
    Channel::new("hoopsPet:interact"),
    Channel::new("hoopsPet:drag"),
    Channel::new("app-exit:guard-state"),
    Channel::new("app-exit:check-result"),
    Channel::new("background-service-exit:pending"),
    Channel::new("background-service-exit:shown"),
    Channel::new("background-service-exit:choose"),
    Channel::new("changeNotificationSize"),
    Channel::new("gameAuto_setAutoSwitchEnabled"),
    Channel::new("gameAuto_startMonitoring"),
    Channel::new("gameAuto_stopMonitoring"),
    Channel::new("gameAuto_getStatus"),
    Channel::new("gameAuto_isCurrent"),
    Channel::new("floating-toolbar-overlay-close"),
    Channel::new("floating-toolbar-overlay-opacity"),
    Channel::new("floating-toolbar-overlay-action"),
    Channel::new("floating-toolbar-show-real-overlay-test"),
    Channel::new("floating-selection-set-enabled"),
    Channel::new("floating-selection-get-enabled"),
    Channel::new("floating-toolbar-read-clipboard"),
    Channel::new("floating-selection-debug-ping"),
    Channel::new("floating-selection-debug-status"),
    Channel::new("floating-selection-debug-try-grab"),
    Channel::new("floating-selection-get-global-accelerator"),
    Channel::new("floating-selection-set-global-accelerator"),
    Channel::new("floating-selection-get-clipboard-monitor"),
    Channel::new("floating-selection-set-clipboard-monitor"),
    Channel::new("floating-selection-get-global-hotkey-enabled"),
    Channel::new("floating-selection-set-global-hotkey-enabled"),
    Channel::new("floating-selection-get-mouse-gesture-hook"),
    Channel::new("floating-selection-set-mouse-gesture-hook"),
    Channel::new("floating-selection-get-double-ctrl-enabled"),
    Channel::new("floating-selection-set-double-ctrl-enabled"),
    Channel::new("floating-selection-get-restore-clipboard-after-simulated-copy"),
    Channel::new("floating-selection-set-restore-clipboard-after-simulated-copy"),
    Channel::new("plugin-install-bundle"),
    Channel::new("plugin-get-floating-selection-renderer-path"),
    Channel::new("plugin-bundle-has-floating-renderer"),
    Channel::new("plugin-id-has-floating-renderer"),
    Channel::new("plugin-get-pc-script-renderer-path"),
    Channel::new("plugin-bundle-has-pc-script-renderer"),
    Channel::new("plugin-id-has-pc-script-renderer"),
    Channel::new("plugin-remove-installed-files"),
    Channel::new("pc-script:start"),
    Channel::new("pc-script:stop"),
    Channel::new("pc-script:status"),
    Channel::new("pc-script:get-state"),
    Channel::new("pc-script:save-and-apply"),
    Channel::new("pc-script:validate-lua-script"),
    Channel::new("pc-script:debug-move"),
    Channel::new("pc-script:probe-keys"),
    Channel::new("pc-script:get-console-logs"),
    Channel::new("pc-script:clear-console-logs"),
    Channel::new("pc-script:peek-error"),
    Channel::new("pc-script:import-lua-file"),
    Channel::new("pc-script:export-lua-file"),
    Channel::new("tary-menu-list"),
    Channel::new("tray-menu:action"),
    Channel::new("tray-tooltip:update"),
    Channel::new("tray-menu:perform"),
    Channel::new("get-temp-path"),
    Channel::new("appVersion"),
    Channel::new("restartApp"),
    Channel::new("open-win"),
    Channel::new("open-dev-tools"),
    Channel::new("getSetting"),
    Channel::new("setSetting"),
    Channel::new("globalHubHotkey:tryRegister"),
    Channel::new("startUpdate"),
    Channel::new("installUpdate"),
    Channel::new("downloadFile"),
    Channel::new("fileExist"),
    Channel::new("exit"),
    Channel::new("dialog"),
    Channel::new("driverRestartPrompt:getLastAck"),
    Channel::new("driverRestartPrompt:setAck"),
    Channel::new("driverRestartPrompt:executeRestart"),
    Channel::new("driverRestartPrompt:getDevForceShow"),
    Channel::new("driverRestartPrompt:getBuildVersionKey"),
    Channel::new("selectFile"),
    Channel::new("showSaveDialog"),
    Channel::new("generatedData"),
    Channel::new("scanDevice"),
    Channel::new("wallpaper:reload"),
    Channel::new("wallpaper:import"),
    Channel::new("wallpaper:download"),
    Channel::new("wallpaper:cancel-download"),
    Channel::new("wallpaper:get-downloaded"),
    Channel::new("wallpaper:is-downloaded"),
    Channel::new("wallpaper:delete"),
    Channel::new("wallpaper:apply"),
    Channel::new("wallpaper:clear"),
    Channel::new("wallpaper:get-current"),
    Channel::new("wallpaper:get-applied-monitors"),
    Channel::new("wallpaper:get-displays"),
    Channel::new("wallpaper:update-brightness"),
    Channel::new("wallpaper:update-style"),
    Channel::new("wallpaper:list"),
    Channel::new("wallpaper:html-ready"),
    Channel::new("close-win"),
    Channel::new("hide-win"),
    Channel::new("notification:show"),
    Channel::new("capsLockHint:show"),
    Channel::new("lockHint:debugSnapshot"),
    Channel::new("lockHint:testShow"),
    Channel::new("lockHint:resetDisplayState"),
    Channel::new("runMusicServer"),
    Channel::new("runMusic"),
    Channel::new("closeMusicServer"),
    Channel::new("runBykbMusicServer"),
    Channel::new("changeBykbEffect"),
    Channel::new("changeBykbParam"),
    Channel::new("runBykbMusic"),
    Channel::new("closeBykbMusicServer"),
    Channel::new("checkBykbServiceRunning"),
    Channel::new("startBykbService"),
    Channel::new("stopBykbService"),
    Channel::new("getBykbServiceInfo"),
    Channel::new("max-win"),
    Channel::new("getPointerSpeed"),
    Channel::new("setPointerSpeed"),
    Channel::new("getRollerSpeed"),
    Channel::new("setRollerSpeed"),
    Channel::new("audioSystemManage"),
    Channel::new("updateTheme"),
    Channel::new("quit-app"),
];

/// Methods accepted by the Rust-agent bridge in the main process.
pub static AGENT_METHODS: &[&str] = &[
    "device.list",
    "device.connect",
    "device.disconnect",
    "rules.set",
    "action.execute",
    "device.write",
    "state.get",
    "trace.set",
    "bindings.set",
];

/// Returns all recovered literal channels.
#[must_use]
pub const fn all() -> &'static [Channel] {
    CHANNELS
}

/// Finds a channel by its exact name.
#[must_use]
pub fn find(name: &str) -> Option<Channel> {
    CHANNELS
        .iter()
        .copied()
        .find(|channel| channel.as_str() == name)
}

/// Returns `true` when `name` is an allowed Rust-agent method.
#[must_use]
pub fn is_agent_method(name: &str) -> bool {
    AGENT_METHODS.contains(&name)
}

/// Classifies a channel into its coarse feature area.
#[must_use]
pub fn category(name: &str) -> Category {
    if name.starts_with("wallpaper:") {
        return Category::Wallpaper;
    }
    if name.starts_with("woodenFish:")
        || name.starts_with("hoopsPet:")
        || name.starts_with("capsLockHint:")
        || name.starts_with("lockHint:")
        || name == "notification:show"
    {
        return Category::Widget;
    }
    if name.starts_with("floating-")
        || name.starts_with("plugin-")
        || name.starts_with("pc-script:")
        || name == "globalHubHotkey:tryRegister"
    {
        return Category::Automation;
    }
    if name.starts_with("gameAuto_") || name.starts_with("gameEngine_") {
        return Category::Game;
    }
    if name.starts_with("atk-")
        || name == "sls-log"
        || name == "atk-main-process-message"
        || name == "audio-diagnostic"
    {
        return Category::Agent;
    }
    if name.starts_with("musicRhythm_")
        || name.starts_with("music-practice:")
        || name.starts_with("music")
        || name.starts_with("runMusic")
        || name.starts_with("closeMusic")
        || name.starts_with("runBykb")
        || name.starts_with("closeBykb")
        || name.starts_with("startBykb")
        || name.starts_with("stopBykb")
        || name.starts_with("getBykb")
        || name.starts_with("checkBykb")
        || name.starts_with("changeBykb")
        || name.starts_with("getPointer")
        || name.starts_with("setPointer")
        || name.starts_with("getRoller")
        || name.starts_with("setRoller")
        || matches!(
            name,
            "dotScreen:status"
                | "dotScreen:stop"
                | "dotScreen:start"
                | "getHeadsetAudioOperation"
                | "dismissHeadsetAudioOperation"
                | "headsetDriverLog"
                | "audioSystemManage"
                | "scanDevice"
                | "generatedData"
        )
    {
        return Category::Device;
    }
    Category::System
}

/// Groups all channels by category, preserving channel order within a group.
#[must_use]
pub fn by_category() -> BTreeMap<Category, Vec<Channel>> {
    let mut groups: BTreeMap<Category, Vec<Channel>> = BTreeMap::new();
    for channel in CHANNELS {
        groups
            .entry(category(channel.as_str()))
            .or_default()
            .push(*channel);
    }
    groups
}
