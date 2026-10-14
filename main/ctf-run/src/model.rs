//! Domain types shared by the runner and the lanes: modes, questions, deliveries, attempts, answers.

use crate::json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Single,
    TimeTrial,
    Mastery,
}

impl Mode {
    /// Splits `id[/time_trial|/mastery]` into the base id and the mode.
    pub fn split(challenge: &str) -> (&str, Mode) {
        if let Some(b) = challenge.strip_suffix("/time_trial") {
            (b, Mode::TimeTrial)
        } else if let Some(b) = challenge.strip_suffix("/mastery") {
            (b, Mode::Mastery)
        } else {
            (challenge, Mode::Single)
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Mode::Single => "",
            Mode::TimeTrial => "/time_trial",
            Mode::Mastery => "/mastery",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Single => "single_solve",
            Mode::TimeTrial => "time_trial",
            Mode::Mastery => "mastery",
        }
    }
}

/// The day number embedded in a challenge id (`...-day-13-...`, `warmups-day-07`).
pub fn day_of(challenge: &str) -> Option<u8> {
    let i = challenge.find("day-")? + 4;
    let digits: String = challenge[i..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Everything a lane may need to know about the run, besides the questions and bytes.
#[derive(Clone, Debug)]
pub struct RunCtx {
    /// Challenge string as sent (with mode suffix).
    pub challenge: String,
    /// Challenge id without the mode suffix.
    pub base: String,
    pub day: Option<u8>,
    pub mode: Mode,
}

impl RunCtx {
    pub fn new(challenge: &str) -> RunCtx {
        let (base, mode) = Mode::split(challenge);
        RunCtx { challenge: challenge.to_string(), base: base.to_string(), day: day_of(base), mode }
    }
}

/// One entry of the start response's `questions`.
#[derive(Clone, Debug)]
pub struct Question {
    /// The answer id to submit under (`set2.python-x-role`).
    pub id: String,
    /// Mastery set number from a `setN.` prefix.
    pub set: Option<u32>,
    /// The id without its `setN.` prefix: what kind tables match on.
    pub key: String,
    pub question: String,
    /// The artifact label: `name`, `delivery N: name`, `archive.zip -> member`.
    pub artifact: Option<String>,
    pub format_kind: Option<String>,
    pub format_syntax: Option<String>,
    pub normalization: Option<String>,
    pub raw: Value,
}

impl Question {
    pub fn from_json(v: &Value) -> Option<Question> {
        let id = v.str_of("id")?.to_string();
        let (set, key) = split_set(&id);
        let fmt = v.get("answer_format");
        let s = |o: Option<&Value>, k: &str| o.and_then(|o| o.str_of(k)).map(str::to_string);
        Some(Question {
            set,
            key: key.to_string(),
            question: v.str_of("question").unwrap_or("").to_string(),
            artifact: s(Some(v), "artifact"),
            format_kind: s(fmt, "kind"),
            format_syntax: s(fmt, "syntax"),
            normalization: s(Some(v), "normalization"),
            raw: v.clone(),
            id,
        })
    }

    /// `delivery N: name` -> (N, name).
    pub fn delivery_label(&self) -> Option<(usize, &str)> {
        let rest = self.artifact.as_deref()?.strip_prefix("delivery ")?;
        let (n, name) = rest.split_once(':')?;
        Some((n.trim().parse().ok()?, name.trim()))
    }

    /// The artifact label with any `delivery N:` prefix removed, split on ` -> `.
    pub fn artifact_path(&self) -> Vec<&str> {
        let label = match self.delivery_label() {
            Some((_, name)) => name,
            None => self.artifact.as_deref().unwrap_or(""),
        };
        label.split(" -> ").map(str::trim).filter(|s| !s.is_empty()).collect()
    }
}

/// `set12.foo-bar` -> (Some(12), "foo-bar").
pub fn split_set(id: &str) -> (Option<u32>, &str) {
    if let Some(rest) = id.strip_prefix("set") {
        if let Some((n, key)) = rest.split_once('.') {
            if let Ok(n) = n.parse() {
                return (Some(n), key);
            }
        }
    }
    (None, id)
}

/// A download the start response offers.
#[derive(Clone, Debug)]
pub struct DeliverySpec {
    /// 0-based position in the normalised delivery list.
    pub index: usize,
    pub link: String,
    pub cap: String,
    pub filename: Option<String>,
    pub sha256: Option<String>,
    pub kind: Option<String>,
    pub size: Option<u64>,
}

/// A downloaded delivery.
pub struct Delivery {
    pub spec: DeliverySpec,
    /// File name (Content-Disposition, else the spec's filename, else `delivery-N.bin`).
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Answers in insertion order.
#[derive(Clone, Debug, Default)]
pub struct Answers(Vec<(String, String)>);

impl Answers {
    pub fn new() -> Answers {
        Answers(Vec::new())
    }

    pub fn set(&mut self, id: impl Into<String>, value: impl Into<String>) {
        let (id, value) = (id.into(), value.into());
        match self.0.iter_mut().find(|(k, _)| *k == id) {
            Some(slot) => slot.1 = value,
            None => self.0.push((id, value)),
        }
    }

    pub fn get(&self, id: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == id).map(|(_, v)| v.as_str())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn merge(&mut self, other: &Answers) {
        for (k, v) in other.iter() {
            self.set(k, v);
        }
    }

    /// Only the answers whose ids satisfy `keep`.
    pub fn filter(&self, keep: impl Fn(&str) -> bool) -> Answers {
        Answers(self.0.iter().filter(|(k, _)| keep(k)).cloned().collect())
    }

    pub fn to_value(&self) -> Value {
        Value::Obj(self.0.iter().map(|(k, v)| (k.clone(), Value::Str(v.clone()))).collect())
    }

    pub fn from_value(v: &Value) -> Option<Answers> {
        let mut a = Answers::new();
        for (k, v) in v.as_object()? {
            a.set(k.clone(), v.as_str()?.to_string());
        }
        Some(a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_modes() {
        assert_eq!(split_set("set12.python-x"), (Some(12), "python-x"));
        assert_eq!(split_set("settle.x"), (None, "settle.x"));
        assert_eq!(split_set("answer"), (None, "answer"));
        assert_eq!(Mode::split("a-day-03-b/mastery"), ("a-day-03-b", Mode::Mastery));
        assert_eq!(day_of("warmups-day-07"), Some(7));
        assert_eq!(day_of("binary-reverse-engineering-day-13-crystal"), Some(13));
        let q = Question::from_json(
            &Value::parse(r#"{"id":"set2.answer","artifact":"delivery 2: x.apk"}"#).unwrap(),
        )
        .unwrap();
        assert_eq!((q.set, q.key.as_str(), q.delivery_label()), (Some(2), "answer", Some((2, "x.apk"))));
        let q = Question::from_json(&Value::parse(r#"{"id":"a","artifact":"e-01.zip -> c.jpg"}"#).unwrap()).unwrap();
        assert_eq!(q.artifact_path(), ["e-01.zip", "c.jpg"]);
    }
}
