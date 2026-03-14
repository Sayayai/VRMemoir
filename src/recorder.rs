use anyhow::{anyhow, Result};
#[cfg(windows)]
use std::collections::VecDeque;
#[cfg(windows)]
use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use sysinfo::System;
#[cfg(windows)]
use tracing::warn;
use tracing::{error, info};

use crate::t;

/// Find VRChat.exe process ID.
pub fn find_vrchat_pid() -> Option<u32> {
    // Optimization: Use targeted process refresh instead of System::new_all()
    // to reduce CPU and I/O overhead.
    let mut system = System::new();
    system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for (pid, process) in system.processes() {
        let process: &sysinfo::Process = process;
        let name = process.name().to_string_lossy();
        if name.eq_ignore_ascii_case("VRChat.exe") || name.eq_ignore_ascii_case("vrchat.exe") {
            return Some((*pid).as_u32());
        }
    }
    None
}

/// Check if a specific process ID is still running.
pub fn is_process_running(pid: u32) -> bool {
    let mut system = System::new();
    let sys_pid = sysinfo::Pid::from(pid as usize);
    system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sys_pid]), true);
    system.process(sys_pid).is_some()
}

/// Read VRChat's configured microphone device name from the Windows registry.
/// Returns None if not set (default device) or on error.
pub fn read_vrchat_mic_device() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu.open_subkey("Software\\VRChat\\VRChat").ok()?;

        // Try Desktop device name first, then generic
        for name in &[
            "VRC_INPUT_MIC_DEVICE_NAME_Desktop_h2596021377",
            "VRC_INPUT_MIC_DEVICE_NAME_h3782209548",
        ] {
            if let Ok(bytes) = key.get_raw_value(name) {
                let data = bytes.bytes;
                // REG_BINARY: UTF-8 string terminated by null bytes
                let s = String::from_utf8_lossy(&data);
                let trimmed = s.trim_end_matches('\0').trim();
                if !trimmed.is_empty() {
                    info!("{}", t!("mic_device_reg", trimmed));
                    return Some(trimmed.to_string());
                }
            }
        }
        info!("{}", t!("mic_device_default"));
        None
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Configuration for mic recording
#[derive(Clone, Debug)]
pub struct MicConfig {
    #[allow(dead_code)]
    pub enabled: bool,
    #[allow(dead_code)]
    pub device_name: Option<String>,
}

/// Audio recorder using WASAPI process loopback.
/// Captures audio from VRChat only and optionally mixes in microphone input.
/// Encodes to OGG/Opus in real time.
pub struct AudioRecorder {
    stop_flag: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<Result<Duration>>>,
}

impl AudioRecorder {
    /// Start recording audio from the given process ID.
    /// If `mic_config.enabled` is true, also captures and mixes microphone audio.
    /// Saves to `output_path` as an OGG/Opus file.
    pub fn start(process_id: u32, output_path: PathBuf, mic_config: MicConfig) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();

        let handle = thread::Builder::new()
            .name("AudioRecorder".to_string())
            .spawn(move || -> Result<Duration> {
                match capture_thread(process_id, &output_path, &stop_clone, &mic_config) {
                    Ok(duration) => {
                        info!(
                            "{}",
                            t!(
                                "recording_finished",
                                output_path.display(),
                                duration.as_secs_f64()
                            )
                        );
                        Ok(duration)
                    }
                    Err(e) => {
                        error!("{}", t!("recording_save_failed", e));
                        Err(e)
                    }
                }
            })?;

        Ok(Self {
            stop_flag,
            thread_handle: Some(handle),
        })
    }

    /// Stop recording and return the duration.
    pub fn stop(mut self) -> Result<Duration> {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread_handle.take() {
            match handle.join() {
                Ok(result) => result,
                Err(_) => Err(anyhow!("Recording thread panicked")),
            }
        } else {
            Err(anyhow!("Recording thread already stopped"))
        }
    }

    /// Check if still recording
    #[allow(dead_code)]
    pub fn is_recording(&self) -> bool {
        self.thread_handle
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }
}

impl Drop for AudioRecorder {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }
}

