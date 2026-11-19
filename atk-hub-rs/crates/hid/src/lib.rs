//! Direct HID drivers for ATK devices.
//!
//! The original app talks to device configuration interfaces through WebHID.
//! This crate does the same through `hidapi`: it finds the vendor interface,
//! writes numbered output reports and correlates the input-report replies.

pub mod compx;
pub mod dotscreen;
pub mod dpi;
pub mod headset;
mod kb_models;
pub mod keyboard;
mod models;
mod tables;

use hidapi::{HidApi, HidDevice};
use serde::Serialize;
use std::time::{Duration, Instant};

/// ATK / VGN vendor ID.
pub const ATK_VENDOR_ID: u16 = 0x3554;
/// VXE vendor ID.
pub const VXE_VENDOR_ID: u16 = 0x373B;

/// Errors from HID discovery and transfers.
#[derive(Debug, thiserror::Error)]
pub enum HidError {
    /// hidapi failure.
    #[error("HID error: {0}")]
    Api(#[from] hidapi::HidError),
    /// No matching interface is connected.
    #[error("no supported device connected")]
    NotFound,
    /// The receiver is present but the wireless mouse is asleep or off.
    #[error("mouse is asleep or switched off; move it to wake it up")]
    Offline,
    /// The device did not answer in time.
    #[error("device did not respond to command {0}")]
    Timeout(u8),
    /// Invalid caller input.
    #[error("{0}")]
    Invalid(String),
}

/// A request/response channel to one HID interface.
pub trait Transport {
    /// Writes `data` on `report_id` and returns the first input report whose
    /// first byte equals `expect`, without the report ID.
    ///
    /// # Errors
    ///
    /// Returns an I/O error or a timeout.
    fn transfer(&mut self, report_id: u8, data: &[u8], expect: u8) -> Result<Vec<u8>, HidError>;

    /// Like [`Transport::transfer`], but waits for the first report that
    /// `accept` approves.
    ///
    /// # Errors
    ///
    /// Returns an I/O error or a timeout.
    fn transfer_where(
        &mut self,
        report_id: u8,
        data: &[u8],
        accept: &dyn Fn(&[u8]) -> bool,
    ) -> Result<Vec<u8>, HidError> {
        let expect = data.first().copied().unwrap_or(0);
        let reply = self.transfer(report_id, data, expect)?;
        if accept(&reply) {
            Ok(reply)
        } else {
            Err(HidError::Timeout(expect))
        }
    }

