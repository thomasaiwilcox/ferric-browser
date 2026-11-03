//! Prefer distribution-maintained suffix data, with a release-packaged fallback.
//! Neither location nor data is supplied by the page or private request pipe.

use libwebauthn::ops::webauthn::{
    DatFilePublicSuffixList, PublicSuffixList, SystemPublicSuffixList,
};
use std::path::Path;

use crate::errors::AdmissionError;

pub(crate) enum SuffixList {
    System(SystemPublicSuffixList),
    Packaged(DatFilePublicSuffixList),
}

impl PublicSuffixList for SuffixList {
    fn public_suffix(&self, host: &str) -> Option<String> {
        match self {
            Self::System(list) => list.public_suffix(host),
            Self::Packaged(list) => list.public_suffix(host),
        }
    }

    fn registrable_domain(&self, host: &str) -> Option<String> {
        match self {
            Self::System(list) => list.registrable_domain(host),
            Self::Packaged(list) => list.registrable_domain(host),
        }
    }
}

pub(crate) fn load() -> Result<SuffixList, AdmissionError> {
    if let Ok(list) = SystemPublicSuffixList::auto() {
        return Ok(SuffixList::System(list));
    }
    let executable =
        std::env::current_exe().map_err(|_| AdmissionError::PublicSuffixListUnavailable)?;
    let prefix = executable
        .parent()
        .and_then(Path::parent)
        .ok_or(AdmissionError::PublicSuffixListUnavailable)?;
    load_packaged(&prefix.join("share/ferric-browser/public_suffix_list.dat"))
}

fn load_packaged(path: &Path) -> Result<SuffixList, AdmissionError> {
    DatFilePublicSuffixList::from_path(path)
        .map(SuffixList::Packaged)
        .map_err(|_| AdmissionError::PublicSuffixListUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_official_list_preserves_private_and_multilabel_boundaries() {
        let list = load_packaged(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/data/public_suffix_list.dat"
        )))
        .unwrap();
        assert_eq!(
            list.registrable_domain("login.example.com").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            list.registrable_domain("login.example.co.uk").as_deref(),
            Some("example.co.uk")
        );
        assert_eq!(
            list.registrable_domain("tenant.github.io").as_deref(),
            Some("tenant.github.io")
        );
        assert_eq!(list.registrable_domain("github.io"), None);
        assert_eq!(list.registrable_domain("com"), None);
        assert!(matches!(
            load_packaged(Path::new("/nonexistent-ferric-fixture/psl.dat")),
            Err(AdmissionError::PublicSuffixListUnavailable)
        ));
    }
}
