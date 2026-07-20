// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use quady::{
    create_header_packet, create_rgb_packet, find_quadcast_device, generate_color_sequence,
    open_quadcast_device, send_control_transfer, ColorScheme, DataPacket, RgbColor, RgbMode,
    INTER_PACKET_DELAY_MS, MAX_COLPAIR_COUNT,
};
use rusb::{DeviceHandle, GlobalContext};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{
    AppHandle, CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem,
};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

/// Effect settings sent from the UI. Mirrors the `Effect` type in src/rgb.ts.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EffectConfig {
    mode: String,
    /// Colors as 0xRRGGBB values.
    colors: Vec<u32>,
    speed: u8,
    brightness: u8,
    /// "all" | "top" | "bottom"
    target: String,
}

struct RgbState {
    tx: Mutex<Sender<Vec<EffectConfig>>>,
    status: Arc<Mutex<String>>,
}

/// Queue the full effect list for the RGB worker thread. The worker resolves
/// which effect drives each LED group (last matching target wins).
#[tauri::command]
fn apply_effects(cfgs: Vec<EffectConfig>, state: tauri::State<RgbState>) -> Result<(), String> {
    state
        .tx
        .lock()
        .unwrap()
        .send(cfgs)
        .map_err(|e| e.to_string())
}

/// Current device status, e.g. "connected" or "device unavailable: ...".
#[tauri::command]
fn device_status(state: tauri::State<RgbState>) -> String {
    state.status.lock().unwrap().clone()
}

/// A preset entry surfaced in the menu-bar tray. Mirrors the UI `Preset` type
/// (only the fields the tray needs).
#[derive(Debug, Clone, serde::Deserialize)]
struct TrayPreset {
    id: String,
    name: String,
    /// Whether this preset matches the effects currently on the device.
    #[serde(default)]
    active: bool,
}

/// Prefix marking a tray menu item as a preset selection.
const TRAY_PRESET_PREFIX: &str = "preset::";

/// Build the tray menu from the current preset list, plus the standard
/// Show/Quit controls.
fn build_tray_menu(presets: &[TrayPreset]) -> SystemTrayMenu {
    let mut menu = SystemTrayMenu::new();
    if presets.is_empty() {
        menu = menu.add_item(CustomMenuItem::new("noop", "No presets saved").disabled());
    } else {
        for p in presets {
            let label = if p.active {
                format!("✓  {}", p.name)
            } else {
                format!("    {}", p.name)
            };
            menu = menu.add_item(CustomMenuItem::new(
                format!("{TRAY_PRESET_PREFIX}{}", p.id),
                label,
            ));
        }
    }
    menu.add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("show", "Show Quady"))
        .add_item(CustomMenuItem::new("quit", "Quit Quady"))
}

/// Rebuild the menu-bar tray so it lists the user's saved presets. Called from
/// the UI whenever presets change, so the tray stays in sync.
#[tauri::command]
fn set_tray_presets(app: AppHandle, presets: Vec<TrayPreset>) -> Result<(), String> {
    app.tray_handle()
        .set_menu(build_tray_menu(&presets))
        .map_err(|e| e.to_string())
}

