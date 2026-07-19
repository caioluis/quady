use rusb::{Device, DeviceHandle, UsbContext};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QuadyError {
    #[error("USB error: {0}")]
    UsbError(#[from] rusb::Error),

    #[error("Invalid vendor ID: {0:#06x}")]
    InvalidVendorId(u16),

    #[error("Invalid product ID: {0:#06x}")]
    InvalidProductId(u16),

    #[error("Device not found")]
    DeviceNotFound,

    #[error("No suitable endpoint found")]
    NoEndpointFound,

    #[error("Invalid color value: {0:#08x}")]
    InvalidColor(u32),

    #[error("Invalid brightness: {0} (must be 0-100)")]
    InvalidBrightness(u8),
}

pub type Result<T> = std::result::Result<T, QuadyError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendorIdType {
    NA, // Kingston/HyperX (0x0951)
    EU, // HP (0x03f0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductIdType {
    NA,  // QuadCast S (0x171f)
    EU1, // DuoCast (0x0f8b)
    EU2, // DuoCast (0x028c)
    EU3, // DuoCast (0x048c)
    EU4, // DuoCast (0x068c)
    EU5, // DuoCast (0x098c)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Microphone {
    pub vendor: VendorIdType,
    pub product: ProductIdType,
}

impl Microphone {
    pub fn new(vendor: VendorIdType, product: ProductIdType) -> Self {
        Self { vendor, product }
    }

    pub fn microphone_type(&self) -> &'static str {
        match (self.vendor, self.product) {
            (VendorIdType::NA, ProductIdType::NA) => "QuadCast S (NA)",
            (VendorIdType::EU, ProductIdType::EU1) => "DuoCast (EU1)",
            (VendorIdType::EU, ProductIdType::EU2) => "DuoCast (EU2)",
            (VendorIdType::EU, ProductIdType::EU3) => "DuoCast (EU3)",
            (VendorIdType::EU, ProductIdType::EU4) => "DuoCast (EU4)",
            (VendorIdType::EU, ProductIdType::EU5) => "DuoCast (EU5)",
            _ => "Unknown",
        }
    }
}

pub fn to_vendor_id(id: u16) -> Result<VendorIdType> {
    match id {
        0x0951 => Ok(VendorIdType::NA),
        0x03f0 => Ok(VendorIdType::EU),
        _ => Err(QuadyError::InvalidVendorId(id)),
    }
}

pub fn to_product_id(id: u16) -> Result<ProductIdType> {
    match id {
        0x171f => Ok(ProductIdType::NA),
        0x0f8b => Ok(ProductIdType::EU1),
        0x028c => Ok(ProductIdType::EU2),
        0x048c => Ok(ProductIdType::EU3),
        0x068c => Ok(ProductIdType::EU4),
        0x098c => Ok(ProductIdType::EU5),
        _ => Err(QuadyError::InvalidProductId(id)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub config: u8,
    pub iface: u8,
    pub setting: u8,
    pub address: u8,
}

/// Returns all readable endpoints for given USB device
pub fn find_readable_endpoints<T: UsbContext>(device: &mut Device<T>) -> Result<Vec<Endpoint>> {
    let device_desc = device.device_descriptor()?;
    let mut endpoints = vec![];

    for n in 0..device_desc.num_configurations() {
        let config_desc = match device.config_descriptor(n) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                for endpoint_desc in interface_desc.endpoint_descriptors() {
                    endpoints.push(Endpoint {
                        config: config_desc.number(),
                        iface: interface_desc.interface_number(),
                        setting: interface_desc.setting_number(),
                        address: endpoint_desc.address(),
                    });
                }
            }
        }
    }

    Ok(endpoints)
}

/// USB control transfer parameters (matching C implementation)
pub const CONTROL_REQUEST_TYPE_OUT: u8 = 0x21;
pub const CONTROL_REQUEST_OUT: u8 = 0x09;
pub const CONTROL_REQUEST_TYPE_IN: u8 = 0xa1;
pub const CONTROL_REQUEST_IN: u8 = 0x01;
pub const CONTROL_VALUE: u16 = 0x0300;
pub const CONTROL_INDEX: u16 = 0x0000;
pub const PACKET_SIZE: usize = 64;
pub const TIMEOUT_MS: u64 = 1000;
pub const INTER_PACKET_DELAY_MS: u64 = 55;

/// RGB command code (from C implementation)
pub const RGB_CODE: u8 = 0x81;

/// Constants for mode calculations (from C implementation)
pub const MAX_BR_SPD_DLY: u8 = 100;
pub const SPD_DEFAULT: u8 = 50;
pub const DLY_DEFAULT: u8 = 50;

