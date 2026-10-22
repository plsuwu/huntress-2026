//! Java archives: the `META-INF/MANIFEST.MF` main section and per-entry sections (72-byte continuation
//! lines joined), the class entries, and whether a named class really declares a launcher
//! (`public static void main(String[])`). Nested `*.jar` members are parsed recursively.

use super::jvm;
use super::zip::Zip;

#[derive(Clone, Debug, Default)]
pub struct Manifest {
    /// Main-section attributes in file order (names as written).
    pub main: Vec<(String, String)>,
    /// Per-entry sections: (`Name:` value, attributes).
    pub sections: Vec<(String, Vec<(String, String)>)>,
}

impl Manifest {
    pub fn parse(b: &[u8]) -> Manifest {
        let t = String::from_utf8_lossy(b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)).into_owned();
        let t = t.replace("\r\n", "\n").replace('\r', "\n");
        let mut m = Manifest::default();
        let mut cur: Vec<(String, String)> = Vec::new();
        let mut first = true;
        let flush = |cur: &mut Vec<(String, String)>, first: &mut bool, m: &mut Manifest| {
            if cur.is_empty() {
                return;
            }
            let attrs = std::mem::take(cur);
            if *first {
                m.main = attrs;
                *first = false;
            } else {
                let name = attrs.iter().find(|a| a.0.eq_ignore_ascii_case("Name")).map(|a| a.1.clone()).unwrap_or_default();
                m.sections.push((name, attrs));
            }
        };
        for line in t.split('\n') {
            if line.is_empty() {
                flush(&mut cur, &mut first, &mut m);
            } else if let Some(c) = line.strip_prefix(' ') {
                if let Some(last) = cur.last_mut() {
                    last.1.push_str(c);
                }
            } else if let Some((k, v)) = line.split_once(':') {
                cur.push((k.trim().to_string(), v.strip_prefix(' ').unwrap_or(v).to_string()));
            }
        }
        flush(&mut cur, &mut first, &mut m);
        m
    }

    /// A main-section attribute, name matched case-insensitively (as the JDK does).
    pub fn get(&self, name: &str) -> Option<&str> {
        self.main.iter().find(|a| a.0.eq_ignore_ascii_case(name)).map(|a| a.1.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct ClassEntry {
    /// Entry name (`a/b/C.class`).
    pub path: String,
    /// `this_class` from the class file (`a/b/C`), if it parsed.
    pub this_class: Option<String>,
    /// Declares `public static void main(String[])`.
    pub has_main: bool,
}

#[derive(Clone, Debug)]
pub struct Jar {
    pub manifest: Option<Manifest>,
    pub classes: Vec<ClassEntry>,
    pub members: Vec<String>,
    /// Nested `*.jar` members (name, parsed jar).
    pub nested: Vec<(String, Jar)>,
}

impl Jar {
    pub fn parse(b: &[u8]) -> Option<Jar> {
        Self::parse_depth(b, 0)
    }

    fn parse_depth(b: &[u8], depth: usize) -> Option<Jar> {
        let z = Zip::parse(b).ok()?;
        let mut manifest = None;
        let mut classes = Vec::new();
        let mut nested = Vec::new();
        for e in z.files() {
            let up = e.name.to_ascii_uppercase();
            if up == "META-INF/MANIFEST.MF" {
                manifest = z.read(e, b"").ok().map(|d| Manifest::parse(&d));
            } else if e.name.ends_with(".class") {
                let info = z.read(e, b"").ok().and_then(|d| jvm::class_info(&d));
                let has_main = info.as_ref().is_some_and(|i| i.methods.iter().any(|m| m.0 == "main" && m.1 == "([Ljava/lang/String;)V" && m.2 & 0x0009 == 0x0009));
                classes.push(ClassEntry { path: e.name.clone(), this_class: info.map(|i| i.this_class), has_main });
            } else if depth < 4 && (up.ends_with(".JAR") || up.ends_with(".WAR")) {
                if let Some(j) = z.read(e, b"").ok().and_then(|d| Self::parse_depth(&d, depth + 1)) {
                    nested.push((e.name.clone(), j));
                }
            }
        }
        if manifest.is_none() && classes.is_empty() && nested.is_empty() {
            return None;
        }
        Some(Jar { manifest, classes, members: z.files().map(|e| e.name.clone()).collect(), nested })
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.manifest.as_ref()?.get(name)
    }

    /// The class entry for a binary name (`a.b.C` or `a/b/C`), matched by entry path or `this_class`.
    pub fn class(&self, binary_name: &str) -> Option<&ClassEntry> {
        let internal = binary_name.trim().replace('.', "/");
        let path = format!("{internal}.class");
        self.classes.iter().find(|c| c.path == path).or_else(|| self.classes.iter().find(|c| c.this_class.as_deref() == Some(internal.as_str())))
    }
}
