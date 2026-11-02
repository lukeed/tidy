//! `new RegExp(pattern)` and `regex.test(text)`, with regress, which implements JavaScript's regular expressions.

#[derive(Debug)]
pub(super) struct Regex(regress::Regex);

impl Regex {
  /// `new RegExp(pattern)`, which fails with a message like JavaScript's.
  pub(super) fn new(pattern: &[u8]) -> Result<Regex, String> {
    let pattern = String::from_utf8_lossy(pattern);
    regress::Regex::new(&pattern).map(Regex).map_err(|error| error.to_string())
  }

  pub(super) fn test(&self, text: &[u8]) -> bool {
    self.0.find(&String::from_utf8_lossy(text)).is_some()
  }
}