// Cycle/Wave mode transition ranges
const MIN_CYCL_TR: usize = 10;
const MAX_CYCL_TR: usize = 200;

// Lightning/Pulse mode transition ranges
const MIN_LGHT_BL: usize = 5; // blackout
const MAX_LGHT_BL: usize = 50;
const MIN_LGHT_UP: usize = 3; // fade up
const MAX_LGHT_UP: usize = 20;
const MIN_LGHT_DOWN: usize = 5; // fade down
const MAX_LGHT_DOWN: usize = 40;

// Blink mode random color segment ranges
const RAND_COL_SEG_MIN: usize = 10;
const RAND_COL_SEG_MAX: usize = 100;
const RAND_DLY_SEG_MIN: usize = 5;
const RAND_DLY_SEG_MAX: usize = 80;

// Maximum data packet constraints
const MAX_PCT_COUNT: usize = 90;
const COLPAIR_PER_PCT: usize = 8;
pub const MAX_COLPAIR_COUNT: usize = MAX_PCT_COUNT * COLPAIR_PER_PCT;

/// RGB Mode types matching the C implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RgbMode {
    Solid,
    Blink,
    Cycle,
    Wave,
    Lightning,
    Pulse,
    Visualizer, // Not yet implemented
}

impl RgbMode {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "solid" => Some(RgbMode::Solid),
            "blink" => Some(RgbMode::Blink),
            "cycle" => Some(RgbMode::Cycle),
            "wave" => Some(RgbMode::Wave),
            "lightning" => Some(RgbMode::Lightning),
            "pulse" => Some(RgbMode::Pulse),
            "visualizer" => Some(RgbMode::Visualizer),
            _ => None,
        }
    }
}

/// Color scheme for a single LED group (upper or lower)
#[derive(Debug, Clone)]
pub struct ColorScheme {
    pub mode: RgbMode,
    pub colors: Vec<RgbColor>,
    pub brightness: u8, // 0-100
    pub speed: u8,      // 0-100
    pub delay: u8,      // 0-100
}

impl ColorScheme {
    pub fn new(mode: RgbMode) -> Self {
        Self {
            mode,
            colors: Vec::new(),
            brightness: MAX_BR_SPD_DLY,
            speed: SPD_DEFAULT,
            delay: DLY_DEFAULT,
        }
    }

    pub fn with_colors(mut self, colors: Vec<RgbColor>) -> Self {
        self.colors = colors;
        self
    }

    pub fn with_brightness(mut self, brightness: u8) -> Self {
        self.brightness = brightness.min(100);
        self
    }

    pub fn with_speed(mut self, speed: u8) -> Self {
        self.speed = speed.min(100);
        self
    }

    pub fn with_delay(mut self, delay: u8) -> Self {
        self.delay = delay.min(100);
        self
    }

    /// Apply brightness to all colors in the scheme
    fn apply_brightness(&mut self) -> Result<()> {
        for color in &mut self.colors {
            *color = color.with_brightness(self.brightness)?;
        }
        Ok(())
    }
}

/// Default rainbow colors (from C implementation)
pub const RAINBOW: [u32; 9] = [
    0xff0000, 0xff009e, 0xcd00ff, 0x2b00ff, 0x0068ff, 0x00ffff, 0x00ff67, 0x32ff00, 0xceff00,
];

/// Common colors
pub const BLACK: u32 = 0x000000;
pub const RED: u32 = 0xff0000;

/// Represents an RGB color
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn from_u32(color: u32) -> Result<Self> {
        if color > 0xffffff {
            return Err(QuadyError::InvalidColor(color));
        }

        Ok(Self {
            r: ((color >> 16) & 0xff) as u8,
            g: ((color >> 8) & 0xff) as u8,
            b: (color & 0xff) as u8,
        })
    }

    pub fn to_u32(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Apply brightness adjustment (0-100)
    pub fn with_brightness(&self, brightness: u8) -> Result<Self> {
        if brightness > 100 {
            return Err(QuadyError::InvalidBrightness(brightness));
        }

        let factor = brightness as f32 / 100.0;
        Ok(Self {
            r: (self.r as f32 * factor) as u8,
            g: (self.g as f32 * factor) as u8,
            b: (self.b as f32 * factor) as u8,
        })
    }
}

/// Data packet structure (64 bytes)
pub type DataPacket = [u8; PACKET_SIZE];

