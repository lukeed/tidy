//! Prettier's `printNumber`, and what JavaScript's `String(number)` gives.

/// `printNumber`: lowercase, without needless signs, zeros and dots.
pub(crate) fn print(raw: &str) -> String {
  if raw.len() == 1 {
    return raw.to_owned();
  }
  let mut number = raw.to_ascii_lowercase();
  let mantissa = mantissa_length(&number);
  if mantissa > 0 && number.as_bytes().get(mantissa) == Some(&b'e') {
    let exponent = mantissa + 1;
    let rest = &number[exponent..];
    let (sign, rest) = match rest.as_bytes().first() {
      Some(b'+') => ("", &rest[1..]),
      Some(b'-') => ("-", &rest[1..]),
      _ => ("", rest),
    };
    let zeros = rest.bytes().take_while(|byte| *byte == b'0').count();
    let zeros = match rest.as_bytes().get(zeros).is_some_and(u8::is_ascii_digit) {
      true => zeros,
      false => zeros.saturating_sub(1),
    };
    if rest.as_bytes().get(zeros).is_some_and(u8::is_ascii_digit) {
      number = format!("{}{sign}{}", &number[..exponent], &rest[zeros..]);
    }
  }
  if mantissa > 0 && number.as_bytes().get(mantissa) == Some(&b'e') {
    let exponent = number[mantissa + 1..].trim_start_matches(['+', '-']);
    if !exponent.is_empty() && exponent.bytes().all(|byte| byte == b'0') {
      number.truncate(mantissa);
    }
  }
  let signed = number.starts_with(['+', '-']) as usize;
  if number[signed..].starts_with('.') {
    number.insert(signed, '0');
  }
  if let Some(dot) = number.find('.') {
    let digits = number[dot + 1..].bytes().take_while(u8::is_ascii_digit).count();
    let end = dot + 1 + digits;
    let at_end = matches!(number.as_bytes().get(end), None | Some(b'e'));
    if at_end && digits >= 2 {
      let kept = number[dot + 1..end].trim_end_matches('0').len().max(1);
      number.replace_range(dot + 1 + kept..end, "");
    }
  }
  if let Some(dot) = number.find('.')
    && matches!(number.as_bytes().get(dot + 1), None | Some(b'e'))
  {
    number.remove(dot);
  }
  number
}

/// The length of `^[+-]?[\d.]+`, or 0 if it doesn't match.
fn mantissa_length(number: &str) -> usize {
  let signed = number.starts_with(['+', '-']) as usize;
  let digits =
    number[signed..].bytes().take_while(|byte| byte.is_ascii_digit() || *byte == b'.').count();
  if digits == 0 { 0 } else { signed + digits }
}

/// `/^(?:\d+|\d+\.\d+)$/`
pub(crate) fn is_simple(number: &str) -> bool {
  let (integer, fraction) = number.split_once('.').unwrap_or((number, "1"));
  let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
  digits(integer) && digits(fraction)
}

/// True if `String(Number(text)) === text`: `text` is a decimal number as JavaScript prints it.
pub(crate) fn is_canonical(text: &str) -> bool {
  if !text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'.' | b'e' | b'+' | b'-')) {
    return false;
  }
  text.parse::<f64>().is_ok_and(|value| string(value) == text)
}

/// `String(value)`: ECMAScript's `Number::toString`.
pub(crate) fn string(value: f64) -> String {
  if value.is_nan() {
    return "NaN".into();
  }
  if value == 0.0 {
    return "0".into();
  }
  if value.is_infinite() {
    return if value > 0.0 { "Infinity" } else { "-Infinity" }.into();
  }
  let sign = if value < 0.0 { "-" } else { "" };
  let scientific = format!("{:e}", value.abs());
  let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
  let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
  let k = digits.len() as i32;
  let n = exponent.parse::<i32>().unwrap_or(0) + 1;
  let body = if k <= n && n <= 21 {
    format!("{digits}{}", "0".repeat((n - k) as usize))
  } else if 0 < n && n <= 21 {
    format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
  } else if -6 < n && n <= 0 {
    format!("0.{}{digits}", "0".repeat(-n as usize))
  } else {
    let exponent = n - 1;
    let exponent = format!("e{}{}", if exponent < 0 { '-' } else { '+' }, exponent.abs());
    match k {
      1 => format!("{digits}{exponent}"),
      _ => format!("{}.{}{exponent}", &digits[..1], &digits[1..]),
    }
  };
  format!("{sign}{body}")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn prints_numbers_as_prettier_does() {
    for (raw, printed) in [
      ("1.0", "1.0"),
      ("1.00", "1.0"),
      ("1.50", "1.5"),
      ("1.1050", "1.105"),
      (".5", "0.5"),
      ("-.5", "-0.5"),
      ("5.", "5"),
      ("5.e3", "5e3"),
      ("1E+05", "1e5"),
      ("1e-05", "1e-5"),
      ("1e+00", "1"),
      ("1.50e+10", "1.5e10"),
      ("0XFF", "0xff"),
      ("1_000.50", "1_000.5"),
      ("0", "0"),
    ] {
      assert_eq!(print(raw), printed, "{raw}");
    }
  }

  #[test]
  fn writes_numbers_as_javascript_does() {
    for (value, text) in [
      (1.0, "1"),
      (1.5, "1.5"),
      (1e21, "1e+21"),
      (1e20, "100000000000000000000"),
      (1.5e-7, "1.5e-7"),
      (0.000001, "0.000001"),
      (123456789012345680000.0, "123456789012345680000"),
      (-2.5, "-2.5"),
    ] {
      assert_eq!(string(value), text);
    }
    assert!(is_canonical("1e+21"));
    assert!(!is_canonical("1e21"));
    assert!(!is_canonical("017"));
  }
}
