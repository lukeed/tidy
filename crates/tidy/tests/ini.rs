//! INI files, with outputs that prettier-plugin-ini 1.3.0 prints, where tidy agrees with it. `tests/oracle/ini.ts`
//! compares the rest with the plugin.

use tidy::doc::Eol;
use tidy::ini::{Dialect, Options, format};

fn tidy(text: &str) -> String {
  tidy_with(text, &Options::default())
}

fn tidy_with(text: &str, options: &Options) -> String {
  let formatted = format(text, options).unwrap();
  assert_eq!(format(&formatted, options).unwrap(), formatted, "formatted again: {text:?}");
  formatted
}

#[test]
fn knows_ini_paths() {
  let cases = [
    ("a/setup.cfg", Some(Dialect::Ini)),
    ("x.INI", Some(Dialect::Ini)),
    (".editorconfig", Some(Dialect::Ini)),
    ("app/.npmrc", Some(Dialect::Ini)),
    ("pylintrc", Some(Dialect::Ini)),
    (".gitconfig", Some(Dialect::Git)),
    ("repo/.gitmodules", Some(Dialect::Git)),
    ("work.gitconfig", Some(Dialect::Git)),
    ("app.pro", None),
    ("export.reg", None),
    (".env", None),
    ("ini", None),
  ];
  for (path, dialect) in cases {
    assert_eq!(Dialect::of_path(path), dialect, "{path}");
  }
}

#[test]
fn formats_params_sections_and_comments() {
  let text = "; top\n  name   =   value  \n[ section ]\n  # note\nkey=a = b\n";
  let expected = "; top\nname = value\n[section ]\n# note\nkey = a = b\n";
  assert_eq!(tidy(text), expected);
}

#[test]
fn writes_equals_without_spaces() {
  let options = Options { spaces: false, ..Options::default() };
  assert_eq!(tidy_with("a = 1\n[b]\nc =\n", &options), "a=1\n[b]\nc=\n");
}

#[test]
fn indents_the_values_of_a_list() {
  let text = "[options]\nname = x\ninstall_requires =\n    click>=8\n    rich\n";
  let expected = "[options]\nname = x\n\ninstall_requires =\n  click>=8\n  rich\n";
  assert_eq!(tidy(text), expected);
  let options = Options { tabs: true, ..Options::default() };
  assert_eq!(tidy_with("a =\n  b\n", &options), "a =\n\tb\n");
}

#[test]
fn keeps_blank_lines_as_the_plugin_does() {
  let cases = [
    // A section gets a blank line before it, after the lines of the one before.
    ("[a]\nx=1\n[b]\ny=2\n", "[a]\nx = 1\n\n[b]\ny = 2\n"),
    // A blank line ends a section, and runs of them collapse.
    ("[a]\nx=1\n\n\n\ny=2\n", "[a]\nx = 1\n\ny = 2\n"),
    // An empty section has one blank line after it; the plugin adds one more on each pass.
    ("[a]\n\n\n[b]\nx=1\n", "[a]\n\n[b]\nx = 1\n"),
    // A list without values is a key with an empty value; the plugin prints a blank line after it.
    ("a =\n   \nb = 1\n", "a =\n\nb = 1\n"),
  ];
  for (text, expected) in cases {
    assert_eq!(tidy(text), expected, "{text:?}");
  }
}

#[test]
fn keeps_indents_where_the_plugin_rejects_a_line() {
  let cases = [
    // configparser: a line indented deeper than its key continues the key's value.
    ("[a]\nkey = first\n    second=2\nbare\n", "[a]\nkey = first\n    second=2\nbare\n"),
    // web-platform-tests metadata.
    (
      "[test.html]\n  [subtest]\n    expected:\n      if os == \"linux\": FAIL\n",
      "[test.html]\n  [subtest]\n    expected:\n      if os == \"linux\": FAIL\n",
    ),
    // A comment after a section, and `]` in its name.
    ("[*.out]   # editor\n[*.[ch]]\nx=1\n", "[*.out] # editor\n[*.[ch]]\nx = 1\n"),
  ];
  for (text, expected) in cases {
    assert_eq!(tidy(text), expected, "{text:?}");
  }
}

#[test]
fn keeps_indents_in_git_config() {
  let options = Options { dialect: Dialect::Git, ..Options::default() };
  let text = "[user]\n\tname=Person\n[alias]\n\tlg = log \\\n\t\t--oneline=x\n\tbare\n\n\n[core]\n";
  let expected =
    "[user]\n\tname = Person\n[alias]\n\tlg = log \\\n\t\t--oneline=x\n\tbare\n\n[core]\n";
  assert_eq!(tidy_with(text, &options), expected);
}

#[test]
fn keeps_the_byte_order_mark_and_writes_the_line_ending() {
  let options = Options { eol: Eol::Crlf, ..Options::default() };
  assert_eq!(
    tidy_with("\u{FEFF}a=1\r\nb =\r  c\n", &options),
    "\u{FEFF}a = 1\r\n\r\nb =\r\n  c\r\n"
  );
  assert_eq!(tidy(" \n\t\n"), "");
}

#[test]
fn formats_ini_in_markdown() {
  let text = "# Setup\n\n```ini\n[a]\nx=1\n```\n";
  let expected = "# Setup\n\n```ini\n[a]\nx = 1\n```\n";
  let options = tidy::Options::default();
  let printed =
    tidy::format(text, tidy::Language::Markdown, &options, None, &mut tidy::embed::Verbatim)
      .unwrap();
  assert_eq!(printed, expected);
}