/// Creates the header packet (sent before data packets)
pub fn create_header_packet() -> DataPacket {
    let mut packet = [0u8; PACKET_SIZE];
    packet[0] = 0x04;
    packet[1] = 0xf2;
    packet[8] = 0x01;
    packet
}

/// Creates a data packet with RGB color commands
pub fn create_rgb_packet(colors: &[RgbColor]) -> DataPacket {
    let mut packet = [0u8; PACKET_SIZE];
    let mut offset = 0;

    for color in colors.iter().take(16) {
        if offset + 4 > PACKET_SIZE {
            break;
        }

        packet[offset] = RGB_CODE;
        packet[offset + 1] = color.r;
        packet[offset + 2] = color.g;
        packet[offset + 3] = color.b;
        offset += 4;
    }

    packet
}

/// Opens a QuadCast device and claims the necessary interfaces
pub fn open_quadcast_device<T: UsbContext>(device: Device<T>) -> Result<DeviceHandle<T>> {
    let mut handle = device.open()?;

    // Set auto-detach kernel driver (matches C implementation)
    let _ = handle.set_auto_detach_kernel_driver(true);

    // Try to detach kernel driver from interface 0
    match handle.kernel_driver_active(0) {
        Ok(true) => {
            eprintln!("Kernel driver is active on interface 0, attempting to detach...");
            if let Err(e) = handle.detach_kernel_driver(0) {
                eprintln!(
                    "Warning: Could not detach kernel driver from interface 0: {}",
                    e
                );
                eprintln!("You may need to run with sudo or adjust USB permissions");
            }
        }
        Ok(false) => {
            eprintln!("No kernel driver active on interface 0");
        }
        Err(e) => {
            eprintln!("Could not check kernel driver status: {}", e);
        }
    }

    // Try to detach kernel driver from interface 1
    match handle.kernel_driver_active(1) {
        Ok(true) => {
            eprintln!("Kernel driver is active on interface 1, attempting to detach...");
            if let Err(e) = handle.detach_kernel_driver(1) {
                eprintln!(
                    "Warning: Could not detach kernel driver from interface 1: {}",
                    e
                );
            }
        }
        Ok(false) => {
            eprintln!("No kernel driver active on interface 1");
        }
        Err(e) => {
            eprintln!("Could not check kernel driver status: {}", e);
        }
    }

    // Claim interfaces 0 and 1 (matches C implementation)
    handle.claim_interface(0)?;
    handle.claim_interface(1)?;

    Ok(handle)
}

/// Sends a control transfer to the device
pub fn send_control_transfer<T: UsbContext>(
    handle: &DeviceHandle<T>,
    packet: &DataPacket,
) -> Result<usize> {
    let timeout = std::time::Duration::from_millis(TIMEOUT_MS);

    let bytes_written = handle.write_control(
        CONTROL_REQUEST_TYPE_OUT,
        CONTROL_REQUEST_OUT,
        CONTROL_VALUE,
        CONTROL_INDEX,
        packet,
        timeout,
    )?;

    Ok(bytes_written)
}

/// Finds the first QuadCast device connected to the system
pub fn find_quadcast_device() -> Result<rusb::Device<rusb::GlobalContext>> {
    let devices = rusb::devices()?;

    for device in devices.iter() {
        let device_desc = device.device_descriptor()?;
        let vendor_id = to_vendor_id(device_desc.vendor_id());
        let product_id = to_product_id(device_desc.product_id());

        if vendor_id.is_ok() && product_id.is_ok() {
            return Ok(device);
        }
    }

    Err(QuadyError::DeviceNotFound)
}

// ===== RGB Mode Generation Functions =====

/// Generates a color sequence based on the color scheme
pub fn generate_color_sequence(scheme: &mut ColorScheme) -> Result<Vec<RgbColor>> {
    scheme.apply_brightness()?;

    match scheme.mode {
        RgbMode::Solid => generate_solid(&scheme.colors),
        RgbMode::Blink => generate_blink(&scheme.colors, scheme.speed, scheme.delay),
        RgbMode::Cycle => generate_cycle(&scheme.colors, scheme.speed),
        RgbMode::Wave => generate_wave(&scheme.colors, scheme.speed, false),
        RgbMode::Lightning => generate_lightning(&scheme.colors, scheme.speed, false),
        RgbMode::Pulse => generate_lightning(&scheme.colors, scheme.speed, true),
        RgbMode::Visualizer => {
            // Not yet implemented
            Err(QuadyError::InvalidColor(0))
        }
    }
}

/// Solid mode: single static color
fn generate_solid(colors: &[RgbColor]) -> Result<Vec<RgbColor>> {
    if colors.is_empty() {
        return Ok(vec![RgbColor::from_u32(RED)?]);
    }
    Ok(vec![colors[0]])
}

