//! HTML for mod GUIs: a forgiving parser into a node tree. Elements keep their tag and
//! attributes; `<style>` and `<script>` keep their raw text; text nodes are `#text`.

#[derive(Clone, Debug, Default)]
pub struct Node {
    /// Lower-case tag, or `#text`.
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<usize>,
    pub parent: Option<usize>,
    /// Text of a `#text` node (and the raw body of `<style>` / `<script>`).
    pub text: String,
    /// Inputs: the typed text.
    pub value: String,
    /// `style.x = …` from scripts (CSS property names).
    pub js_style: Vec<(String, String)>,
    pub hidden: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Dom {
    /// Node 0 is the document root.
    pub nodes: Vec<Node>,
}

const VOID: &[&str] = &["br", "hr", "img", "input", "meta", "link"];
const RAW: &[&str] = &["style", "script"];

fn entities(s: &str) -> String {
    s.replace("&nbsp;", "\u{a0}").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

impl Dom {
    pub fn parse(src: &str) -> Dom {
        let mut d = Dom { nodes: vec![Node { tag: "#root".into(), ..Default::default() }] };
        let c: Vec<char> = src.chars().collect();
        let mut stack = vec![0usize];
        let mut i = 0;
        let mut text = String::new();
        while i < c.len() {
            if c[i] == '<' {
                // Comment.
                if c[i..].starts_with(&['<', '!', '-', '-']) {
                    d.flush(&mut text, &stack);
                    i += 4;
                    while i + 2 < c.len() && !(c[i] == '-' && c[i + 1] == '-' && c[i + 2] == '>') {
                        i += 1;
                    }
                    i += 3;
                    continue;
                }
                // Doctype and friends.
                if c.get(i + 1) == Some(&'!') || c.get(i + 1) == Some(&'?') {
                    while i < c.len() && c[i] != '>' {
                        i += 1;
                    }
                    i += 1;
                    continue;
                }
                let closing = c.get(i + 1) == Some(&'/');
                let mut j = i + 1 + closing as usize;
                let s = j;
                while j < c.len() && (c[j].is_alphanumeric() || c[j] == '-') {
                    j += 1;
                }
                if j == s {
                    text.push('<');
                    i += 1;
                    continue;
                }
                d.flush(&mut text, &stack);
                let tag: String = c[s..j].iter().collect::<String>().to_lowercase();
                // Attributes.
                let mut attrs = Vec::new();
                let mut self_close = false;
                loop {
                    while j < c.len() && c[j].is_whitespace() {
                        j += 1;
                    }
                    if j >= c.len() {
                        break;
                    }
                    if c[j] == '>' {
                        j += 1;
                        break;
                    }
                    if c[j] == '/' {
                        self_close = true;
                        j += 1;
                        continue;
                    }
                    let ks = j;
                    while j < c.len() && !c[j].is_whitespace() && c[j] != '=' && c[j] != '>' && c[j] != '/' {
                        j += 1;
                    }
                    let key: String = c[ks..j].iter().collect::<String>().to_lowercase();
                    let mut val = String::new();
                    if j < c.len() && c[j] == '=' {
                        j += 1;
                        if j < c.len() && (c[j] == '"' || c[j] == '\'') {
                            let q = c[j];
                            j += 1;
                            while j < c.len() && c[j] != q {
                                val.push(c[j]);
                                j += 1;
                            }
                            j += 1;
                        } else {
                            while j < c.len() && !c[j].is_whitespace() && c[j] != '>' {
                                val.push(c[j]);
                                j += 1;
                            }
                        }
                    }
                    if key.is_empty() {
                        j += 1;
                    } else {
                        attrs.push((key, entities(&val)));
                    }
                }
                i = j;
                if closing {
                    if let Some(k) = stack.iter().rposition(|n| d.nodes[*n].tag == tag) {
                        if k > 0 {
                            stack.truncate(k);
                        }
                    }
                    continue;
                }
                let parent = *stack.last().unwrap();
                let id = d.push(Node { tag: tag.clone(), attrs, parent: Some(parent), ..Default::default() });
                if RAW.contains(&tag.as_str()) {
                    let end: Vec<char> = format!("</{tag}").chars().collect();
                    let s = i;
                    while i < c.len() {
                        if c.len() - i >= end.len() && c[i..i + end.len()].iter().zip(&end).all(|(a, b)| a.to_ascii_lowercase() == *b) {
                            break;
                        }
                        i += 1;
                    }
                    d.nodes[id].text = c[s..i.min(c.len())].iter().collect();
                    while i < c.len() && c[i] != '>' {
                        i += 1;
                    }
                    i += 1;
                } else if !self_close && !VOID.contains(&tag.as_str()) {
                    stack.push(id);
                }
                if tag == "input" {
                    d.nodes[id].value = d.attr(id, "value").unwrap_or_default();
                }
            } else {
                text.push(c[i]);
                i += 1;
            }
        }
        d.flush(&mut text, &stack);
        d
    }

    fn push(&mut self, n: Node) -> usize {
        let p = n.parent;
        self.nodes.push(n);
        let id = self.nodes.len() - 1;
        if let Some(p) = p {
            self.nodes[p].children.push(id);
        }
        id
    }

    fn flush(&mut self, text: &mut String, stack: &[usize]) {
        if text.is_empty() {
            return;
        }
        let t = entities(text);
        text.clear();
        // Whitespace between tags is dropped; runs of spaces inside text become one.
        if t.trim().is_empty() {
            return;
        }
        let mut s = String::new();
        let mut space = false;
        for ch in t.chars() {
            if ch.is_whitespace() && ch != '\u{a0}' {
                space = true;
            } else {
                if space && !s.is_empty() {
                    s.push(' ');
                }
                if space && s.is_empty() && t.starts_with(char::is_whitespace) {
                    s.push(' ');
                }
                space = false;
                s.push(ch);
            }
        }
        if space {
            s.push(' ');
        }
        let parent = *stack.last().unwrap();
        self.push(Node { tag: "#text".into(), text: s, parent: Some(parent), ..Default::default() });
    }

    pub fn attr(&self, n: usize, k: &str) -> Option<String> {
        self.nodes.get(n)?.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone())
    }

    pub fn set_attr(&mut self, n: usize, k: &str, v: &str) {
        let a = &mut self.nodes[n].attrs;
        match a.iter_mut().find(|(x, _)| x == k) {
            Some(slot) => slot.1 = v.to_string(),
            None => a.push((k.to_string(), v.to_string())),
        }
    }

    pub fn classes(&self, n: usize) -> Vec<String> {
        self.attr(n, "class").unwrap_or_default().split_whitespace().map(String::from).collect()
    }

    pub fn by_id(&self, id: &str) -> Option<usize> {
        (0..self.nodes.len()).find(|&n| self.attr(n, "id").as_deref() == Some(id))
    }

    /// `#id`, `.class` or `tag`: the first match.
    pub fn query(&self, q: &str) -> Option<usize> {
        let q = q.trim();
        if let Some(id) = q.strip_prefix('#') {
            return self.by_id(id);
        }
        if let Some(c) = q.strip_prefix('.') {
            return (0..self.nodes.len()).find(|&n| self.classes(n).iter().any(|x| x == c));
        }
        (0..self.nodes.len()).find(|&n| self.nodes[n].tag == q)
    }

    /// All the text under a node.
    pub fn text_of(&self, n: usize) -> String {
        let node = &self.nodes[n];
        if node.tag == "#text" {
            return node.text.clone();
        }
        let s: String = node.children.iter().map(|c| self.text_of(*c)).collect();
        s.trim().to_string()
    }

    /// Replaces a node's children with one text node.
    pub fn set_text(&mut self, n: usize, t: &str) {
        let kids = std::mem::take(&mut self.nodes[n].children);
        for k in kids {
            self.nodes[k].parent = None;
        }
        self.push(Node { tag: "#text".into(), text: t.to_string(), parent: Some(n), ..Default::default() });
    }

    /// The first element with this tag (`title`, `body`…).
    pub fn find_tag(&self, tag: &str) -> Option<usize> {
        (0..self.nodes.len()).find(|&n| self.nodes[n].tag == tag && self.attached(n))
    }

    /// Still in the tree (not replaced by `textContent = …`).
    pub fn attached(&self, mut n: usize) -> bool {
        while n != 0 {
            match self.nodes[n].parent {
                Some(p) => n = p,
                None => return false,
            }
        }
        true
    }

    pub fn all_text_of(&self, tag: &str) -> Vec<String> {
        (0..self.nodes.len()).filter(|&n| self.nodes[n].tag == tag && self.attached(n)).map(|n| self.nodes[n].text.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_parses_tags_attributes_text_and_raw_blocks() {
        let d = Dom::parse("<!DOCTYPE html><html><head><title>Пульт</title><style>p { color: red; }</style></head><body><div id=main class='a b'>Привет, <b>мир</b>!<br><input id=n value=\"5\"></div><script>let x = 1 < 2;</script></body></html>");
        let main = d.by_id("main").unwrap();
        assert_eq!(d.classes(main), vec!["a", "b"]);
        assert_eq!(d.text_of(main), "Привет, мир!");
        assert_eq!(d.text_of(d.find_tag("title").unwrap()), "Пульт");
        assert_eq!(d.all_text_of("style"), vec!["p { color: red; }"]);
        assert_eq!(d.all_text_of("script"), vec!["let x = 1 < 2;"]);
        assert_eq!(d.nodes[d.by_id("n").unwrap()].value, "5");
        assert_eq!(d.query(".b"), Some(main));
    }
}
