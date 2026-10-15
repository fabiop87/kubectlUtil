//! Funções pequenas e puras, fáceis de testar.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn valid_port(port: &str) -> bool {
    port.parse::<u16>().map(|p| p > 0).unwrap_or(false)
}

fn unix_seconds() -> Option<u64> {
    SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Data/hora UTC legível para o histórico (ex.: `2026-10-14 15:30:00 UTC`).
pub fn timestamp() -> String {
    match unix_seconds() {
        Some(secs) => format_utc(secs),
        None => "data desconhecida".into(),
    }
}

/// Sufixo para nomes de arquivo (ex.: `20261014153000`).
pub fn file_stamp() -> String {
    match unix_seconds() {
        Some(secs) => format_utc(secs).chars().filter(char::is_ascii_digit).collect(),
        None => "sem-data".into(),
    }
}

/// Converte segundos desde a época Unix em data/hora UTC, sem depender de crates externas.
/// Usa o algoritmo `civil_from_days` de Howard Hinnant.
pub fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (hour, minute, second) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_port_accepts_only_1_to_65535() {
        assert!(valid_port("1"));
        assert!(valid_port("5432"));
        assert!(valid_port("65535"));
        assert!(!valid_port("0"));
        assert!(!valid_port("65536"));
        assert!(!valid_port("-1"));
        assert!(!valid_port("abc"));
        assert!(!valid_port(""));
    }

    #[test]
    fn format_utc_known_dates() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc(1_700_000_000), "2023-11-14 22:13:20 UTC");
        // 29 de fevereiro de ano bissexto.
        assert_eq!(format_utc(1_709_164_800), "2024-02-29 00:00:00 UTC");
        // Último segundo de 2023.
        assert_eq!(format_utc(1_704_067_199), "2023-12-31 23:59:59 UTC");
    }

    #[test]
    fn file_stamp_has_only_digits() {
        let stamp = file_stamp();
        assert!(stamp.chars().all(|c| c.is_ascii_digit()), "{stamp}");
        assert_eq!(stamp.len(), 14);
    }
}