    /// Writes `data` without waiting for a reply.
    ///
    /// # Errors
    ///
    /// Returns an I/O error.
    fn write_only(&mut self, report_id: u8, data: &[u8]) -> Result<(), HidError> {
        let expect = data.first().copied().unwrap_or(0);
        match self.transfer(report_id, data, expect) {
            Ok(_) | Err(HidError::Timeout(_)) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

/// A connected configuration interface.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// OS device path used to reopen the interface.
    pub path: String,
    /// Vendor ID.
    pub vendor_id: u16,
    /// Product ID.
    pub product_id: u16,
    /// Product string reported by the device.
    pub product: String,
    /// Usage page of the interface.
    pub usage_page: u16,
    /// Usage of the interface.
    pub usage: u16,
    /// Protocol family (`"compx"`).
    pub protocol: &'static str,
}

/// Lists connected COMPX mouse configuration interfaces.
///
/// # Errors
///
/// Returns an error when hidapi cannot initialise.
pub fn list_mice() -> Result<Vec<DeviceInfo>, HidError> {
    let api = HidApi::new()?;
    let vendor =
        |info: &&hidapi::DeviceInfo| matches!(info.vendor_id(), ATK_VENDOR_ID | VXE_VENDOR_ID);
    // WebHID matches the whole interface by its 0xFF04 marker collection, but
    // the report-ID-8 command channel lives in the sibling 0xFF02 collection.
    let marked: Vec<(u16, u16, i32)> = api
        .device_list()
        .filter(vendor)
        .filter(|info| info.usage_page() == compx::USAGE_PAGE && info.usage() == compx::USAGE)
        .map(|info| (info.vendor_id(), info.product_id(), info.interface_number()))
        .collect();
    let mut devices: Vec<DeviceInfo> = api
        .device_list()
        .filter(vendor)
        .filter(|info| {
            info.usage_page() == compx::COMMAND_USAGE_PAGE
                && info.usage() == compx::USAGE
                && marked.contains(&(info.vendor_id(), info.product_id(), info.interface_number()))
        })
        .map(|info| DeviceInfo {
            path: info.path().to_string_lossy().into_owned(),
            vendor_id: info.vendor_id(),
            product_id: info.product_id(),
            product: info.product_string().unwrap_or("ATK Mouse").to_owned(),
            usage_page: info.usage_page(),
            usage: info.usage(),
            protocol: "compx",
        })
        .collect();
    devices.dedup_by(|a, b| a.path == b.path);
    Ok(devices)
}

/// hidapi-backed transport.
pub struct HidTransport {
    device: HidDevice,
    timeout: Duration,
    output_len: Option<usize>,
}

impl std::fmt::Debug for HidTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HidTransport")
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl HidTransport {
    /// Opens an interface by the path returned from [`list_mice`].
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be opened.
    pub fn open(path: &str) -> Result<Self, HidError> {
        let api = HidApi::new()?;
        let c_path =
            std::ffi::CString::new(path).map_err(|error| HidError::Invalid(error.to_string()))?;
        let device = api.open_path(&c_path)?;
        Ok(Self {
            device,
            timeout: Duration::from_millis(1500),
            output_len: None,
        })
    }

    /// Changes how long a transfer waits for its reply.
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    /// Writes a report, padding it to the interface's declared output length.
    ///
    /// Windows rejects output buffers whose size differs from the report
    /// descriptor, so the first successful size is remembered.
    fn write_padded(&mut self, report_id: u8, data: &[u8]) -> Result<(), HidError> {
        let build = |len: usize| {
            let mut frame = vec![0_u8; len.max(data.len() + 1)];
            frame[0] = report_id;
            frame[1..=data.len()].copy_from_slice(data);
            frame
        };
        if let Some(len) = self.output_len {
            self.device.write(&build(len))?;
            return Ok(());
        }
        let mut last = None;
        for len in [data.len() + 1, 17, 33, 65, 21, 9] {
            match self.device.write(&build(len)) {
                Ok(_) => {
                    self.output_len = Some(len);
                    return Ok(());
                }
                Err(error) => last = Some(error),
            }
        }
        Err(last.map_or(HidError::NotFound, HidError::Api))
    }

    /// Writes a report without waiting for a reply.
    ///
    /// # Errors
    ///
    /// Returns an I/O error.
    pub fn send(&mut self, report_id: u8, data: &[u8]) -> Result<(), HidError> {
        self.write_padded(report_id, data)
    }

    /// Opens the first connected COMPX mouse.
    ///
    /// # Errors
    ///
    /// Returns [`HidError::NotFound`] when nothing is connected.
    pub fn open_first_mouse() -> Result<(Self, DeviceInfo), HidError> {
        let info = list_mice()?.into_iter().next().ok_or(HidError::NotFound)?;
        Ok((Self::open(&info.path)?, info))
    }
}

impl Transport for HidTransport {
    fn write_only(&mut self, report_id: u8, data: &[u8]) -> Result<(), HidError> {
        self.write_padded(report_id, data)
    }

    fn transfer(&mut self, report_id: u8, data: &[u8], expect: u8) -> Result<Vec<u8>, HidError> {
        self.transfer_where(report_id, data, &|payload: &[u8]| {
            payload.first() == Some(&expect)
        })
    }

    fn transfer_where(
        &mut self,
        report_id: u8,
        data: &[u8],
        accept: &dyn Fn(&[u8]) -> bool,
    ) -> Result<Vec<u8>, HidError> {
        let expect = data.first().copied().unwrap_or(0);
        // Drain stale reports so replies correlate with this request.
        let mut scratch = [0_u8; 1024];
        while self.device.read_timeout(&mut scratch, 0)? > 0 {}

        for _attempt in 0..3 {
            self.write_padded(report_id, data)?;

            let deadline = Instant::now() + self.timeout;
            while Instant::now() < deadline {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let wait = i32::try_from(remaining.as_millis()).unwrap_or(i32::MAX);
                let count = self.device.read_timeout(&mut scratch, wait)?;
                if count == 0 {
                    break;
                }
                // Numbered reports arrive with the report ID first.
                let payload = if scratch[0] == report_id && count > data.len() {
                    &scratch[1..count]
                } else {
                    &scratch[..count]
                };
                if accept(payload) {
                    return Ok(payload.to_vec());
                }
            }
        }
        Err(HidError::Timeout(expect))
    }
}
