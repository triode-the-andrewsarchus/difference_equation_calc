//! Compact numeric formatting utilities for responsive UI layouts.
//!
//! Ensures that numbers rendered in readouts, tooltips, cards, drag controls,
//! and plots never exceed a bounded character length (~5-8 chars) so that UI
//! widgets never expand, shrink, or trigger layout jitter during live direct manipulation.

/// Formats a float into a compact, bounded string (typically 5 to 8 characters).
///
/// - Extreme values (>= 10,000 or < 0.001) switch to scientific notation with a 2-decimal mantissa (e.g. `1.23e5`, `4.56e-4`).
/// - Standard range values use adaptive decimal precision to maintain 3 to 4 significant figures.
/// - Non-finite values display as `"NaN"`, `"+∞"`, or `"-∞"`.
pub fn format_compact_value(val: f64) -> String {
    if val.is_nan() {
        return "NaN".to_string();
    }
    if val.is_infinite() {
        return if val > 0.0 {
            "+∞".to_string()
        } else {
            "-∞".to_string()
        };
    }

    let abs_val = val.abs();
    if abs_val < 1e-9 {
        return "0.000".to_string();
    }

    if abs_val >= 10_000.0 || abs_val < 0.001 {
        // Scientific notation: 2 decimals in mantissa -> e.g. "1.23e5", "-1.23e-4"
        format!("{:.2e}", val)
    } else if abs_val >= 1000.0 {
        format!("{:.1}", val) // e.g. 1234.5 (6 chars)
    } else if abs_val >= 100.0 {
        format!("{:.2}", val) // e.g. 123.45 (6 chars)
    } else if abs_val >= 10.0 {
        format!("{:.3}", val) // e.g. 12.345 (6 chars)
    } else {
        format!("{:.4}", val) // e.g. 1.2345 or 0.1234 (6 chars)
    }
}

/// Formats a relative difference with an explicit `+` or `-` sign compactly.
pub fn format_compact_diff(val: f64) -> String {
    if val > 0.0 {
        format!("+{}", format_compact_value(val))
    } else {
        format_compact_value(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_compact_bounds() {
        let test_cases = [
            0.0,
            -0.0,
            0.0000001,
            0.0005,
            0.001,
            0.01234,
            0.123456,
            1.23456,
            12.3456,
            123.456,
            1234.56,
            12345.6,
            1_000_000.0,
            1e15,
            -0.0000001,
            -0.0005,
            -0.001,
            -0.01234,
            -0.123456,
            -1.23456,
            -12.3456,
            -123.456,
            -1234.56,
            -12345.6,
            -1_000_000.0,
            -1e15,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];

        for &val in &test_cases {
            let s = format_compact_value(val);
            assert!(
                s.len() <= 9,
                "Formatted string '{}' for {} is longer than 9 characters (len={})",
                s,
                val,
                s.len()
            );
            assert!(!s.is_empty());
        }
    }
}