/// Route tray menu clicks: preset items ask the UI to apply that preset, while
/// Show/Quit drive the window and app lifecycle.
fn on_tray_event(app: &AppHandle, event: SystemTrayEvent) {
    if let SystemTrayEvent::MenuItemClick { id, .. } = event {
        match id.as_str() {
            "quit" => app.exit(0),
            "show" => {
                if let Some(win) = app.get_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            other => {
                if let Some(preset_id) = other.strip_prefix(TRAY_PRESET_PREFIX) {
                    if let Some(win) = app.get_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                    let _ = app.emit_all("apply-preset", preset_id.to_string());
                }
            }
        }
    }
}

fn main() {
    let (tx, rx) = mpsc::channel::<Vec<EffectConfig>>();
    let status = Arc::new(Mutex::new("waiting for effect".to_string()));

    let worker_status = status.clone();
    std::thread::spawn(move || rgb_worker(rx, worker_status));

    tauri::Builder::default()
        .manage(RgbState {
            tx: Mutex::new(tx),
            status,
        })
        .system_tray(SystemTray::new().with_menu(build_tray_menu(&[])))
        .on_system_tray_event(on_tray_event)
        .invoke_handler(tauri::generate_handler![
            greet,
            apply_effects,
            device_status,
            set_tray_presets
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Generate the frame sequence for one effect, plus the frame offset the
/// lower LEDs use when this effect drives both groups (Wave phase shift,
/// Lightning top/bottom alternation).
fn build_led_sequence(cfg: &EffectConfig) -> Result<(Vec<RgbColor>, usize), String> {
    let mode =
        RgbMode::from_str(&cfg.mode).ok_or_else(|| format!("unknown mode: {}", cfg.mode))?;
    let colors: Vec<RgbColor> = cfg
        .colors
        .iter()
        .map(|&c| RgbColor::from_u32(c))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let mut scheme = ColorScheme::new(mode)
        .with_brightness(cfg.brightness)
        .with_speed(cfg.speed);
    if !colors.is_empty() {
        scheme = scheme.with_colors(colors.clone());
    }

    let seq = generate_color_sequence(&mut scheme).map_err(|e| e.to_string())?;
    if seq.is_empty() {
        return Err("mode generated an empty color sequence".to_string());
    }

    let offset = match mode {
        RgbMode::Wave => seq.len() / colors.len().max(1),
        RgbMode::Lightning => seq.len() / 2,
        _ => 0,
    };
    Ok((seq, offset))
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Turn the effect list into the packet cycle to stream to the device.
///
/// Each LED group is driven by the last effect whose `target` includes it, so
/// one effect can color the top ring while another colors the bottom. Upper
/// and lower frames are interleaved per packet; untargeted groups stay black.
fn build_packets(cfgs: &[EffectConfig]) -> Result<Vec<DataPacket>, String> {
    let black = RgbColor::new(0, 0, 0);

    let top_cfg = cfgs.iter().rev().find(|c| c.target != "bottom");
    let bottom_cfg = cfgs.iter().rev().find(|c| c.target != "top");
    let top = top_cfg.map(build_led_sequence).transpose()?;
    let bottom = bottom_cfg.map(build_led_sequence).transpose()?;

    if top.is_none() && bottom.is_none() {
        // Nothing targeted: blank both LED groups.
        return Ok(vec![create_rgb_packet(&[black; 16])]);
    }

    // Loop long enough for both sequences to wrap cleanly, within the
    // device's sequence budget.
    let top_len = top.as_ref().map_or(1, |(s, _)| s.len());
    let bottom_len = bottom.as_ref().map_or(1, |(s, _)| s.len());
    let total = (top_len / gcd(top_len, bottom_len) * bottom_len).min(MAX_COLPAIR_COUNT);

    let chunk_size = 8; // 8 upper/lower pairs = 16 colors per 64-byte packet
    let mut packets = Vec::new();
    for chunk_start in (0..total).step_by(chunk_size) {
        let mut interleaved = Vec::with_capacity(chunk_size * 2);
        for i in chunk_start..(chunk_start + chunk_size).min(total) {
            let upper = top.as_ref().map_or(black, |(s, _)| s[i % s.len()]);
            let lower = bottom
                .as_ref()
                .map_or(black, |(s, off)| s[(i + off) % s.len()]);
            interleaved.push(upper);
            interleaved.push(lower);
        }
        packets.push(create_rgb_packet(&interleaved));
    }
    Ok(packets)
}

/// Background thread: owns the USB handle, receives effects from the UI and
/// continuously streams the current packet cycle to the device.
fn rgb_worker(rx: Receiver<Vec<EffectConfig>>, status: Arc<Mutex<String>>) {
    let set_status = |s: String| *status.lock().unwrap() = s;
    let header = create_header_packet();
    let mut handle: Option<DeviceHandle<GlobalContext>> = None;
    let mut packets: Vec<DataPacket> = Vec::new();
    let mut pending: Option<Vec<EffectConfig>> = None;

    loop {
        // Collapse any queued configs down to the most recent one.
        loop {
            match rx.try_recv() {
                Ok(cfg) => pending = Some(cfg),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }

        if let Some(cfgs) = pending.take() {
            match build_packets(&cfgs) {
                Ok(p) => packets = p,
                Err(e) => set_status(format!("invalid effect: {e}")),
            }
        }

        // Nothing to display yet: block until the UI sends an effect.
        if packets.is_empty() {
            match rx.recv() {
                Ok(cfg) => {
                    pending = Some(cfg);
                    continue;
                }
                Err(_) => return,
            }
        }

        if handle.is_none() {
            match find_quadcast_device().and_then(open_quadcast_device) {
                Ok(h) => {
                    set_status("connected".to_string());
                    handle = Some(h);
                }
                Err(e) => {
                    set_status(format!("device unavailable: {e}"));
                    // Retry in a bit, picking up config changes meanwhile.
                    match rx.recv_timeout(Duration::from_secs(2)) {
                        Ok(cfg) => pending = Some(cfg),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                    continue;
                }
            }
        }

        // Stream one full cycle, bailing out early when a new effect arrives.
        let h = handle.as_ref().unwrap();
        let mut send_failed = false;
        'cycle: for rgb_packet in &packets {
            match rx.try_recv() {
                Ok(cfg) => {
                    pending = Some(cfg);
                    break 'cycle;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => return,
            }
            for packet in [&header, rgb_packet] {
                if let Err(e) = send_control_transfer(h, packet) {
                    set_status(format!("send failed: {e}"));
                    send_failed = true;
                    break 'cycle;
                }
                std::thread::sleep(Duration::from_millis(INTER_PACKET_DELAY_MS));
            }
        }
        if send_failed {
            // Drop the handle and reconnect on the next iteration.
            handle = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quady::{find_quadcast_device, open_quadcast_device, ColorScheme, RgbMode};

    // ===== Hardware Integration Tests =====
    // These tests require a physical QuadCast device connected

    #[test]
    #[ignore] // Requires physical device AND proper USB permissions
    fn test_find_and_open_device() {
        // Note: This test requires:
        // 1. Physical QuadCast device connected
        // 2. Proper USB permissions (may need sudo on Linux/macOS)
        // Run with: sudo -E cargo test --bin quady test_find_and_open_device -- --ignored --nocapture

        match find_quadcast_device() {
            Ok(device) => {
                println!("✓ Found QuadCast device");
                match open_quadcast_device(device) {
                    Ok(handle) => {
                        println!("✓ Successfully opened and claimed device interfaces");
                        drop(handle);
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to open device: {}", e);
                        eprintln!(
                            "  Hint: You may need to run with sudo or adjust USB permissions"
                        );
                        panic!("Device open failed");
                    }
                }
            }
            Err(e) => {
                eprintln!("✗ No QuadCast device found: {}", e);
                eprintln!("  Hint: Make sure the device is connected and detected by the system");

                #[cfg(target_os = "macos")]
                eprintln!("  Run 'system_profiler SPUSBDataType | grep -i hyperx' to check");

                #[cfg(target_os = "linux")]
                eprintln!("  Run 'lsusb | grep -i hyperx' to check");

                panic!("Device not found");
            }
        }
    }

    // ===== Unit Tests for main.rs Logic =====

    #[test]
    fn test_packet_generation_for_solid_mode() {
        // Test that we can generate packets without errors
        let mode = RgbMode::Solid;
        let mut scheme = ColorScheme::new(mode)
            .with_colors(vec![RgbColor::new(255, 0, 0)])
            .with_brightness(100);

        let color_sequence = generate_color_sequence(&mut scheme).unwrap();
        assert_eq!(
            color_sequence.len(),
            1,
            "Solid mode should generate 1 frame"
        );

        // Build packets like run_rgb_mode does
        let mut packets = Vec::new();
        let chunk_size = 8;

        for chunk in color_sequence.chunks(chunk_size) {
            let mut interleaved = Vec::new();
            for color in chunk {
                interleaved.push(*color);
                interleaved.push(*color);
            }
            packets.push(create_rgb_packet(&interleaved));
        }

        assert_eq!(packets.len(), 1, "Should create 1 packet for solid mode");
        assert_eq!(packets[0].len(), 64, "Packet should be 64 bytes");
    }

    #[test]
    fn test_packet_generation_for_cycle_mode() {
        let mode = RgbMode::Cycle;
        let mut scheme = ColorScheme::new(mode).with_brightness(100).with_speed(50);

        let color_sequence = generate_color_sequence(&mut scheme).unwrap();
        assert!(
            color_sequence.len() > 1,
            "Cycle mode should generate multiple frames"
        );

        // Build packets
        let mut packets = Vec::new();
        let chunk_size = 8;

        for chunk in color_sequence.chunks(chunk_size) {
            let mut interleaved = Vec::new();
            for color in chunk {
                interleaved.push(*color);
                interleaved.push(*color);
            }
            packets.push(create_rgb_packet(&interleaved));
        }

        assert!(
            packets.len() > 0,
            "Should create at least one packet for cycle mode"
        );

        // Verify all packets are valid size
        for packet in &packets {
            assert_eq!(packet.len(), 64, "All packets should be 64 bytes");
        }
    }

    #[test]
    fn test_interleaved_color_packing() {
        // Test the interleaving logic for upper/lower LEDs
        let colors = vec![RgbColor::new(255, 0, 0), RgbColor::new(0, 255, 0)];

        let mut interleaved = Vec::new();
        for color in &colors {
            interleaved.push(*color); // Upper LED
            interleaved.push(*color); // Lower LED
        }

        assert_eq!(interleaved.len(), 4);
        assert_eq!(interleaved[0], RgbColor::new(255, 0, 0)); // Upper red
        assert_eq!(interleaved[1], RgbColor::new(255, 0, 0)); // Lower red
        assert_eq!(interleaved[2], RgbColor::new(0, 255, 0)); // Upper green
        assert_eq!(interleaved[3], RgbColor::new(0, 255, 0)); // Lower green
    }

    #[test]
    fn test_all_modes_generate_valid_packets() {
        let modes = [
            RgbMode::Solid,
            RgbMode::Blink,
            RgbMode::Cycle,
            RgbMode::Wave,
            RgbMode::Lightning,
            RgbMode::Pulse,
        ];

        for mode in modes {
            let mut scheme = ColorScheme::new(mode).with_brightness(100).with_speed(50);

            let color_sequence = generate_color_sequence(&mut scheme).unwrap();
            assert!(
                color_sequence.len() > 0,
                "Mode {:?} should generate at least one color",
                mode
            );

            // Build packets
            let mut packets = Vec::new();
            let chunk_size = 8;

            for chunk in color_sequence.chunks(chunk_size) {
                let mut interleaved = Vec::new();
                for color in chunk {
                    interleaved.push(*color);
                    interleaved.push(*color);
                }
                packets.push(create_rgb_packet(&interleaved));
            }

            assert!(
                packets.len() > 0,
                "Mode {:?} should create at least one packet",
                mode
            );

            // Verify packet structure
            for packet in &packets {
                assert_eq!(
                    packet.len(),
                    64,
                    "Mode {:?} packets should be 64 bytes",
                    mode
                );
            }
        }
    }

    #[test]
    fn test_header_packet_creation() {
        let header = create_header_packet();

        assert_eq!(header.len(), 64);
        assert_eq!(header[0], 0x04);
        assert_eq!(header[1], 0xf2);
        assert_eq!(header[8], 0x01);
    }

    #[test]
    fn test_greet_command() {
        let result = greet("World");
        assert_eq!(result, "Hello, World! You've been greeted from Rust!");
    }

    #[test]
    fn test_custom_colors_packet_generation() {
        let mode = RgbMode::Solid;
        let custom_color = RgbColor::new(128, 64, 192); // Purple

        let mut scheme = ColorScheme::new(mode)
            .with_colors(vec![custom_color])
            .with_brightness(100);

        let color_sequence = generate_color_sequence(&mut scheme).unwrap();

        assert_eq!(color_sequence.len(), 1);
        assert_eq!(color_sequence[0], custom_color);

        // Verify packet contains the color
        let interleaved = vec![custom_color, custom_color];
        let packet = create_rgb_packet(&interleaved);

        // Check RGB values are in the packet
        assert_eq!(packet[0], 0x81); // RGB_CODE
        assert_eq!(packet[1], 128); // R
        assert_eq!(packet[2], 64); // G
        assert_eq!(packet[3], 192); // B
    }
}