/// The actual capture thread function.
/// Uses WASAPI process loopback to capture VRChat audio, optionally mixes mic input.
#[cfg(windows)]
fn capture_thread(
    process_id: u32,
    output_path: &std::path::Path,
    stop_flag: &AtomicBool,
    mic_config: &MicConfig,
) -> Result<Duration> {
    // Initialize COM for this thread
    wasapi::initialize_mta()
        .ok()
        .map_err(|e| anyhow!("COM init failed: {:?}", e))?;

    // Set up capture format: 48kHz, 16-bit, stereo (Opus native)
    let sample_rate: usize = 48000;
    let channels: usize = 2;
    let bits_per_sample: usize = 16;

    let desired_format = wasapi::WaveFormat::new(
        bits_per_sample,
        bits_per_sample,
        &wasapi::SampleType::Int,
        sample_rate,
        channels,
        None,
    );

    let blockalign = desired_format.get_blockalign();

    info!(
        "WASAPI: PID={}, {}Hz, {}ch, {}bit",
        process_id, sample_rate, channels, bits_per_sample
    );

    // Create process-specific loopback client (VRChat audio)
    let mut audio_client = wasapi::AudioClient::new_application_loopback_client(process_id, true)
        .map_err(|e| {
        anyhow!(
            "Failed to create loopback client for PID {}: {:?}",
            process_id,
            e
        )
    })?;

    // Initialize in shared event mode with autoconvert
    let mode = wasapi::StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: 0,
    };
    audio_client
        .initialize_client(&desired_format, &wasapi::Direction::Capture, &mode)
        .map_err(|e| anyhow!("Failed to initialize capture client: {:?}", e))?;

    let h_event = audio_client
        .set_get_eventhandle()
        .map_err(|e| anyhow!("Failed to get event handle: {:?}", e))?;

    let capture_client = audio_client
        .get_audiocaptureclient()
        .map_err(|e| anyhow!("Failed to get capture client: {:?}", e))?;

    // --- Mic capture setup (optional) ---
    let mic_capture = if mic_config.enabled {
        match setup_mic_capture(&desired_format, mic_config) {
            Ok(mc) => {
                info!("{}", t!("mic_capture_success"));
                Some(mc)
            }
            Err(e) => {
                warn!("{}", t!("mic_capture_failed", e));
                None
            }
        }
    } else {
        info!("{}", t!("mic_recording_disabled"));
        None
    };

    // Set up Opus encoder
    let mut opus_encoder = audiopus::coder::Encoder::new(
        audiopus::SampleRate::Hz48000,
        audiopus::Channels::Stereo,
        audiopus::Application::Audio,
    )
    .map_err(|e| anyhow!("Opus encoder init failed: {:?}", e))?;

    // Set bitrate to 64kbps
    opus_encoder
        .set_bitrate(audiopus::Bitrate::BitsPerSecond(64000))
        .map_err(|e| anyhow!("Failed to set Opus bitrate: {:?}", e))?;

    // Opus frame size: 960 samples at 48kHz = 20ms
    let opus_frame_size: usize = 960;
    let frame_bytes = opus_frame_size * channels * (bits_per_sample / 8);

    // OGG stream writer
    let mut ogg_file =
        File::create(output_path).map_err(|e| anyhow!("Failed to create output file: {:?}", e))?;

    let serial = rand_serial();
    let mut ogg_stream = ogg::PacketWriter::new(&mut ogg_file);

    // Write Opus header packets
    write_opus_header(&mut ogg_stream, serial, sample_rate as u32, channels as u16)?;

    // Audio sample queues
    let mut vrc_queue: VecDeque<u8> = VecDeque::with_capacity(frame_bytes * 4);
    let mut mic_queue: VecDeque<u8> = VecDeque::with_capacity(frame_bytes * 4);
    let mut opus_output = vec![0u8; 4000]; // Opus output buffer
    let mut granule_pos: u64 = 0;

    // Start streams
    audio_client
        .start_stream()
        .map_err(|e| anyhow!("Failed to start VRChat stream: {:?}", e))?;

    if let Some(ref mc) = mic_capture {
        mc.client_wrapper
            .start_stream()
            .map_err(|e| anyhow!("Failed to start mic stream: {:?}", e))?;
    }

    let start_time = std::time::Instant::now();

    info!(
        "{}",
        t!(
            "recording_started",
            process_id,
            if mic_capture.is_some() {
                t!("with_mic")
            } else {
                String::new()
            }
        )
    );

    loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        // Read VRChat audio
        let new_frames = capture_client
            .get_next_packet_size()
            .unwrap_or(None)
            .unwrap_or(0);

        if new_frames > 0 {
            let additional = (new_frames as usize * blockalign as usize)
                .saturating_sub(vrc_queue.capacity() - vrc_queue.len());
            vrc_queue.reserve(additional);
            if let Err(e) = capture_client.read_from_device_to_deque(&mut vrc_queue) {
                warn!("{}", t!("vrchat_capture_read_error", format!("{:?}", e)));
            }
        }

        // Read mic audio (if enabled)
        if let Some(ref mc) = mic_capture {
            let mic_frames = mc
                .capture_client
                .get_next_packet_size()
                .unwrap_or(None)
                .unwrap_or(0);
            if mic_frames > 0 {
                let additional = (mic_frames as usize * blockalign as usize)
                    .saturating_sub(mic_queue.capacity() - mic_queue.len());
                mic_queue.reserve(additional);
                if let Err(e) = mc.capture_client.read_from_device_to_deque(&mut mic_queue) {
                    warn!("{}", t!("mic_capture_read_error", format!("{:?}", e)));
                }
            }
        }

        // Encode complete Opus frames
        // When mic is enabled, we mix; otherwise just use VRChat audio
        while vrc_queue.len() >= frame_bytes {
            let mut vrc_frame = vec![0u8; frame_bytes];
            for byte in vrc_frame.iter_mut() {
                *byte = vrc_queue.pop_front().unwrap();
            }

            // Convert VRChat bytes to i16 samples
            let vrc_pcm: Vec<i16> = vrc_frame
                .chunks_exact(2)
                .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();

            let final_pcm = if mic_capture.is_some() && mic_queue.len() >= frame_bytes {
                // Extract mic frame
                let mut mic_frame = vec![0u8; frame_bytes];
                for byte in mic_frame.iter_mut() {
                    *byte = mic_queue.pop_front().unwrap();
                }
                let mic_pcm: Vec<i16> = mic_frame
                    .chunks_exact(2)
                    .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                    .collect();

                // Mix: add samples with clamping
                mix_pcm(&vrc_pcm, &mic_pcm)
            } else {
                vrc_pcm
            };

            // Encode with Opus
            match opus_encoder.encode(&final_pcm, &mut opus_output) {
                Ok(encoded_len) => {
                    granule_pos += opus_frame_size as u64;
                    if let Err(e) = ogg_stream.write_packet(
                        opus_output[..encoded_len].to_vec(),
                        serial,
                        ogg::PacketWriteEndInfo::NormalPacket,
                        granule_pos,
                    ) {
                        warn!("{}", t!("ogg_write_error", format!("{:?}", e)));
                    }
                }
                Err(e) => {
                    warn!("{}", t!("opus_encode_error", format!("{:?}", e)));
                }
            }
        }

        // Wait for next buffer event (timeout 300ms)
        if h_event.wait_for_event(300).is_err() {
            // Timeout — check if process is still alive
            if !is_process_running(process_id) {
                info!("{}", t!("vrchat_exited_stop"));
                break;
            }
        }
    }

    // Stop streams
    let _ = audio_client.stop_stream();
    if let Some(ref mc) = mic_capture {
        let _ = mc.client_wrapper.stop_stream();
    }

    // Flush remaining samples (pad with silence if needed for last frame)
    if !vrc_queue.is_empty() {
        let mut last_frame = vec![0u8; frame_bytes];
        for (i, byte) in vrc_queue.iter().enumerate() {
            if i < frame_bytes {
                last_frame[i] = *byte;
            }
        }
        let pcm_samples: Vec<i16> = last_frame
            .chunks_exact(2)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();

        if let Ok(encoded_len) = opus_encoder.encode(&pcm_samples, &mut opus_output) {
            granule_pos += opus_frame_size as u64;
            let _ = ogg_stream.write_packet(
                opus_output[..encoded_len].to_vec(),
                serial,
                ogg::PacketWriteEndInfo::EndStream,
                granule_pos,
            );
        }
    } else {
        // Write end-of-stream marker with a silent frame
        let silent_pcm = vec![0i16; opus_frame_size * channels];
        if let Ok(encoded_len) = opus_encoder.encode(&silent_pcm, &mut opus_output) {
            granule_pos += opus_frame_size as u64;
            let _ = ogg_stream.write_packet(
                opus_output[..encoded_len].to_vec(),
                serial,
                ogg::PacketWriteEndInfo::EndStream,
                granule_pos,
            );
        }
    }

    let duration = start_time.elapsed();
    info!("{}", t!("recording_stopped", duration.as_secs_f64()));

    Ok(duration)
}