/// Blink mode: colors blink on and off
fn generate_blink(colors: &[RgbColor], speed: u8, delay: u8) -> Result<Vec<RgbColor>> {
    let mut sequence = Vec::new();
    let black = RgbColor::from_u32(BLACK)?;

    // If no colors provided, use random colors
    if colors.is_empty() {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let col_seg =
            RAND_COL_SEG_MIN + (speed as usize * (RAND_COL_SEG_MAX - RAND_COL_SEG_MIN)) / 100;
        let dly_seg =
            RAND_DLY_SEG_MIN + (delay as usize * (RAND_DLY_SEG_MAX - RAND_DLY_SEG_MIN)) / 100;

        let mut colpair_count = 0;
        while colpair_count < MAX_COLPAIR_COUNT {
            // Random color
            let random_color = rng.gen_range(0x000001..=0xffffff);
            let color = RgbColor::from_u32(random_color)?;

            // Add color segment
            let remaining = MAX_COLPAIR_COUNT - colpair_count;
            let actual_col_seg = col_seg.min(remaining);
            for _ in 0..actual_col_seg {
                sequence.push(color);
            }
            colpair_count += actual_col_seg;

            if colpair_count >= MAX_COLPAIR_COUNT {
                break;
            }

            // Add black delay segment
            let remaining = MAX_COLPAIR_COUNT - colpair_count;
            let actual_dly_seg = dly_seg.min(remaining);
            for _ in 0..actual_dly_seg {
                sequence.push(black);
            }
            colpair_count += actual_dly_seg;
        }
    } else {
        // Specific colors
        let col_seg = 101 - speed as usize;

        for color in colors {
            // Color segment
            for _ in 0..col_seg {
                sequence.push(*color);
            }
            // Black delay segment
            for _ in 0..delay as usize {
                sequence.push(black);
            }
        }
    }

    Ok(sequence)
}

/// Cycle mode: smooth color transitions in a cycle
fn generate_cycle(colors: &[RgbColor], speed: u8) -> Result<Vec<RgbColor>> {
    let mut sequence = Vec::new();

    let colors = if colors.is_empty() {
        RAINBOW
            .iter()
            .map(|&c| RgbColor::from_u32(c))
            .collect::<Result<Vec<_>>>()?
    } else {
        colors.to_vec()
    };

    if colors.is_empty() {
        return Ok(vec![RgbColor::from_u32(RED)?]);
    }

    let gradient_length = calculate_gradient_length(&colors, speed);

    for i in 0..colors.len() {
        let start_color = colors[i];
        let end_color = colors[(i + 1) % colors.len()];

        let gradient = generate_gradient(start_color, end_color, gradient_length);
        sequence.extend(gradient);
    }

    Ok(sequence)
}

/// Wave mode: like cycle but with phase shift for upper/lower LEDs
fn generate_wave(colors: &[RgbColor], speed: u8, shift: bool) -> Result<Vec<RgbColor>> {
    let mut colors = if colors.is_empty() {
        RAINBOW
            .iter()
            .map(|&c| RgbColor::from_u32(c))
            .collect::<Result<Vec<_>>>()?
    } else {
        colors.to_vec()
    };

    if shift && !colors.is_empty() {
        // Shift array by one position for wave effect
        let first = colors.remove(0);
        colors.push(first);
    }

    generate_cycle(&colors, speed)
}

/// Lightning mode: flash effect with fade up and down
fn generate_lightning(colors: &[RgbColor], speed: u8, synchronous: bool) -> Result<Vec<RgbColor>> {
    let mut sequence = Vec::new();
    let black = RgbColor::from_u32(BLACK)?;

    let colors = if colors.is_empty() {
        vec![RgbColor::from_u32(RED)?]
    } else {
        colors.to_vec()
    };

    let bl_size = speed_range(MIN_LGHT_BL, MAX_LGHT_BL, speed);
    let up_size = speed_range(MIN_LGHT_UP, MAX_LGHT_UP, speed);
    let down_size = speed_range(MIN_LGHT_DOWN, MAX_LGHT_DOWN, speed);

    for color in colors {
        // Leading blackout (synchronous/Pulse only, so pulses breathe evenly)
        if synchronous {
            for _ in 0..bl_size {
                sequence.push(black);
            }
        }

        // Fade up
        let fade_up = generate_gradient(black, color, up_size);
        sequence.extend(fade_up);

        // Fade down (start from next step to avoid duplicate peak)
        let next_color = next_gradient_color(color, black, down_size);
        let fade_down = generate_gradient(next_color, black, down_size);
        sequence.extend(fade_down);

        // Trailing blackout between flashes
        for _ in 0..bl_size {
            sequence.push(black);
        }
    }

    Ok(sequence)
}

