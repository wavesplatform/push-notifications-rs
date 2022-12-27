use regex::{Captures, Regex};
use std::borrow::Cow;
use std::collections::HashMap;

pub(super) fn interpolate(s: &str, subst: &HashMap<&str, &str>) -> String {
    let re = Regex::new(r#"\[%s:([a-zA-z]+)]"#).expect("regex");
    re.replace_all(s, |caps: &Captures| {
        let key = caps.get(1).expect("regex capture").as_str();
        subst
            .get(key)
            .map(|s| Cow::Borrowed(*s))
            .unwrap_or_else(|| Cow::Owned(format!("<{}>", key)))
    })
    .to_string()
}

#[test]
fn test_interpolate() {
    let subst = HashMap::from([("foo", "bar"), ("fee", "baz")]);
    assert_eq!(&interpolate("", &subst), "");
    assert_eq!(&interpolate("[%s:foo]", &subst), "bar");
    assert_eq!(&interpolate("[%s:foo] bar", &subst), "bar bar");
    assert_eq!(&interpolate("[%s:foo] [%s:fee]", &subst), "bar baz");
    assert_eq!(&interpolate("[%s:foo] [%s:foo]", &subst), "bar bar");
    assert_eq!(
        &interpolate("[%s:foo] [%s:fee] [%s:foo]", &subst),
        "bar baz bar"
    );
    assert_eq!(&interpolate("[%s:unknown]", &subst), "<unknown>");
}
