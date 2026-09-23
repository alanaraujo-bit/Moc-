//! Password, passphrase and PIN generation with honest entropy figures.
//!
//! Every choice comes from the OS RNG with rejection sampling (no modulo bias). The
//! reported entropy is exact for the space the generator actually samples from.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use zeroize::Zeroizing;

use crate::crypto::{random, SecretString};
use crate::error::{CoreError, Result};

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
/// Accepted by virtually every site's password rules.
const SYMBOLS: &str = "!#$%&*+-=?@^_.:~";
/// Characters that are easy to confuse when read aloud or typed from a screen.
const AMBIGUOUS: &str = "Il1|O0o";

static PT_WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
static EN_WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();

pub fn words(language: WordLanguage) -> &'static [&'static str] {
    match language {
        WordLanguage::Pt => PT_WORDS.get_or_init(|| include_str!("wordlists/pt-br.txt").lines().filter(|l| !l.is_empty()).collect()),
        WordLanguage::En => EN_WORDS.get_or_init(|| include_str!("wordlists/en.txt").lines().filter(|l| !l.is_empty()).collect()),
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum WordLanguage {
    #[default]
    Pt,
    En,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CharOptions {
    pub length: u32,
    pub lowercase: bool,
    pub uppercase: bool,
    pub digits: bool,
    pub symbols: bool,
    pub avoid_ambiguous: bool,
    /// Extra characters the user never wants (some sites reject specific symbols).
    #[serde(default)]
    pub exclude: String,
}

impl Default for CharOptions {
    fn default() -> Self {
        Self { length: 20, lowercase: true, uppercase: true, digits: true, symbols: true, avoid_ambiguous: true, exclude: String::new() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PassphraseOptions {
    pub words: u32,
    pub separator: String,
    pub capitalize: bool,
    pub include_number: bool,
    #[serde(default)]
    pub language: WordLanguage,
}

impl Default for PassphraseOptions {
    fn default() -> Self {
        Self { words: 5, separator: "-".into(), capitalize: true, include_number: true, language: WordLanguage::Pt }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Recipe {
    Characters(CharOptions),
    Passphrase(PassphraseOptions),
    Pin { length: u32 },
}

impl Default for Recipe {
    fn default() -> Self {
        Recipe::Characters(CharOptions::default())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Generated {
    pub value: SecretString,
    pub entropy_bits: f64,
}

pub fn generate(recipe: &Recipe) -> Result<Generated> {
    match recipe {
        Recipe::Characters(o) => characters(o),
        Recipe::Passphrase(o) => passphrase(o),
        Recipe::Pin { length } => pin(*length),
    }
}

fn class_chars(set: &str, o: &CharOptions) -> Vec<char> {
    set.chars()
        .filter(|c| !(o.avoid_ambiguous && AMBIGUOUS.contains(*c)))
        .filter(|c| !o.exclude.contains(*c))
        .collect()
}

pub fn characters(o: &CharOptions) -> Result<Generated> {
    if !(4..=128).contains(&o.length) {
        return Err(CoreError::Invalid("o comprimento vai de 4 a 128".into()));
    }
    let mut classes: Vec<Vec<char>> = Vec::new();
    for (enabled, set) in [(o.lowercase, LOWER), (o.uppercase, UPPER), (o.digits, DIGITS), (o.symbols, SYMBOLS)] {
        if enabled {
            let chars = class_chars(set, o);
            if !chars.is_empty() {
                classes.push(chars);
            }
        }
    }
    if classes.is_empty() {
        return Err(CoreError::Invalid("escolha pelo menos um tipo de caractere".into()));
    }
    let len = o.length as usize;
    if classes.len() > len {
        return Err(CoreError::Invalid("comprimento curto demais para os tipos escolhidos".into()));
    }
    let alphabet: Vec<char> = classes.iter().flatten().copied().collect();

    // Uniform over all strings containing at least one char of each class: sample
    // uniformly, reject strings missing a class. Expected retries are tiny.
    let value = loop {
        let mut s = Zeroizing::new(String::with_capacity(len));
        for _ in 0..len {
            s.push(alphabet[random::below(alphabet.len() as u32) as usize]);
        }
        if classes.iter().all(|cls| s.chars().any(|c| cls.contains(&c))) {
            break SecretString::new(s.as_str());
        }
    };
    Ok(Generated { value, entropy_bits: constrained_entropy(&classes.iter().map(Vec::len).collect::<Vec<_>>(), len) })
}

/// log2 of the number of length-`len` strings over the union of `sizes` classes that use
/// every class at least once (inclusion–exclusion).
fn constrained_entropy(sizes: &[usize], len: usize) -> f64 {
    let n: usize = sizes.iter().sum();
    let k = sizes.len();
    // Work relative to n^len to stay in range: count = n^len * Σ (-1)^|S| ((n-|S|)/n)^len
    let mut ratio = 0.0f64;
    for mask in 0u32..(1 << k) {
        let removed: usize = (0..k).filter(|i| mask & (1 << i) != 0).map(|i| sizes[i]).sum();
        let sign = if mask.count_ones() % 2 == 0 { 1.0 } else { -1.0 };
        ratio += sign * ((n - removed) as f64 / n as f64).powi(len as i32);
    }
    len as f64 * (n as f64).log2() + ratio.max(f64::MIN_POSITIVE).log2()
}

pub fn passphrase(o: &PassphraseOptions) -> Result<Generated> {
    if !(3..=12).contains(&o.words) {
        return Err(CoreError::Invalid("de 3 a 12 palavras".into()));
    }
    if o.separator.chars().count() > 3 {
        return Err(CoreError::Invalid("separador de até 3 caracteres".into()));
    }
    let list = words(o.language);
    let count = o.words as usize;
    let mut picked: Vec<Zeroizing<String>> = (0..count)
        .map(|_| {
            let w = list[random::below(list.len() as u32) as usize];
            let mut w = w.to_string();
            if o.capitalize {
                let mut cs = w.chars();
                if let Some(first) = cs.next() {
                    w = first.to_uppercase().collect::<String>() + cs.as_str();
                }
            }
            Zeroizing::new(w)
        })
        .collect();
    let mut entropy = count as f64 * (list.len() as f64).log2();
    if o.include_number {
        let at = random::below(count as u32) as usize;
        let digit = random::below(10);
        picked[at].push_str(&digit.to_string());
        entropy += (10f64).log2() + (count as f64).log2();
    }
    let joined = Zeroizing::new(picked.iter().map(|w| w.as_str()).collect::<Vec<_>>().join(&o.separator));
    Ok(Generated { value: SecretString::new(joined.as_str()), entropy_bits: entropy })
}

fn is_trivial_pin(p: &str) -> bool {
    let d: Vec<i32> = p.chars().filter_map(|c| c.to_digit(10)).map(|v| v as i32).collect();
    if d.windows(2).all(|w| w[0] == w[1]) {
        return true;
    }
    let asc = d.windows(2).all(|w| w[1] - w[0] == 1);
    let desc = d.windows(2).all(|w| w[0] - w[1] == 1);
    asc || desc || matches!(p, "1212" | "1122" | "2580" | "0852" | "1004" | "2000" | "6969" | "1313")
}

pub fn pin(length: u32) -> Result<Generated> {
    if !(4..=12).contains(&length) {
        return Err(CoreError::Invalid("o PIN vai de 4 a 12 dígitos".into()));
    }
    let value = loop {
        let s: String = (0..length).map(|_| char::from(b'0' + random::below(10) as u8)).collect();
        if !is_trivial_pin(&s) {
            break SecretString::new(s);
        }
    };
    // Rejected trivial PINs are a negligible fraction; report the plain space.
    Ok(Generated { value, entropy_bits: length as f64 * (10f64).log2() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn characters_respect_options() {
        for _ in 0..200 {
            let g = characters(&CharOptions::default()).unwrap();
            let v = g.value.expose();
            assert_eq!(v.chars().count(), 20);
            assert!(v.chars().any(|c| c.is_ascii_lowercase()));
            assert!(v.chars().any(|c| c.is_ascii_uppercase()));
            assert!(v.chars().any(|c| c.is_ascii_digit()));
            assert!(v.chars().any(|c| SYMBOLS.contains(c)));
            assert!(!v.chars().any(|c| AMBIGUOUS.contains(c)));
        }
        let only_digits = CharOptions { lowercase: false, uppercase: false, symbols: false, exclude: "9".into(), ..Default::default() };
        let g = characters(&only_digits).unwrap();
        assert!(g.value.expose().chars().all(|c| c.is_ascii_digit() && c != '9'));
        assert!(characters(&CharOptions { lowercase: false, uppercase: false, digits: false, symbols: false, ..Default::default() }).is_err());
    }

    #[test]
    fn entropy_is_sane() {
        // Single class: exactly len * log2(n).
        assert!((constrained_entropy(&[26], 10) - 10.0 * 26f64.log2()).abs() < 1e-9);
        // Constraints only remove a little entropy.
        let full = 20.0 * 72f64.log2();
        let e = constrained_entropy(&[24, 24, 8, 16], 20);
        assert!(e < full && e > full - 1.0, "{e} vs {full}");
        // Default settings give well over 100 bits.
        assert!(characters(&CharOptions::default()).unwrap().entropy_bits > 110.0);
    }

    #[test]
    fn passphrases() {
        assert_eq!(words(WordLanguage::Pt).len(), 2048);
        assert_eq!(words(WordLanguage::En).len(), 7776);
        let g = passphrase(&PassphraseOptions::default()).unwrap();
        let v = g.value.expose();
        assert_eq!(v.split('-').count(), 5);
        assert!(v.chars().any(|c| c.is_ascii_digit()));
        assert!(v.split('-').all(|w| w.chars().next().unwrap().is_uppercase()));
        assert!((g.entropy_bits - (55.0 + 10f64.log2() + 5f64.log2())).abs() < 1e-9);
    }

    #[test]
    fn pins_avoid_trivial_sequences() {
        assert!(is_trivial_pin("1234") && is_trivial_pin("0000") && is_trivial_pin("9876"));
        assert!(!is_trivial_pin("4071"));
        for _ in 0..200 {
            let p = pin(4).unwrap();
            assert!(!is_trivial_pin(p.value.expose()));
        }
    }
}