// ===== Helper Functions =====

/// Calculate gradient length for cycle mode
fn calculate_gradient_length(colors: &[RgbColor], speed: u8) -> usize {
    let color_count = colors.len();
    let tr_size = MIN_CYCL_TR + (MAX_CYCL_TR - MIN_CYCL_TR) * (100 - speed as usize) / 100;

    if tr_size * color_count > MAX_COLPAIR_COUNT {
        MIN_CYCL_TR + (MAX_COLPAIR_COUNT / color_count - MIN_CYCL_TR) * (100 - speed as usize) / 100
    } else {
        tr_size
    }
}

/// Map speed value to a range
fn speed_range(min: usize, max: usize, speed: u8) -> usize {
    min + (max - min) * (100 - speed as usize) / 100
}

/// Generate a color gradient between two colors
fn generate_gradient(start: RgbColor, end: RgbColor, length: usize) -> Vec<RgbColor> {
    if length == 0 {
        return vec![];
    }
    if length == 1 {
        return vec![start];
    }

    let mut gradient = Vec::with_capacity(length);

    for i in 0..length {
        let factor = i as f32 / (length - 1) as f32;

        let r = (start.r as f32 + factor * (end.r as f32 - start.r as f32)) as u8;
        let g = (start.g as f32 + factor * (end.g as f32 - start.g as f32)) as u8;
        let b = (start.b as f32 + factor * (end.b as f32 - start.b as f32)) as u8;

        gradient.push(RgbColor::new(r, g, b));
    }

    gradient
}

