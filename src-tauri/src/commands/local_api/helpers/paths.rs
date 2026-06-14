pub(crate) fn normalize_path(raw: &str) -> String {
    let path = raw.split('?').next().unwrap_or(raw).trim();
    if path.is_empty() {
        "/".to_string()
    } else if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    }
}

pub(crate) fn path_segments(path: &str) -> Vec<String> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(decode_segment)
        .collect()
}

fn decode_segment(value: &str) -> String {
    urlencoding::decode(value)
        .map(|decoded| decoded.into_owned())
        .unwrap_or_else(|_| value.to_string())
}
