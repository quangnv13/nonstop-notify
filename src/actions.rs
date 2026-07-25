pub fn build_url(base: &str, route: &str) -> Option<String> {
    let route = route.trim();
    if route.is_empty()
        || route.starts_with("//")
        || route.to_ascii_lowercase().starts_with("javascript:")
        || route.to_ascii_lowercase().starts_with("file:")
    {
        return None;
    }

    if route.starts_with("http://") || route.starts_with("https://") {
        return Some(route.to_string());
    }

    if route.starts_with('/') {
        return Some(format!("{}{}", base.trim_end_matches('/'), route));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_relative_route() {
        assert_eq!(
            build_url("http://127.0.0.1:4137", "/runs/1"),
            Some("http://127.0.0.1:4137/runs/1".into())
        );
    }

    #[test]
    fn accepts_absolute_http_https() {
        assert_eq!(
            build_url("x", "http://a.test"),
            Some("http://a.test".into())
        );
        assert_eq!(
            build_url("x", "https://a.test"),
            Some("https://a.test".into())
        );
    }

    #[test]
    fn rejects_unsafe_or_empty_routes() {
        assert_eq!(build_url("x", ""), None);
        assert_eq!(build_url("x", "javascript:alert(1)"), None);
        assert_eq!(build_url("x", "file:///tmp/a"), None);
        assert_eq!(build_url("x", "//evil.test"), None);
    }
}