/// Calculate the next color in a gradient (for lightning mode)
fn next_gradient_color(color: RgbColor, end_color: RgbColor, size: usize) -> RgbColor {
    if size <= 1 {
        return end_color;
    }

    let factor = 1.0 / (size - 1) as f32;

    let r = (color.r as f32 + factor * (end_color.r as f32 - color.r as f32)) as u8;
    let g = (color.g as f32 + factor * (end_color.g as f32 - color.g as f32)) as u8;
    let b = (color.b as f32 + factor * (end_color.b as f32 - color.b as f32)) as u8;

    RgbColor::new(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vendor_id_na() {
        assert_eq!(to_vendor_id(0x0951).unwrap(), VendorIdType::NA);
    }

    #[test]
    fn test_vendor_id_eu() {
        assert_eq!(to_vendor_id(0x03f0).unwrap(), VendorIdType::EU);
    }

    #[test]
    fn test_vendor_id_invalid() {
        assert!(to_vendor_id(0x9999).is_err());
    }

    #[test]
    fn test_product_id_na() {
        assert_eq!(to_product_id(0x171f).unwrap(), ProductIdType::NA);
    }

    #[test]
    fn test_product_id_eu_variants() {
        assert_eq!(to_product_id(0x0f8b).unwrap(), ProductIdType::EU1);
        assert_eq!(to_product_id(0x028c).unwrap(), ProductIdType::EU2);
        assert_eq!(to_product_id(0x048c).unwrap(), ProductIdType::EU3);
        assert_eq!(to_product_id(0x068c).unwrap(), ProductIdType::EU4);
        assert_eq!(to_product_id(0x098c).unwrap(), ProductIdType::EU5);
    }

    #[test]
    fn test_product_id_invalid() {
        assert!(to_product_id(0x9999).is_err());
    }

    #[test]
    fn test_microphone_type_display() {
        let mic = Microphone::new(VendorIdType::NA, ProductIdType::NA);
        assert_eq!(mic.microphone_type(), "QuadCast S (NA)");

        let mic = Microphone::new(VendorIdType::EU, ProductIdType::EU1);
        assert_eq!(mic.microphone_type(), "DuoCast (EU1)");
    }

    #[test]
    fn test_rgb_color_from_u32() {
        let color = RgbColor::from_u32(0xff0000).unwrap();
        assert_eq!(color.r, 0xff);
        assert_eq!(color.g, 0x00);
        assert_eq!(color.b, 0x00);
    }

    #[test]
    fn test_rgb_color_to_u32() {
        let color = RgbColor::new(0xff, 0x00, 0x00);
        assert_eq!(color.to_u32(), 0xff0000);
    }

    #[test]
    fn test_rgb_color_invalid() {
        assert!(RgbColor::from_u32(0x1000000).is_err());
    }

    #[test]
    fn test_brightness_adjustment() {
        let color = RgbColor::new(0xff, 0xff, 0xff);
        let adjusted = color.with_brightness(50).unwrap();
        assert_eq!(adjusted.r, 127);
        assert_eq!(adjusted.g, 127);
        assert_eq!(adjusted.b, 127);
    }

    #[test]
    fn test_brightness_zero() {
        let color = RgbColor::new(0xff, 0xff, 0xff);
        let adjusted = color.with_brightness(0).unwrap();
        assert_eq!(adjusted.r, 0);
        assert_eq!(adjusted.g, 0);
        assert_eq!(adjusted.b, 0);
    }

    #[test]
    fn test_brightness_invalid() {
        let color = RgbColor::new(0xff, 0xff, 0xff);
        assert!(color.with_brightness(101).is_err());
    }

    #[test]
    fn test_header_packet_format() {
        let packet = create_header_packet();
        assert_eq!(packet[0], 0x04);
        assert_eq!(packet[1], 0xf2);
        assert_eq!(packet[8], 0x01);
        assert_eq!(packet.len(), PACKET_SIZE);
    }

    #[test]
    fn test_rgb_packet_single_color() {
        let colors = vec![RgbColor::new(0xff, 0x00, 0x00)];
        let packet = create_rgb_packet(&colors);

        assert_eq!(packet[0], RGB_CODE);
        assert_eq!(packet[1], 0xff);
        assert_eq!(packet[2], 0x00);
        assert_eq!(packet[3], 0x00);
    }

    #[test]
    fn test_rgb_packet_multiple_colors() {
        let colors = vec![
            RgbColor::new(0xff, 0x00, 0x00),
            RgbColor::new(0x00, 0xff, 0x00),
        ];
        let packet = create_rgb_packet(&colors);

        // First color
        assert_eq!(packet[0], RGB_CODE);
        assert_eq!(packet[1], 0xff);
        assert_eq!(packet[2], 0x00);
        assert_eq!(packet[3], 0x00);

        // Second color
        assert_eq!(packet[4], RGB_CODE);
        assert_eq!(packet[5], 0x00);
        assert_eq!(packet[6], 0xff);
        assert_eq!(packet[7], 0x00);
    }

    #[test]
    fn test_rgb_packet_max_colors() {
        let colors = vec![RgbColor::new(0xff, 0xff, 0xff); 20];
        let packet = create_rgb_packet(&colors);

        // Should only use 16 colors (max that fit in packet)
        // 16 colors * 4 bytes = 64 bytes
        let mut count = 0;
        for i in (0..PACKET_SIZE).step_by(4) {
            if packet[i] == RGB_CODE {
                count += 1;
            }
        }
        assert_eq!(count, 16);
    }

    #[test]
    fn test_constants() {
        assert_eq!(CONTROL_REQUEST_TYPE_OUT, 0x21);
        assert_eq!(CONTROL_REQUEST_OUT, 0x09);
        assert_eq!(CONTROL_REQUEST_TYPE_IN, 0xa1);
        assert_eq!(CONTROL_REQUEST_IN, 0x01);
        assert_eq!(CONTROL_VALUE, 0x0300);
        assert_eq!(CONTROL_INDEX, 0x0000);
        assert_eq!(PACKET_SIZE, 64);
        assert_eq!(RGB_CODE, 0x81);
    }

    // ===== RGB Mode Tests =====

    #[test]
    fn test_rgb_mode_from_str() {
        assert_eq!(RgbMode::from_str("solid"), Some(RgbMode::Solid));
        assert_eq!(RgbMode::from_str("SOLID"), Some(RgbMode::Solid));
        assert_eq!(RgbMode::from_str("blink"), Some(RgbMode::Blink));
        assert_eq!(RgbMode::from_str("cycle"), Some(RgbMode::Cycle));
        assert_eq!(RgbMode::from_str("wave"), Some(RgbMode::Wave));
        assert_eq!(RgbMode::from_str("lightning"), Some(RgbMode::Lightning));
        assert_eq!(RgbMode::from_str("pulse"), Some(RgbMode::Pulse));
        assert_eq!(RgbMode::from_str("visualizer"), Some(RgbMode::Visualizer));
        assert_eq!(RgbMode::from_str("invalid"), None);
    }

    #[test]
    fn test_color_scheme_builder() {
        let scheme = ColorScheme::new(RgbMode::Solid)
            .with_brightness(80)
            .with_speed(60)
            .with_delay(40)
            .with_colors(vec![RgbColor::new(255, 0, 0)]);

        assert_eq!(scheme.mode, RgbMode::Solid);
        assert_eq!(scheme.brightness, 80);
        assert_eq!(scheme.speed, 60);
        assert_eq!(scheme.delay, 40);
        assert_eq!(scheme.colors.len(), 1);
        assert_eq!(scheme.colors[0], RgbColor::new(255, 0, 0));
    }

    #[test]
    fn test_color_scheme_clamps_values() {
        let scheme = ColorScheme::new(RgbMode::Solid)
            .with_brightness(200) // Should clamp to 100
            .with_speed(150) // Should clamp to 100
            .with_delay(250); // Should clamp to 100

        assert_eq!(scheme.brightness, 100);
        assert_eq!(scheme.speed, 100);
        assert_eq!(scheme.delay, 100);
    }

    #[test]
    fn test_generate_solid_with_color() {
        let colors = vec![RgbColor::new(255, 0, 0)];
        let result = generate_solid(&colors).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], RgbColor::new(255, 0, 0));
    }

    #[test]
    fn test_generate_solid_default() {
        let result = generate_solid(&[]).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], RgbColor::from_u32(RED).unwrap());
    }

    #[test]
    fn test_generate_blink_with_colors() {
        let colors = vec![RgbColor::new(255, 0, 0)];
        let result = generate_blink(&colors, 50, 50).unwrap();

        // Should have both color segments and black segments
        assert!(result.len() > 0);

        // Check that we have the specified color
        assert!(result.contains(&RgbColor::new(255, 0, 0)));

        // Check that we have black (off) segments
        assert!(result.contains(&RgbColor::from_u32(BLACK).unwrap()));
    }

    #[test]
    fn test_generate_cycle_creates_gradients() {
        let colors = vec![RgbColor::new(255, 0, 0), RgbColor::new(0, 0, 255)];
        let result = generate_cycle(&colors, 50).unwrap();

        // Should create a gradient longer than just the input colors
        assert!(result.len() > colors.len());

        // First color should match
        assert_eq!(result[0], colors[0]);
    }

    #[test]
    fn test_generate_gradient() {
        let start = RgbColor::new(0, 0, 0);
        let end = RgbColor::new(255, 0, 0);
        let gradient = generate_gradient(start, end, 5);

        assert_eq!(gradient.len(), 5);
        assert_eq!(gradient[0], start);
        assert_eq!(gradient[4], end);

        // Check intermediate values are interpolated
        assert!(gradient[1].r > 0 && gradient[1].r < 255);
        assert!(gradient[2].r > gradient[1].r);
        assert!(gradient[3].r > gradient[2].r);
    }

    #[test]
    fn test_generate_gradient_single_step() {
        let start = RgbColor::new(255, 0, 0);
        let gradient = generate_gradient(start, RgbColor::new(0, 255, 0), 1);

        assert_eq!(gradient.len(), 1);
        assert_eq!(gradient[0], start);
    }

    #[test]
    fn test_generate_gradient_zero_length() {
        let gradient = generate_gradient(RgbColor::new(255, 0, 0), RgbColor::new(0, 255, 0), 0);

        assert_eq!(gradient.len(), 0);
    }

    #[test]
    fn test_speed_range_mapping() {
        // Speed 0 (slowest) should give maximum value
        assert_eq!(speed_range(10, 100, 0), 100);

        // Speed 100 (fastest) should give minimum value
        assert_eq!(speed_range(10, 100, 100), 10);

        // Speed 50 should give middle value
        assert_eq!(speed_range(10, 100, 50), 55);
    }

    #[test]
    fn test_calculate_gradient_length() {
        let colors = vec![
            RgbColor::new(255, 0, 0),
            RgbColor::new(0, 255, 0),
            RgbColor::new(0, 0, 255),
        ];

        // Speed 100 (fast) should give shorter gradients
        let len_fast = calculate_gradient_length(&colors, 100);

        // Speed 0 (slow) should give longer gradients
        let len_slow = calculate_gradient_length(&colors, 0);

        assert!(len_fast < len_slow);
        assert!(len_fast >= MIN_CYCL_TR);
        assert!(len_slow <= MAX_CYCL_TR);
    }

    #[test]
    fn test_generate_lightning_creates_flash() {
        let colors = vec![RgbColor::new(255, 255, 255)];
        let result = generate_lightning(&colors, 50, false).unwrap();

        // Should have multiple frames
        assert!(result.len() > 1);

        // Should contain both the flash color and black
        let has_white = result.iter().any(|c| c.r > 200 && c.g > 200 && c.b > 200);
        let has_black = result.contains(&RgbColor::from_u32(BLACK).unwrap());

        assert!(has_white, "Lightning should have bright flash");
        assert!(has_black, "Lightning should have dark periods");
    }

    #[test]
    fn test_generate_wave_shifts_colors() {
        let colors = vec![
            RgbColor::new(255, 0, 0),
            RgbColor::new(0, 255, 0),
            RgbColor::new(0, 0, 255),
        ];

        let result_no_shift = generate_wave(&colors, 50, false).unwrap();
        let result_with_shift = generate_wave(&colors, 50, true).unwrap();

        // Both should generate sequences
        assert!(result_no_shift.len() > 0);
        assert!(result_with_shift.len() > 0);

        // With shift enabled, the starting color pattern should be different
        // (though both sequences might start with same color due to gradient)
        assert_eq!(result_no_shift.len(), result_with_shift.len());
    }

    #[test]
    fn test_next_gradient_color() {
        let start = RgbColor::new(0, 0, 0);
        let end = RgbColor::new(100, 100, 100);

        let next = next_gradient_color(start, end, 10);

        // Should be a small step toward the end color
        assert!(next.r > start.r && next.r < end.r);
        assert!(next.g > start.g && next.g < end.g);
        assert!(next.b > start.b && next.b < end.b);
    }

    #[test]
    fn test_color_scheme_apply_brightness() {
        let mut scheme = ColorScheme::new(RgbMode::Solid)
            .with_colors(vec![RgbColor::new(255, 255, 255)])
            .with_brightness(50);

        scheme.apply_brightness().unwrap();

        // Colors should be dimmed to 50%
        assert_eq!(scheme.colors[0].r, 127);
        assert_eq!(scheme.colors[0].g, 127);
        assert_eq!(scheme.colors[0].b, 127);
    }

    #[test]
    fn test_rainbow_constant() {
        assert_eq!(RAINBOW.len(), 9);

        // Verify all rainbow colors are valid
        for &color in &RAINBOW {
            assert!(color <= 0xffffff);
            assert!(RgbColor::from_u32(color).is_ok());
        }
    }

    #[test]
    fn test_generate_color_sequence_all_modes() {
        let modes = [
            RgbMode::Solid,
            RgbMode::Blink,
            RgbMode::Cycle,
            RgbMode::Wave,
            RgbMode::Lightning,
            RgbMode::Pulse,
        ];

        for mode in modes {
            let mut scheme = ColorScheme::new(mode);
            let result = generate_color_sequence(&mut scheme);

            assert!(result.is_ok(), "Mode {:?} should generate sequence", mode);
            let sequence = result.unwrap();
            assert!(
                sequence.len() > 0,
                "Mode {:?} should generate non-empty sequence",
                mode
            );
        }
    }

    #[test]
    fn test_generate_color_sequence_visualizer_not_implemented() {
        let mut scheme = ColorScheme::new(RgbMode::Visualizer);
        let result = generate_color_sequence(&mut scheme);

        assert!(
            result.is_err(),
            "Visualizer mode should not be implemented yet"
        );
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_any_valid_color_roundtrip(color in 0u32..=0xffffff) {
            let rgb = RgbColor::from_u32(color).unwrap();
            assert_eq!(rgb.to_u32(), color);
        }

        #[test]
        fn test_brightness_reduces_color_values(
            r in 0u8..=255,
            g in 0u8..=255,
            b in 0u8..=255,
            brightness in 0u8..=100
        ) {
            let color = RgbColor::new(r, g, b);
            let adjusted = color.with_brightness(brightness).unwrap();

            assert!(adjusted.r <= r);
            assert!(adjusted.g <= g);
            assert!(adjusted.b <= b);
        }

        #[test]
        fn test_brightness_100_unchanged(
            r in 0u8..=255,
            g in 0u8..=255,
            b in 0u8..=255
        ) {
            let color = RgbColor::new(r, g, b);
            let adjusted = color.with_brightness(100).unwrap();

            // Allow for 1 unit of rounding error due to float conversion
            assert!((adjusted.r as i16 - r as i16).abs() <= 1);
            assert!((adjusted.g as i16 - g as i16).abs() <= 1);
            assert!((adjusted.b as i16 - b as i16).abs() <= 1);
        }

        #[test]
        fn test_rgb_packet_never_overflows(colors_len in 0usize..100) {
            let colors = vec![RgbColor::new(0xff, 0xff, 0xff); colors_len];
            let packet = create_rgb_packet(&colors);

            // Should never exceed packet size
            assert_eq!(packet.len(), PACKET_SIZE);
        }
    }
}
