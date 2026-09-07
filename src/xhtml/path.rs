//! Pure href and path syntax operations shared by EPUB and KF8 stages.
//!
//! Functions here normalize paths, resolve relative references, and identify
//! external references. EPUB semantic and KF8 flow decisions belong to callers.

pub(crate) fn resolve_path(base_href: &str, target: &str) -> Option<String> {
    let target = target.split(['#', '?']).next().unwrap_or(target);
    if target.is_empty() || is_external_reference(target) {
        return None;
    }
    let base = if target.starts_with('/') {
        target.to_owned()
    } else if let Some((directory, _)) = base_href.rsplit_once('/') {
        format!("{directory}/{target}")
    } else {
        target.to_owned()
    };
    normalize_path(&base)
}

pub(crate) fn normalize_path(path: &str) -> Option<String> {
    let normalized = normalize_path_lossy(path);
    (!normalized.is_empty()).then_some(normalized)
}

pub(crate) fn normalize_path_lossy(path: &str) -> String {
    let mut components = Vec::new();
    let normalized = path.replace('\\', "/");
    for component in normalized.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            value => components.push(value),
        }
    }
    components.join("/")
}

pub(crate) fn is_external_reference(target: &str) -> bool {
    target.starts_with("data:")
        || target.starts_with("//")
        || target.contains("://")
        || target.starts_with("kindle:")
}
