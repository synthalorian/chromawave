use std::env;
use std::fs;
use std::process;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Relative luminance per WCAG 2.1
    /// https://www.w3.org/TR/WCAG21/#dfn-relative-luminance
    fn relative_luminance(&self) -> f64 {
        let r = Self::linearize(self.r);
        let g = Self::linearize(self.g);
        let b = Self::linearize(self.b);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    fn linearize(channel: u8) -> f64 {
        let c = channel as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Convert to HSL (hue 0–360, saturation 0–1, lightness 0–1)
    fn to_hsl(&self) -> (f64, f64, f64) {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < f64::EPSILON {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if (max - r).abs() < f64::EPSILON {
            let mut h = (g - b) / d;
            if g < b {
                h += 6.0;
            }
            h
        } else if (max - g).abs() < f64::EPSILON {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s, l)
    }

    /// Create from HSL (hue 0–360, saturation 0–1, lightness 0–1)
    fn from_hsl(h: f64, s: f64, l: f64) -> Color {
        let h = ((h % 360.0) + 360.0) % 360.0; // normalize
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);
        if s.abs() < f64::EPSILON {
            let v = (l * 255.0).round() as u8;
            return Color { r: v, g: v, b: v };
        }
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let hk = h / 360.0;
        let tc = |mut t: f64| -> f64 {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 1.0 / 2.0 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        Color {
            r: (tc(hk + 1.0 / 3.0) * 255.0).round() as u8,
            g: (tc(hk) * 255.0).round() as u8,
            b: (tc(hk - 1.0 / 3.0) * 255.0).round() as u8,
        }
    }
}

// ---------------------------------------------------------------------------
// Blend modes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
}

impl BlendMode {
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "normal" => Ok(Self::Normal),
            "multiply" => Ok(Self::Multiply),
            "screen" => Ok(Self::Screen),
            "overlay" => Ok(Self::Overlay),
            other => Err(format!(
                "unknown blend mode '{}': expected normal, multiply, screen, or overlay",
                other
            )),
        }
    }

    fn blend_channel(self, a: u8, b: u8) -> u8 {
        let af = a as f64 / 255.0;
        let bf = b as f64 / 255.0;
        let result = match self {
            Self::Normal => bf,
            Self::Multiply => af * bf,
            Self::Screen => 1.0 - (1.0 - af) * (1.0 - bf),
            Self::Overlay => {
                if af < 0.5 {
                    2.0 * af * bf
                } else {
                    1.0 - 2.0 * (1.0 - af) * (1.0 - bf)
                }
            }
        };
        (result * 255.0).round() as u8
    }

    fn blend(self, a: &Color, b: &Color) -> Color {
        Color {
            r: self.blend_channel(a.r, b.r),
            g: self.blend_channel(a.g, b.g),
            b: self.blend_channel(a.b, b.b),
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_hex(input: &str) -> Result<Color, String> {
    let hex = input.strip_prefix('#').unwrap_or(input);
    if hex.len() != 6 {
        return Err(format!(
            "invalid hex color '{}': expected 6 hex digits, got {}",
            input,
            hex.len()
        ));
    }
    let r = u8::from_str_radix(&hex[0..2], 16)
        .map_err(|_| format!("invalid hex color '{}': bad red channel", input))?;
    let g = u8::from_str_radix(&hex[2..4], 16)
        .map_err(|_| format!("invalid hex color '{}': bad green channel", input))?;
    let b = u8::from_str_radix(&hex[4..6], 16)
        .map_err(|_| format!("invalid hex color '{}': bad blue channel", input))?;
    Ok(Color { r, g, b })
}

fn parse_mix_ratio(input: &str) -> Result<f64, String> {
    let ratio: f64 = input
        .parse()
        .map_err(|_| format!("invalid mix ratio '{}': expected a number 0.0–1.0", input))?;
    if !(0.0..=1.0).contains(&ratio) {
        return Err(format!(
            "invalid mix ratio '{}': must be between 0.0 and 1.0",
            input
        ));
    }
    Ok(ratio)
}

fn parse_count(input: &str) -> Result<usize, String> {
    let n: usize = input
        .parse()
        .map_err(|_| format!("invalid count '{}': expected a positive integer", input))?;
    if n == 0 {
        return Err("count must be at least 1".into());
    }
    if n > 256 {
        return Err(format!("count {} exceeds maximum of 256", n));
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// PPM P3 parser
// ---------------------------------------------------------------------------

/// Parse a PPM P3 (ASCII) image and return pixel colors.
/// Format: "P3\n<width> <height>\n<maxval>\n<r> <g> <b> ..." with optional comments.
fn parse_ppm_p3(input: &str) -> Result<Vec<Color>, String> {
    let mut tokens: Vec<&str> = Vec::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Strip inline comments
        let line = match line.find('#') {
            Some(pos) => &line[..pos],
            None => line,
        };
        for tok in line.split_whitespace() {
            tokens.push(tok);
        }
    }

    if tokens.is_empty() {
        return Err("empty PPM file".into());
    }
    if tokens[0] != "P3" {
        return Err(format!(
            "unsupported PPM format '{}': only P3 (ASCII) is supported",
            tokens[0]
        ));
    }
    if tokens.len() < 4 {
        return Err("truncated PPM header: expected P3 <width> <height> <maxval>".into());
    }

    let width: usize = tokens[1]
        .parse()
        .map_err(|_| format!("invalid PPM width '{}'", tokens[1]))?;
    let height: usize = tokens[2]
        .parse()
        .map_err(|_| format!("invalid PPM height '{}'", tokens[2]))?;
    let maxval: u32 = tokens[3]
        .parse()
        .map_err(|_| format!("invalid PPM maxval '{}'", tokens[3]))?;

    if width == 0 || height == 0 {
        return Err("PPM dimensions must be non-zero".into());
    }
    if maxval == 0 || maxval > 65535 {
        return Err(format!("invalid PPM maxval {}: must be 1–65535", maxval));
    }

    let pixel_count = width * height;
    let expected_tokens = 4 + pixel_count * 3;
    if tokens.len() < expected_tokens {
        return Err(format!(
            "truncated PPM: expected {} pixels ({}×{}), got data for {}",
            pixel_count,
            width,
            height,
            (tokens.len() - 4) / 3
        ));
    }

    let scale = 255.0 / maxval as f64;
    let mut colors = Vec::with_capacity(pixel_count);
    for i in 0..pixel_count {
        let base = 4 + i * 3;
        let r: u32 = tokens[base]
            .parse()
            .map_err(|_| format!("invalid red value '{}' at pixel {}", tokens[base], i))?;
        let g: u32 = tokens[base + 1]
            .parse()
            .map_err(|_| format!("invalid green value '{}' at pixel {}", tokens[base + 1], i))?;
        let b: u32 = tokens[base + 2]
            .parse()
            .map_err(|_| format!("invalid blue value '{}' at pixel {}", tokens[base + 2], i))?;
        if r > maxval || g > maxval || b > maxval {
            return Err(format!(
                "pixel {} value ({}, {}, {}) exceeds maxval {}",
                i, r, g, b, maxval
            ));
        }
        colors.push(Color {
            r: (r as f64 * scale).round() as u8,
            g: (g as f64 * scale).round() as u8,
            b: (b as f64 * scale).round() as u8,
        });
    }
    Ok(colors)
}

// ---------------------------------------------------------------------------
// Median-cut quantizer
// ---------------------------------------------------------------------------

/// A bucket of colors for median-cut partitioning.
struct ColorBucket {
    colors: Vec<Color>,
}

impl ColorBucket {
    fn new(colors: Vec<Color>) -> Self {
        Self { colors }
    }

    /// Find the channel with the greatest range.
    fn widest_channel(&self) -> u8 {
        let (mut r_min, mut r_max) = (255u8, 0u8);
        let (mut g_min, mut g_max) = (255u8, 0u8);
        let (mut b_min, mut b_max) = (255u8, 0u8);
        for c in &self.colors {
            r_min = r_min.min(c.r);
            r_max = r_max.max(c.r);
            g_min = g_min.min(c.g);
            g_max = g_max.max(c.g);
            b_min = b_min.min(c.b);
            b_max = b_max.max(c.b);
        }
        let r_range = r_max as u16 - r_min as u16;
        let g_range = g_max as u16 - g_min as u16;
        let b_range = b_max as u16 - b_min as u16;
        if r_range >= g_range && r_range >= b_range {
            0 // red
        } else if g_range >= b_range {
            1 // green
        } else {
            2 // blue
        }
    }

    /// Sort colors by the given channel (0=r, 1=g, 2=b).
    fn sort_by_channel(&mut self, channel: u8) {
        self.colors.sort_by_key(|c| match channel {
            0 => c.r,
            1 => c.g,
            _ => c.b,
        });
    }

    /// Average the colors in this bucket.
    fn average(&self) -> Color {
        if self.colors.is_empty() {
            return Color { r: 0, g: 0, b: 0 };
        }
        let (mut r_sum, mut g_sum, mut b_sum) = (0u64, 0u64, 0u64);
        for c in &self.colors {
            r_sum += c.r as u64;
            g_sum += c.g as u64;
            b_sum += c.b as u64;
        }
        let n = self.colors.len() as u64;
        Color {
            r: (r_sum / n) as u8,
            g: (g_sum / n) as u8,
            b: (b_sum / n) as u8,
        }
    }
}

/// Extract `count` dominant colors using median-cut quantization.
fn median_cut(colors: &[Color], count: usize) -> Vec<Color> {
    if colors.is_empty() || count == 0 {
        return Vec::new();
    }
    if count >= colors.len() {
        // Deduplicate and return
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for c in colors {
            if seen.insert((c.r, c.g, c.b)) {
                out.push(c.clone());
            }
        }
        return out;
    }

    let mut buckets: Vec<ColorBucket> = vec![ColorBucket::new(colors.to_vec())];

    while buckets.len() < count {
        // Find the bucket with the most colors and widest range
        let mut best_idx = 0;
        let mut best_score = 0usize;
        for (i, b) in buckets.iter().enumerate() {
            if b.colors.len() > best_score {
                best_score = b.colors.len();
                best_idx = i;
            }
        }
        if best_score <= 1 {
            break; // can't split further
        }
        let mut bucket = buckets.remove(best_idx);
        let ch = bucket.widest_channel();
        bucket.sort_by_channel(ch);
        let mid = bucket.colors.len() / 2;
        let right = bucket.colors.split_off(mid);
        buckets.push(ColorBucket::new(bucket.colors));
        buckets.push(ColorBucket::new(right));
    }

    buckets.iter().map(|b| b.average()).collect()
}

// ---------------------------------------------------------------------------
// Pure core functions
// ---------------------------------------------------------------------------

fn mix_colors(a: &Color, b: &Color, t: f64) -> Color {
    let lerp = |x: u8, y: u8| -> u8 { (x as f64 * (1.0 - t) + y as f64 * t).round() as u8 };
    Color {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
    }
}

fn mix_colors_blended(a: &Color, b: &Color, t: f64, mode: BlendMode) -> Color {
    let blended = mode.blend(a, b);
    mix_colors(a, &blended, t)
}

fn contrast_ratio(a: &Color, b: &Color) -> f64 {
    let la = a.relative_luminance();
    let lb = b.relative_luminance();
    let (lighter, darker) = if la > lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

/// WCAG contrast rating string.
fn contrast_rating(ratio: f64) -> &'static str {
    if ratio >= 7.0 {
        "AAA"
    } else if ratio >= 4.5 {
        "AA"
    } else if ratio >= 3.0 {
        "AA Large"
    } else {
        "Fail"
    }
}

/// Generate a palette from a base color.
/// scheme: "complementary", "analogous", "triadic", "split-complementary"
fn generate_palette(base: &Color, scheme: &str, count: usize) -> Result<Vec<Color>, String> {
    let (h, s, l) = base.to_hsl();
    match scheme {
        "complementary" => {
            let mut colors = vec![base.clone()];
            colors.push(Color::from_hsl(h + 180.0, s, l));
            // Add variations
            while colors.len() < count {
                let i = colors.len() as f64;
                let shift = 15.0 * i;
                let l_shift = if colors.len() % 2 == 0 { 0.1 } else { -0.1 };
                colors.push(Color::from_hsl(
                    h + 180.0 + shift,
                    s,
                    (l + l_shift).clamp(0.0, 1.0),
                ));
            }
            Ok(colors)
        }
        "analogous" => {
            let step = 30.0;
            let mut colors = Vec::with_capacity(count);
            let start = h - step * (count as f64 - 1.0) / 2.0;
            for i in 0..count {
                colors.push(Color::from_hsl(start + step * i as f64, s, l));
            }
            Ok(colors)
        }
        "triadic" => {
            let mut colors = vec![base.clone()];
            colors.push(Color::from_hsl(h + 120.0, s, l));
            colors.push(Color::from_hsl(h + 240.0, s, l));
            // Add variations
            while colors.len() < count {
                let i = colors.len() as f64;
                let base_h = match colors.len() % 3 {
                    0 => h,
                    1 => h + 120.0,
                    _ => h + 240.0,
                };
                let l_shift = 0.1 * (i / 3.0);
                colors.push(Color::from_hsl(base_h, s, (l + l_shift).clamp(0.0, 1.0)));
            }
            Ok(colors)
        }
        "split-complementary" => {
            let mut colors = vec![base.clone()];
            colors.push(Color::from_hsl(h + 150.0, s, l));
            colors.push(Color::from_hsl(h + 210.0, s, l));
            while colors.len() < count {
                let i = colors.len() as f64;
                let base_h = match colors.len() % 3 {
                    0 => h,
                    1 => h + 150.0,
                    _ => h + 210.0,
                };
                let l_shift = 0.1 * (i / 3.0);
                colors.push(Color::from_hsl(base_h, s, (l + l_shift).clamp(0.0, 1.0)));
            }
            Ok(colors)
        }
        other => Err(format!(
            "unknown scheme '{}': expected complementary, analogous, triadic, or split-complementary",
            other
        )),
    }
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

fn print_usage() {
    eprintln!("chromawave — palette extraction, mixing, and contrast checks");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  chromawave palette <file.ppm> [--count N]");
    eprintln!("  chromawave mix <color1> <color2> <ratio> [--mode MODE]");
    eprintln!("  chromawave blend <color1> <color2> --mode MODE");
    eprintln!("  chromawave contrast <color1> <color2>");
    eprintln!("  chromawave generate <color> [--scheme SCHEME] [--count N]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  palette    Extract dominant colors from a PPM P3 image");
    eprintln!("  mix        Blend two colors by ratio (0.0–1.0)");
    eprintln!("  blend      Blend two colors with a blend mode");
    eprintln!("  contrast   WCAG 2.1 contrast ratio between two colors");
    eprintln!("  generate   Generate a palette from a base color");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --count N       Number of colors (palette: default 5, generate: default 3)");
    eprintln!("  --mode MODE     Blend mode: normal, multiply, screen, overlay");
    eprintln!(
        "  --scheme SCHEME Palette scheme: complementary, analogous, triadic, split-complementary"
    );
    eprintln!();
    eprintln!("Colors are 6-digit hex, with or without '#'.");
    eprintln!("Ratio is a float 0.0–1.0 (0 = all color1, 1 = all color2).");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  chromawave palette photo.ppm --count 8");
    eprintln!("  chromawave mix '#ff0066' '#00ccff' 0.35");
    eprintln!("  chromawave blend '#ff0066' '#00ccff' --mode multiply");
    eprintln!("  chromawave contrast '#f8f8f2' '#080812'");
    eprintln!("  chromawave generate '#ff0066' --scheme triadic --count 6");
}

fn extract_flag(args: &[String], flag: &str) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

fn positional_args<'a>(args: &'a [String], flags: &[&str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut skip_next = false;
    for (i, arg) in args.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if flags.contains(&arg.as_str()) {
            skip_next = true;
            continue;
        }
        if arg.starts_with("--") {
            continue;
        }
        let _ = i;
        out.push(arg.as_str());
    }
    out
}

fn cmd_palette(args: &[String]) -> Result<(), String> {
    let pos = positional_args(args, &["--count"]);
    if pos.is_empty() {
        return Err(
            "palette requires a PPM file argument: chromawave palette <file.ppm> [--count N]"
                .into(),
        );
    }
    let path = pos[0];
    let count = match extract_flag(args, "--count") {
        Some(v) => parse_count(&v)?,
        None => 5,
    };
    let content = fs::read_to_string(path).map_err(|e| format!("cannot read '{}': {}", path, e))?;
    let pixels = parse_ppm_p3(&content)?;
    if pixels.is_empty() {
        return Err(format!("'{}' contains no pixels", path));
    }
    let palette = median_cut(&pixels, count);
    println!("# chromawave palette v1");
    println!("# source: {} ({} pixels)", path, pixels.len());
    for c in &palette {
        println!("{}", c.to_hex());
    }
    Ok(())
}

fn cmd_mix(args: &[String]) -> Result<(), String> {
    let pos = positional_args(args, &["--mode"]);
    if pos.len() != 3 {
        return Err(
            "mix requires exactly 3 arguments: <color1> <color2> <ratio> [--mode MODE]".into(),
        );
    }
    let c1 = parse_hex(pos[0])?;
    let c2 = parse_hex(pos[1])?;
    let t = parse_mix_ratio(pos[2])?;
    let mode = match extract_flag(args, "--mode") {
        Some(v) => BlendMode::from_str(&v)?,
        None => BlendMode::Normal,
    };
    let result = if mode == BlendMode::Normal {
        mix_colors(&c1, &c2, t)
    } else {
        mix_colors_blended(&c1, &c2, t, mode)
    };
    println!("{}", result.to_hex());
    Ok(())
}

fn cmd_blend(args: &[String]) -> Result<(), String> {
    let pos = positional_args(args, &["--mode"]);
    if pos.len() != 2 {
        return Err("blend requires exactly 2 arguments: <color1> <color2> --mode MODE".into());
    }
    let mode = match extract_flag(args, "--mode") {
        Some(v) => BlendMode::from_str(&v)?,
        None => {
            return Err("blend requires --mode: normal, multiply, screen, or overlay".into());
        }
    };
    let c1 = parse_hex(pos[0])?;
    let c2 = parse_hex(pos[1])?;
    let result = mode.blend(&c1, &c2);
    println!("{}", result.to_hex());
    Ok(())
}

fn cmd_contrast(args: &[String]) -> Result<(), String> {
    let pos = positional_args(args, &[]);
    if pos.len() != 2 {
        return Err("contrast requires exactly 2 arguments: <color1> <color2>".into());
    }
    let c1 = parse_hex(pos[0])?;
    let c2 = parse_hex(pos[1])?;
    let ratio = contrast_ratio(&c1, &c2);
    let rating = contrast_rating(ratio);
    println!("{:.2} {}", ratio, rating);
    Ok(())
}

fn cmd_generate(args: &[String]) -> Result<(), String> {
    let pos = positional_args(args, &["--scheme", "--count"]);
    if pos.len() != 1 {
        return Err(
            "generate requires exactly 1 argument: <color> [--scheme SCHEME] [--count N]".into(),
        );
    }
    let base = parse_hex(pos[0])?;
    let scheme = extract_flag(args, "--scheme").unwrap_or_else(|| "complementary".into());
    let count = match extract_flag(args, "--count") {
        Some(v) => parse_count(&v)?,
        None => 3,
    };
    let palette = generate_palette(&base, &scheme, count)?;
    println!("# chromawave generate v1");
    println!("# base: {} scheme: {}", base.to_hex(), scheme);
    for c in &palette {
        println!("{}", c.to_hex());
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return Err("no command given".into());
    }
    match args[0].as_str() {
        "palette" => cmd_palette(&args[1..]),
        "mix" => cmd_mix(&args[1..]),
        "blend" => cmd_blend(&args[1..]),
        "contrast" => cmd_contrast(&args[1..]),
        "generate" => cmd_generate(&args[1..]),
        "--help" | "-h" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!(
            "unknown command '{}': run chromawave --help for usage",
            other
        )),
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {}", e);
        process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_hex ----------------------------------------------------------

    #[test]
    fn parse_hex_with_hash() {
        let c = parse_hex("#ff0066").unwrap();
        assert_eq!(
            c,
            Color {
                r: 255,
                g: 0,
                b: 102
            }
        );
    }

    #[test]
    fn parse_hex_without_hash() {
        let c = parse_hex("00ccff").unwrap();
        assert_eq!(
            c,
            Color {
                r: 0,
                g: 204,
                b: 255
            }
        );
    }

    #[test]
    fn parse_hex_uppercase() {
        let c = parse_hex("#FF0066").unwrap();
        assert_eq!(
            c,
            Color {
                r: 255,
                g: 0,
                b: 102
            }
        );
    }

    #[test]
    fn parse_hex_too_short() {
        assert!(parse_hex("#ff00").is_err());
    }

    #[test]
    fn parse_hex_too_long() {
        assert!(parse_hex("#ff006699").is_err());
    }

    #[test]
    fn parse_hex_invalid_chars() {
        assert!(parse_hex("#gggggg").is_err());
    }

    #[test]
    fn parse_hex_empty() {
        assert!(parse_hex("").is_err());
        assert!(parse_hex("#").is_err());
    }

    // -- parse_mix_ratio ----------------------------------------------------

    #[test]
    fn mix_ratio_valid() {
        assert_eq!(parse_mix_ratio("0.0").unwrap(), 0.0);
        assert_eq!(parse_mix_ratio("0.5").unwrap(), 0.5);
        assert_eq!(parse_mix_ratio("1.0").unwrap(), 1.0);
    }

    #[test]
    fn mix_ratio_out_of_range() {
        assert!(parse_mix_ratio("-0.1").is_err());
        assert!(parse_mix_ratio("1.1").is_err());
    }

    #[test]
    fn mix_ratio_not_a_number() {
        assert!(parse_mix_ratio("abc").is_err());
    }

    // -- parse_count ----------------------------------------------------------

    #[test]
    fn count_valid() {
        assert_eq!(parse_count("1").unwrap(), 1);
        assert_eq!(parse_count("256").unwrap(), 256);
    }

    #[test]
    fn count_zero() {
        assert!(parse_count("0").is_err());
    }

    #[test]
    fn count_too_large() {
        assert!(parse_count("257").is_err());
    }

    #[test]
    fn count_not_a_number() {
        assert!(parse_count("abc").is_err());
    }

    // -- to_hex ---------------------------------------------------------------

    #[test]
    fn to_hex_roundtrip() {
        let c = Color {
            r: 255,
            g: 0,
            b: 102,
        };
        assert_eq!(c.to_hex(), "#ff0066");
        assert_eq!(parse_hex(&c.to_hex()).unwrap(), c);
    }

    // -- mix_colors -----------------------------------------------------------

    #[test]
    fn mix_at_zero() {
        let a = parse_hex("#ff0000").unwrap();
        let b = parse_hex("#0000ff").unwrap();
        let m = mix_colors(&a, &b, 0.0);
        assert_eq!(m, a);
    }

    #[test]
    fn mix_at_one() {
        let a = parse_hex("#ff0000").unwrap();
        let b = parse_hex("#0000ff").unwrap();
        let m = mix_colors(&a, &b, 1.0);
        assert_eq!(m, b);
    }

    #[test]
    fn mix_midpoint() {
        let a = parse_hex("#000000").unwrap();
        let b = parse_hex("#ffffff").unwrap();
        let m = mix_colors(&a, &b, 0.5);
        assert_eq!(
            m,
            Color {
                r: 128,
                g: 128,
                b: 128
            }
        );
    }

    #[test]
    fn mix_specific() {
        let a = parse_hex("#ff0066").unwrap();
        let b = parse_hex("#00ccff").unwrap();
        let m = mix_colors(&a, &b, 0.35);
        assert_eq!(
            m,
            Color {
                r: 166,
                g: 71,
                b: 156
            }
        );
    }

    // -- contrast_ratio -------------------------------------------------------

    #[test]
    fn contrast_black_white() {
        let black = parse_hex("#000000").unwrap();
        let white = parse_hex("#ffffff").unwrap();
        let r = contrast_ratio(&black, &white);
        assert!((r - 21.0).abs() < 0.01);
    }

    #[test]
    fn contrast_same_color() {
        let c = parse_hex("#808080").unwrap();
        let r = contrast_ratio(&c, &c);
        assert!((r - 1.0).abs() < 0.01);
    }

    #[test]
    fn contrast_symmetric() {
        let a = parse_hex("#f8f8f2").unwrap();
        let b = parse_hex("#080812").unwrap();
        let r1 = contrast_ratio(&a, &b);
        let r2 = contrast_ratio(&b, &a);
        assert!((r1 - r2).abs() < 1e-10);
    }

    #[test]
    fn contrast_wcag_aa_threshold() {
        let gray = parse_hex("#767676").unwrap();
        let white = parse_hex("#ffffff").unwrap();
        let r = contrast_ratio(&gray, &white);
        assert!(r >= 4.5, "expected >= 4.5, got {:.2}", r);
    }

    #[test]
    fn contrast_rating_labels() {
        assert_eq!(contrast_rating(21.0), "AAA");
        assert_eq!(contrast_rating(7.0), "AAA");
        assert_eq!(contrast_rating(4.5), "AA");
        assert_eq!(contrast_rating(3.0), "AA Large");
        assert_eq!(contrast_rating(2.9), "Fail");
        assert_eq!(contrast_rating(1.0), "Fail");
    }

    // -- linearize -------------------------------------------------------------

    #[test]
    fn linearize_zero() {
        assert_eq!(Color::linearize(0), 0.0);
    }

    #[test]
    fn linearize_max() {
        assert!((Color::linearize(255) - 1.0).abs() < 1e-10);
    }

    // -- HSL conversions -------------------------------------------------------

    #[test]
    fn hsl_red() {
        let c = Color { r: 255, g: 0, b: 0 };
        let (h, s, l) = c.to_hsl();
        assert!((h - 0.0).abs() < 0.01);
        assert!((s - 1.0).abs() < 0.01);
        assert!((l - 0.5).abs() < 0.01);
    }

    #[test]
    fn hsl_roundtrip() {
        let original = Color {
            r: 100,
            g: 150,
            b: 200,
        };
        let (h, s, l) = original.to_hsl();
        let back = Color::from_hsl(h, s, l);
        // Allow ±2 per channel for floating point roundtrip
        assert!((original.r as i16 - back.r as i16).abs() <= 2);
        assert!((original.g as i16 - back.g as i16).abs() <= 2);
        assert!((original.b as i16 - back.b as i16).abs() <= 2);
    }

    #[test]
    fn hsl_gray() {
        let c = Color {
            r: 128,
            g: 128,
            b: 128,
        };
        let (_, s, _) = c.to_hsl();
        assert!(s.abs() < 0.01, "gray should have zero saturation");
    }

    #[test]
    fn hsl_wrap_around() {
        // Hue 370 should normalize to 10
        let c1 = Color::from_hsl(10.0, 1.0, 0.5);
        let c2 = Color::from_hsl(370.0, 1.0, 0.5);
        assert_eq!(c1, c2);
    }

    // -- Blend modes ------------------------------------------------------------

    #[test]
    fn blend_normal() {
        let a = parse_hex("#ff0000").unwrap();
        let b = parse_hex("#0000ff").unwrap();
        let result = BlendMode::Normal.blend(&a, &b);
        assert_eq!(result, b);
    }

    #[test]
    fn blend_multiply_black() {
        let a = parse_hex("#ff0000").unwrap();
        let black = parse_hex("#000000").unwrap();
        let result = BlendMode::Multiply.blend(&a, &black);
        assert_eq!(result, black);
    }

    #[test]
    fn blend_multiply_white() {
        let a = parse_hex("#ff0066").unwrap();
        let white = parse_hex("#ffffff").unwrap();
        let result = BlendMode::Multiply.blend(&a, &white);
        assert_eq!(result, a);
    }

    #[test]
    fn blend_screen_black() {
        let a = parse_hex("#ff0066").unwrap();
        let black = parse_hex("#000000").unwrap();
        let result = BlendMode::Screen.blend(&a, &black);
        assert_eq!(result, a);
    }

    #[test]
    fn blend_screen_white() {
        let a = parse_hex("#ff0066").unwrap();
        let white = parse_hex("#ffffff").unwrap();
        let result = BlendMode::Screen.blend(&a, &white);
        assert_eq!(result, white);
    }

    #[test]
    fn blend_overlay_midpoint() {
        let gray = parse_hex("#808080").unwrap();
        let b = parse_hex("#404040").unwrap();
        let result = BlendMode::Overlay.blend(&gray, &b);
        // Just verify it's deterministic
        let result2 = BlendMode::Overlay.blend(&gray, &b);
        assert_eq!(result, result2);
    }

    #[test]
    fn blend_mode_from_str() {
        assert_eq!(BlendMode::from_str("normal").unwrap(), BlendMode::Normal);
        assert_eq!(
            BlendMode::from_str("multiply").unwrap(),
            BlendMode::Multiply
        );
        assert_eq!(BlendMode::from_str("Screen").unwrap(), BlendMode::Screen);
        assert_eq!(BlendMode::from_str("OVERLAY").unwrap(), BlendMode::Overlay);
        assert!(BlendMode::from_str("dodge").is_err());
    }

    // -- PPM P3 parser -----------------------------------------------------------

    #[test]
    fn ppm_basic() {
        let ppm = "P3\n2 1\n255\n255 0 0 0 255 0\n";
        let colors = parse_ppm_p3(ppm).unwrap();
        assert_eq!(colors.len(), 2);
        assert_eq!(colors[0], Color { r: 255, g: 0, b: 0 });
        assert_eq!(colors[1], Color { r: 0, g: 255, b: 0 });
    }

    #[test]
    fn ppm_with_comments() {
        let ppm = "P3\n# this is a comment\n2 1\n255\n255 0 0 # red\n0 255 0\n";
        let colors = parse_ppm_p3(ppm).unwrap();
        assert_eq!(colors.len(), 2);
    }

    #[test]
    fn ppm_maxval_scaling() {
        let ppm = "P3\n1 1\n100\n100 50 0\n";
        let colors = parse_ppm_p3(ppm).unwrap();
        assert_eq!(colors[0].r, 255);
        // 50/100 * 255 = 127.5 → 127 or 128 depending on float precision
        assert!(
            colors[0].g == 127 || colors[0].g == 128,
            "expected 127 or 128, got {}",
            colors[0].g
        );
        assert_eq!(colors[0].b, 0);
    }

    #[test]
    fn ppm_truncated() {
        let ppm = "P3\n2 2\n255\n255 0 0\n";
        assert!(parse_ppm_p3(ppm).is_err());
    }

    #[test]
    fn ppm_empty() {
        assert!(parse_ppm_p3("").is_err());
    }

    #[test]
    fn ppm_wrong_magic() {
        assert!(parse_ppm_p3("P6\n1 1\n255\n0 0 0\n").is_err());
    }

    #[test]
    fn ppm_zero_dimensions() {
        assert!(parse_ppm_p3("P3\n0 1\n255\n").is_err());
    }

    #[test]
    fn ppm_value_exceeds_maxval() {
        let ppm = "P3\n1 1\n100\n200 0 0\n";
        assert!(parse_ppm_p3(ppm).is_err());
    }

    // -- Median cut --------------------------------------------------------------

    #[test]
    fn median_cut_single_color() {
        let c = Color {
            r: 100,
            g: 100,
            b: 100,
        };
        let colors = vec![c.clone(); 5];
        let palette = median_cut(&colors, 3);
        // All identical colors: every bucket averages to the same color
        assert!(!palette.is_empty());
        for p in &palette {
            assert_eq!(*p, c);
        }
    }

    #[test]
    fn median_cut_two_distinct() {
        let mut colors = Vec::new();
        for _ in 0..10 {
            colors.push(Color { r: 255, g: 0, b: 0 });
        }
        for _ in 0..10 {
            colors.push(Color { r: 0, g: 0, b: 255 });
        }
        let palette = median_cut(&colors, 2);
        assert_eq!(palette.len(), 2);
        // One should be reddish, other bluish
        let has_red = palette.iter().any(|c| c.r > 200 && c.b < 50);
        let has_blue = palette.iter().any(|c| c.b > 200 && c.r < 50);
        assert!(has_red, "expected a red-ish color in {:?}", palette);
        assert!(has_blue, "expected a blue-ish color in {:?}", palette);
    }

    #[test]
    fn median_cut_empty() {
        let palette = median_cut(&[], 5);
        assert!(palette.is_empty());
    }

    #[test]
    fn median_cut_more_than_unique() {
        let colors = vec![Color { r: 255, g: 0, b: 0 }, Color { r: 0, g: 255, b: 0 }];
        let palette = median_cut(&colors, 10);
        assert!(palette.len() <= 2);
    }

    #[test]
    fn median_cut_deterministic() {
        let mut colors = Vec::new();
        for i in 0..100u8 {
            colors.push(Color {
                r: i * 2,
                g: 255 - i * 2,
                b: i,
            });
        }
        let p1 = median_cut(&colors, 4);
        let p2 = median_cut(&colors, 4);
        assert_eq!(p1, p2);
    }

    // -- Generate palette ----------------------------------------------------------

    #[test]
    fn generate_complementary() {
        let base = parse_hex("#ff0000").unwrap();
        let palette = generate_palette(&base, "complementary", 2).unwrap();
        assert_eq!(palette.len(), 2);
        assert_eq!(palette[0], base);
        // Complement of red is cyan-ish
        let (_, _, l1) = palette[1].to_hsl();
        let (_, _, l0) = base.to_hsl();
        assert!(
            (l1 - l0).abs() < 0.01,
            "complementary should have same lightness"
        );
    }

    #[test]
    fn generate_analogous() {
        let base = parse_hex("#ff0000").unwrap();
        let palette = generate_palette(&base, "analogous", 3).unwrap();
        assert_eq!(palette.len(), 3);
    }

    #[test]
    fn generate_triadic() {
        let base = parse_hex("#ff0000").unwrap();
        let palette = generate_palette(&base, "triadic", 3).unwrap();
        assert_eq!(palette.len(), 3);
        assert_eq!(palette[0], base);
    }

    #[test]
    fn generate_split_complementary() {
        let base = parse_hex("#ff0000").unwrap();
        let palette = generate_palette(&base, "split-complementary", 3).unwrap();
        assert_eq!(palette.len(), 3);
    }

    #[test]
    fn generate_unknown_scheme() {
        let base = parse_hex("#ff0000").unwrap();
        assert!(generate_palette(&base, "monochromatic", 3).is_err());
    }

    #[test]
    fn generate_count_larger_than_base() {
        let base = parse_hex("#ff0000").unwrap();
        let palette = generate_palette(&base, "triadic", 7).unwrap();
        assert_eq!(palette.len(), 7);
    }

    // -- CLI dispatch ---------------------------------------------------------------

    #[test]
    fn cmd_mix_args_count() {
        assert!(cmd_mix(&[]).is_err());
        assert!(cmd_mix(&["#ff0000".into()]).is_err());
        assert!(cmd_mix(&["#ff0000".into(), "#00ff00".into()]).is_err());
    }

    #[test]
    fn cmd_contrast_args_count() {
        assert!(cmd_contrast(&[]).is_err());
        assert!(cmd_contrast(&["#ff0000".into()]).is_err());
    }

    #[test]
    fn cmd_blend_requires_mode() {
        let args = vec!["#ff0000".to_string(), "#00ff00".to_string()];
        assert!(cmd_blend(&args).is_err());
    }

    #[test]
    fn cmd_generate_args_count() {
        assert!(cmd_generate(&[]).is_err());
    }

    // -- Flag extraction -----------------------------------------------------------

    #[test]
    fn extract_flag_present() {
        let args = vec![
            "file.ppm".to_string(),
            "--count".to_string(),
            "8".to_string(),
        ];
        assert_eq!(extract_flag(&args, "--count"), Some("8".to_string()));
    }

    #[test]
    fn extract_flag_missing() {
        let args = vec!["file.ppm".to_string()];
        assert_eq!(extract_flag(&args, "--count"), None);
    }

    #[test]
    fn positional_args_filters_flags() {
        let args = vec![
            "file.ppm".to_string(),
            "--count".to_string(),
            "8".to_string(),
        ];
        let pos = positional_args(&args, &["--count"]);
        assert_eq!(pos, vec!["file.ppm"]);
    }
}
