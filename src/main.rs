use std::env;
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

fn contrast_ratio(a: &Color, b: &Color) -> f64 {
    let la = a.relative_luminance();
    let lb = b.relative_luminance();
    let (lighter, darker) = if la > lb { (la, lb) } else { (lb, la) };
    (lighter + 0.05) / (darker + 0.05)
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

fn print_usage() {
    eprintln!("chromawave — palette extraction, mixing, and contrast checks");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  chromawave mix <color1> <color2> <ratio>");
    eprintln!("  chromawave contrast <color1> <color2>");
    eprintln!();
    eprintln!("Colors are 6-digit hex, with or without '#'.");
    eprintln!("Ratio is a float 0.0–1.0 (0 = all color1, 1 = all color2).");
}

fn cmd_mix(args: &[String]) -> Result<(), String> {
    if args.len() != 3 {
        return Err("mix requires exactly 3 arguments: <color1> <color2> <ratio>".into());
    }
    let c1 = parse_hex(&args[0])?;
    let c2 = parse_hex(&args[1])?;
    let t = parse_mix_ratio(&args[2])?;
    let result = mix_colors(&c1, &c2, t);
    println!("{}", result.to_hex());
    Ok(())
}

fn cmd_contrast(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        return Err("contrast requires exactly 2 arguments: <color1> <color2>".into());
    }
    let c1 = parse_hex(&args[0])?;
    let c2 = parse_hex(&args[1])?;
    let ratio = contrast_ratio(&c1, &c2);
    println!("{:.2}", ratio);
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return Err("no command given".into());
    }
    match args[0].as_str() {
        "mix" => cmd_mix(&args[1..]),
        "contrast" => cmd_contrast(&args[1..]),
        "--help" | "-h" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command '{}'", other)),
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
        assert_eq!(c, Color { r: 255, g: 0, b: 102 });
    }

    #[test]
    fn parse_hex_without_hash() {
        let c = parse_hex("00ccff").unwrap();
        assert_eq!(c, Color { r: 0, g: 204, b: 255 });
    }

    #[test]
    fn parse_hex_uppercase() {
        let c = parse_hex("#FF0066").unwrap();
        assert_eq!(c, Color { r: 255, g: 0, b: 102 });
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

    // -- to_hex ---------------------------------------------------------------

    #[test]
    fn to_hex_roundtrip() {
        let c = Color { r: 255, g: 0, b: 102 };
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
        assert_eq!(m, Color { r: 128, g: 128, b: 128 });
    }

    #[test]
    fn mix_specific() {
        let a = parse_hex("#ff0066").unwrap();
        let b = parse_hex("#00ccff").unwrap();
        let m = mix_colors(&a, &b, 0.35);
        // r = 255*0.65 + 0*0.35 = 165.75 → 166
        // g = 0*0.65 + 204*0.35 = 71.4 → 71
        // b = 102*0.65 + 255*0.35 = 66.3 + 89.25 = 155.55 → 156
        assert_eq!(m, Color { r: 166, g: 71, b: 156 });
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
        // #767676 on white is the lightest gray that passes WCAG AA (4.5:1)
        let gray = parse_hex("#767676").unwrap();
        let white = parse_hex("#ffffff").unwrap();
        let r = contrast_ratio(&gray, &white);
        assert!(r >= 4.5, "expected >= 4.5, got {:.2}", r);
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

    // -- CLI dispatch -----------------------------------------------------------

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
}