#[cfg(not(windows))]
fn capture_thread(
    _process_id: u32,
    _output_path: &std::path::Path,
    _stop_flag: &AtomicBool,
    _mic_config: &MicConfig,
) -> Result<Duration> {
    Err(anyhow!("Audio recording is only supported on Windows"))
}

/// Holds the mic capture resources
#[cfg(windows)]
struct MicCaptureState {
    client_wrapper: wasapi::AudioClient,
    capture_client: wasapi::AudioCaptureClient,
}

/// Set up WASAPI mic capture client
#[cfg(windows)]
fn setup_mic_capture(
    desired_format: &wasapi::WaveFormat,
    mic_config: &MicConfig,
) -> Result<MicCaptureState> {
    // Find the mic device
    let enumerator = wasapi::DeviceEnumerator::new()
        .map_err(|e| anyhow!("Failed to create DeviceEnumerator: {:?}", e))?;

    let device = if let Some(ref device_name) = mic_config.device_name {
        // Find device by name
        find_input_device_by_name(&enumerator, device_name)?
    } else {
        // Use default input device
        enumerator
            .get_default_device(&wasapi::Direction::Capture)
            .map_err(|e| anyhow!("Failed to get default input device: {:?}", e))?
    };

    let dev_name = device
        .get_friendlyname()
        .unwrap_or_else(|_| t!("unknown_device"));
    info!("{}", t!("using_mic_device", dev_name));

    let mut mic_client = device
        .get_iaudioclient()
        .map_err(|e| anyhow!("Failed to get mic AudioClient: {:?}", e))?;

    let mode = wasapi::StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: 0,
    };

    mic_client
        .initialize_client(desired_format, &wasapi::Direction::Capture, &mode)
        .map_err(|e| anyhow!("Failed to initialize mic client: {:?}", e))?;

    // Set event handle (needed for shared event mode)
    let _mic_event = mic_client
        .set_get_eventhandle()
        .map_err(|e| anyhow!("Failed to get mic event handle: {:?}", e))?;

    let mic_capture_client = mic_client
        .get_audiocaptureclient()
        .map_err(|e| anyhow!("Failed to get mic capture client: {:?}", e))?;

    Ok(MicCaptureState {
        client_wrapper: mic_client,
        capture_client: mic_capture_client,
    })
}

