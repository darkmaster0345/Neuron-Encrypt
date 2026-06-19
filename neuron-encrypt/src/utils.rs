pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let len_a = a.len();
    let len_b = b.len();
    let max_len = len_a.max(len_b);

    let mut byte_comparison_result = 0;

    for i in 0..max_len {
        let byte_a = a.get(i).unwrap_or(&0);
        let byte_b = b.get(i).unwrap_or(&0);
        byte_comparison_result |= byte_a ^ byte_b;
    }

    let len_diff = len_a ^ len_b;
    let len_mismatch_flag = (((len_diff | len_diff.wrapping_neg()) >> (usize::BITS - 1)) & 1) as u8;

    let final_result = byte_comparison_result | len_mismatch_flag;

    final_result == 0
}

pub fn is_vx2_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("vx2"))
        .unwrap_or(false)
}

pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

pub struct HumanBytes(pub u64);

impl std::fmt::Display for HumanBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let b = self.0;
        if b < 1024 {
            write!(f, "{b} B")
        } else if b < 1024 * 1024 {
            write!(f, "{:.1} KB", b as f64 / 1024.0)
        } else if b < 1024 * 1024 * 1024 {
            write!(f, "{:.1} MB", b as f64 / (1024.0 * 1024.0))
        } else {
            write!(f, "{:.2} GB", b as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}
