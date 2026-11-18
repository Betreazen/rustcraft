//! A status as the answers and the journal write it: the game's text component (JSON) the
//! player reads, as its translation keys (which the language files map) and as plain English from
//! its fallbacks.

use serde_json::Value;

/// Every translation key of the component, depth first: its own, its arguments', its siblings'.
pub fn keys(c: &Value) -> Vec<String> {
    let mut out = Vec::new();
    collect(c, &mut out);
    out
}

fn collect(c: &Value, out: &mut Vec<String>) {
    match c {
        Value::Object(o) => {
            if let Some(Value::String(k)) = o.get("translate") {
                out.push(k.clone());
            }
            for part in ["with", "extra"] {
                if let Some(Value::Array(a)) = o.get(part) {
                    a.iter().for_each(|v| collect(v, out));
                }
            }
        }
        Value::Array(a) => a.iter().for_each(|v| collect(v, out)),
        _ => {}
    }
}

/// The component as plain text: literals as they are, a translation as its fallback (its key
/// when it has none) with its arguments put in.
pub fn plain(c: &Value) -> String {
    match c {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Array(a) => a.iter().map(plain).collect(),
        Value::Object(o) => {
            let mut s = match o.get("translate") {
                Some(Value::String(key)) => {
                    let template = o.get("fallback").and_then(Value::as_str).unwrap_or(key);
                    let args: Vec<String> = match o.get("with") {
                        Some(Value::Array(a)) => a.iter().map(plain).collect(),
                        _ => Vec::new(),
                    };
                    fill(template, &args)
                }
                _ => o.get("text").map(plain).unwrap_or_default(),
            };
            if let Some(Value::Array(extra)) = o.get("extra") {
                extra.iter().for_each(|e| s.push_str(&plain(e)));
            }
            s
        }
        _ => String::new(),
    }
}

/// `%s` and `%1$s` in `template` filled from `args`, as the game formats a translation; `%%` is
/// a percent sign.
pub fn fill(template: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut next = 0;
    let mut rest = template;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if let Some(tail) = after.strip_prefix('%') {
            out.push('%');
            rest = tail;
        } else if let Some(tail) = after.strip_prefix('s') {
            out.push_str(args.get(next).map_or("", String::as_str));
            next += 1;
            rest = tail;
        } else if let Some((n, tail)) = after.split_once("$s")
            && let Ok(n) = n.parse::<usize>()
        {
            out.push_str(args.get(n.saturating_sub(1)).map_or("", String::as_str));
            rest = tail;
        } else {
            out.push('%');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// What the status-change tracker compares: the keys, not the numbers in them, so a progress
/// count does not make a change every second. A status without a key is its plain text.
pub fn signature(c: &Value) -> String {
    let k = keys(c);
    if k.is_empty() { plain(c) } else { k.join("+") }
}

/// The status as an answer writes it: `keys | plain text`.
pub fn status_line(c: &Value) -> String {
    let k = keys(c);
    let p = plain(c);
    if k.is_empty() { p } else { format!("{} | {p}", k.join(", ")) }
}
