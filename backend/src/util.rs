//! Small shared helpers used across API modules.

/// Turn a human name into a URL-safe slug: lowercase, non-alphanumerics
/// collapsed to single dashes, no leading/trailing dashes.
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Generate a random alphanumeric string of the given length (e.g. passwords,
/// invitation tokens).
pub fn random_string(len: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("My Project"), "my-project");
        assert_eq!(slugify("hello-world"), "hello-world");
        assert_eq!(slugify("TestDB"), "testdb");
    }

    #[test]
    fn test_slugify_special_chars() {
        assert_eq!(slugify("My   Project!"), "my-project");
        assert_eq!(slugify("hello@world#123"), "hello-world-123");
        assert_eq!(slugify("  spaces  "), "spaces");
    }

    #[test]
    fn test_slugify_consecutive_dashes() {
        assert_eq!(slugify("a---b---c"), "a-b-c");
    }

    #[test]
    fn test_slugify_empty() {
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn test_slugify_japanese() {
        assert_eq!(slugify("テスト"), "");
    }

    #[test]
    fn test_random_string_length() {
        assert_eq!(random_string(24).len(), 24);
        assert_eq!(random_string(48).len(), 48);
    }

    #[test]
    fn test_random_string_unique() {
        assert_ne!(random_string(24), random_string(24));
    }

    #[test]
    fn test_random_string_chars() {
        assert!(random_string(24).chars().all(|c| c.is_alphanumeric()));
    }
}
