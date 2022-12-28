use super::lokalise_gateway::dto::KeysResponse;
use crate::model::Lang;
use std::{
    collections::{BTreeSet, HashMap},
    fmt,
};

pub(super) type Key = String;
pub(super) type Value = String;
pub(super) type ValuesMap = HashMap<Lang, Value>;

pub(super) struct TranslationMap(HashMap<Key, ValuesMap>);

impl TranslationMap {
    pub(super) fn build(keys: KeysResponse) -> Self {
        let mut translations = HashMap::<Key, ValuesMap>::new();
        for key in keys.keys {
            let key_name = key.key_name.web;

            if let Some(t) = key.translations {
                for tr in t {
                    translations
                        .entry(key_name.clone())
                        .or_default()
                        .insert(tr.language_iso, tr.translation);
                }
            }
        }
        TranslationMap(translations)
    }

    pub(super) fn is_complete(&self) -> bool {
        let TranslationMap(translations) = self;
        let keys = self.keys();
        let langs = self.langs();
        for lang in &langs {
            for key in &keys {
                let has_value = translations
                    .get(key)
                    .map(|values| values.get(lang))
                    .flatten()
                    .is_some();
                if !has_value {
                    return false;
                }
            }
        }
        true
    }

    fn keys(&self) -> BTreeSet<Key> {
        let TranslationMap(translations) = self;
        translations.keys().map(String::to_owned).collect()
    }

    fn langs(&self) -> BTreeSet<Lang> {
        let TranslationMap(translations) = self;
        translations
            .values()
            .map(|values| values.keys())
            .flatten()
            .map(String::to_owned)
            .collect()
    }

    pub(super) fn translate(&self, key: &str, lang: &str) -> Option<&Value> {
        let TranslationMap(translations) = self;
        translations[key].get(lang)
    }
}

impl fmt::Debug for TranslationMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Languages: {:?}, Keys: {:?}, Translations: {:?}",
            self.langs(),
            self.keys(),
            self.0,
        )
    }
}
