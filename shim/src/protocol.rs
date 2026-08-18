use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Msg {
    Send { id: String, text: String },
    Ack { id: String },
    Error { id: String, reason: String },
}

pub fn parse(line: &str) -> Result<Msg, String> {
    serde_json::from_str(line.trim()).map_err(|e| e.to_string())
}

pub fn to_line(msg: &Msg) -> String {
    let mut s = serde_json::to_string(msg).expect("serialize");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_all_variants() {
        for m in [
            Msg::Send { id: "1".into(), text: "hi\nthere".into() },
            Msg::Ack { id: "1".into() },
            Msg::Error { id: "1".into(), reason: "user-typing".into() },
        ] {
            let line = to_line(&m);
            assert!(line.ends_with('\n'));
            assert_eq!(parse(&line).unwrap(), m);
        }
    }

    #[test]
    fn send_wire_format_matches_spec() {
        let line = to_line(&Msg::Send { id: "a".into(), text: "t".into() });
        assert_eq!(line.trim(), r#"{"type":"send","id":"a","text":"t"}"#);
    }

    #[test]
    fn bad_json_is_err() {
        assert!(parse("not json\n").is_err());
    }
}
