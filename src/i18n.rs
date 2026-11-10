//! English and Italian UI strings, embedded from `locales/*.txt`.

use std::collections::HashMap;

const EN: &str = include_str!("../locales/en.txt");
const IT: &str = include_str!("../locales/it.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    It,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::It];

    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::It => "Italiano",
        }
    }

    /// Maps a locale such as "it", "it-IT" or "it_IT.UTF-8" to a language; anything else is English.
    pub fn from_locale(locale: Option<&str>) -> Lang {
        let primary = locale.and_then(|l| l.split(['-', '_', '.']).next());
        match primary {
            Some(p) if p.eq_ignore_ascii_case("it") => Lang::It,
            _ => Lang::En,
        }
    }

    pub fn detect() -> Lang {
        Lang::from_locale(sys_locale::get_locale().as_deref())
    }
}

type Table = HashMap<&'static str, String>;

fn parse(src: &'static str) -> Table {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim().replace("\\n", "\n")))
        .collect()
}

pub struct Tr {
    lang: Lang,
    current: Table,
    english: Table,
}

impl Tr {
    pub fn new(lang: Lang) -> Self {
        let english = parse(EN);
        let current = match lang {
            Lang::En => english.clone(),
            Lang::It => parse(IT),
        };
        Tr {
            lang,
            current,
            english,
        }
    }

    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Looks up `key`; falls back to English, then to the key itself.
    pub fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.current
            .get(key)
            .or_else(|| self.english.get(key))
            .map_or(key, String::as_str)
    }

    /// Like `t`, replacing each `{name}` placeholder with its value.
    pub fn tf(&self, key: &str, args: &[(&str, &str)]) -> String {
        args.iter()
            .fold(self.t(key).to_string(), |s, (name, value)| {
                s.replace(&format!("{{{name}}}"), value)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn from_locale_detects_italian() {
        for l in ["it", "it-IT", "it_IT.UTF-8"] {
            assert_eq!(Lang::from_locale(Some(l)), Lang::It, "{l}");
        }
        for l in [Some("en-US"), Some("fr"), None] {
            assert_eq!(Lang::from_locale(l), Lang::En);
        }
    }

    #[test]
    fn every_english_key_exists_in_italian() {
        let keys = |t: Table| t.into_keys().collect::<BTreeSet<_>>();
        assert_eq!(keys(parse(EN)), keys(parse(IT)));
    }

    #[test]
    fn placeholders_match_between_languages() {
        let (en, it) = (parse(EN), parse(IT));
        let ph = |s: &str| {
            s.split('{')
                .skip(1)
                .filter_map(|p| p.split_once('}').map(|(n, _)| n.to_string()))
                .collect::<BTreeSet<_>>()
        };
        for (k, v) in &en {
            assert_eq!(ph(v), ph(&it[k]), "{k}");
        }
    }

    #[test]
    fn missing_key_falls_back() {
        assert_eq!(Tr::new(Lang::It).t("does.not.exist"), "does.not.exist");
    }

    #[test]
    fn tf_replaces_placeholders() {
        let s = Tr::new(Lang::En).tf("range.out_of_range", &[("page", "12"), ("total", "10")]);
        assert_eq!(s, "Page 12 does not exist (total 10)");
    }
}