/// Find an input device by its friendly name
#[cfg(windows)]
fn find_input_device_by_name(
    enumerator: &wasapi::DeviceEnumerator,
    name: &str,
) -> Result<wasapi::Device> {
    let collection = enumerator
        .get_device_collection(&wasapi::Direction::Capture)
        .map_err(|e| anyhow!("Failed to enumerate capture devices: {:?}", e))?;

    let name_lower = name.to_lowercase();
    for device in &collection {
        let device = device.map_err(|e| anyhow!("Failed to get device: {:?}", e))?;
        if let Ok(friendly_name) = device.get_friendlyname() {
            if friendly_name.to_lowercase().contains(&name_lower) {
                info!("{}", t!("found_mic_device", friendly_name));
                return Ok(device);
            }
        }
    }

    Err(anyhow!("Input device containing \"{}\" not found", name))
}

/// Mix two PCM i16 sample buffers together with clamping to prevent overflow
#[cfg(windows)]
fn mix_pcm(a: &[i16], b: &[i16]) -> Vec<i16> {
    let len = a.len().max(b.len());
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let sa = if i < a.len() { a[i] as i32 } else { 0 };
        let sb = if i < b.len() { b[i] as i32 } else { 0 };
        let mixed = (sa + sb).clamp(i16::MIN as i32, i16::MAX as i32);
        out.push(mixed as i16);
    }
    out
}

/// Generate a pseudo-random serial number for OGG stream
#[cfg(windows)]
fn rand_serial() -> u32 {
    use std::time::SystemTime;
    let t = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    (t.as_nanos() & 0xFFFFFFFF) as u32
}

/// Write the required Opus header and comment packets to an OGG stream.
/// See https://www.rfc-editor.org/rfc/rfc7845#section-5
#[cfg(windows)]
fn write_opus_header(
    writer: &mut ogg::PacketWriter<&mut File>,
    serial: u32,
    sample_rate: u32,
    channels: u16,
) -> Result<()> {
    // OpusHead packet
    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead"); // Magic signature
    head.push(1); // Version
    head.push(channels as u8); // Channel count
    head.extend_from_slice(&0u16.to_le_bytes()); // Pre-skip
    head.extend_from_slice(&sample_rate.to_le_bytes()); // Input sample rate
    head.extend_from_slice(&0i16.to_le_bytes()); // Output gain
    head.push(0); // Channel mapping family

    writer
        .write_packet(head, serial, ogg::PacketWriteEndInfo::EndPage, 0)
        .map_err(|e| anyhow!("Failed to write OpusHead: {:?}", e))?;

    // OpusTags packet
    let vendor = b"vrmemoir";
    let mut tags = Vec::with_capacity(16 + vendor.len());
    tags.extend_from_slice(b"OpusTags");
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0u32.to_le_bytes()); // No user comments

    writer
        .write_packet(tags, serial, ogg::PacketWriteEndInfo::EndPage, 0)
        .map_err(|e| anyhow!("Failed to write OpusTags: {:?}", e))?;

    Ok(())
}
