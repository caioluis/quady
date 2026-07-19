# QuadCast RGB Modes

This implementation mimics all the RGB modes from the original C implementation (QuadcastRGB-C).

## Available Modes

### 1. Solid
Single static color that stays constant.

```rust
let mode = RgbMode::Solid;
let mut scheme = ColorScheme::new(mode)
    .with_colors(vec![RgbColor::new(255, 0, 0)])  // Red
    .with_brightness(100);
```

### 2. Blink
Colors blink on and off with configurable speed and delay.

```rust
let mode = RgbMode::Blink;
let mut scheme = ColorScheme::new(mode)
    .with_colors(vec![
        RgbColor::new(255, 0, 0),    // Red
        RgbColor::new(0, 255, 0),    // Green
        RgbColor::new(0, 0, 255),    // Blue
    ])
    .with_speed(70)      // How fast the blink is (0-100)
    .with_delay(30)      // Delay between blinks (0-100)
    .with_brightness(100);
```

**Random colors:** If no colors are specified, it will use random colors.

### 3. Cycle
Smooth color transitions that loop continuously. Creates gradients between colors.

```rust
let mode = RgbMode::Cycle;
let mut scheme = ColorScheme::new(mode)
    .with_speed(50)      // Animation speed (0-100, higher = faster)
    .with_brightness(100);
```

**Default:** Uses rainbow colors if no colors specified.

### 4. Wave
Like Cycle mode but with a phase shift between upper and lower LEDs, creating a wave effect.

```rust
let mode = RgbMode::Wave;
let mut scheme = ColorScheme::new(mode)
    .with_speed(50)
    .with_brightness(100);
```

**Default:** Uses rainbow colors if no colors specified.

### 5. Lightning
Flash effect with fade up and down. Upper and lower LEDs flash alternately.

```rust
let mode = RgbMode::Lightning;
let mut scheme = ColorScheme::new(mode)
    .with_colors(vec![RgbColor::new(255, 255, 255)])  // White lightning
    .with_speed(60)      // Controls flash speed (0-100)
    .with_brightness(100);
```

### 6. Pulse
Like Lightning but synchronized - upper and lower LEDs pulse together.

```rust
let mode = RgbMode::Pulse;
let mut scheme = ColorScheme::new(mode)
    .with_colors(vec![RgbColor::new(255, 0, 255)])  // Purple pulse
    .with_speed(50)
    .with_brightness(100);
```

### 7. Visualizer
Not yet implemented.

## Usage in main.rs

To change the RGB mode, edit the `run_rgb_mode()` function in `src-tauri/src/main.rs`:

```rust
fn run_rgb_mode() -> QuadyResult<()> {
    // ... device setup code ...
    
    // Change this line to select your desired mode:
    let mode = RgbMode::Cycle;  // Try: Solid, Blink, Cycle, Wave, Lightning, Pulse
    
    // Configure the color scheme:
    let mut scheme = ColorScheme::new(mode)
        .with_brightness(100)  // 0-100
        .with_speed(50);       // 0-100 (affects animation speed)
    
    // Optional: Add custom colors
    // .with_colors(vec![
    //     RgbColor::new(255, 0, 0),
    //     RgbColor::new(0, 255, 0),
    //     RgbColor::new(0, 0, 255),
    // ])
    
    // ... rest of the code ...
}
```

## Constants and Defaults

From `lib.rs`:

- `MAX_BR_SPD_DLY`: 100 (maximum for brightness, speed, delay)
- `SPD_DEFAULT`: 50 (default speed)
- `DLY_DEFAULT`: 50 (default delay for blink mode)
- `RAINBOW`: Default rainbow color palette with 9 colors
- `BLACK`: 0x000000
- `RED`: 0xff0000

## Color Conversion

You can create colors in multiple ways:

```rust
// Direct RGB values
let color = RgbColor::new(255, 128, 0);

// From hex value
let color = RgbColor::from_u32(0xff8000)?;

// Using predefined constants
let rainbow_colors: Vec<RgbColor> = RAINBOW
    .iter()
    .map(|&c| RgbColor::from_u32(c))
    .collect::<Result<Vec<_>>>()?;
```

## Implementation Details

The implementation closely follows the C code structure:

- **Mode generation**: Each mode has its own generation function that creates a sequence of RGB colors
- **Gradient generation**: Smooth transitions between colors using linear interpolation
- **Speed mapping**: Speed parameters are mapped to appropriate ranges for each mode
- **Packet structure**: Colors are packed into 64-byte USB packets with the format `[0x81, R, G, B]` repeated
- **Interleaving**: Upper and lower LED data is interleaved in the packets as the device expects

## Building

```bash
cd src-tauri
cargo build
```

## Testing

Run the application:
```bash
cargo run
```

The RGB mode will start automatically in a background thread when the application launches.
