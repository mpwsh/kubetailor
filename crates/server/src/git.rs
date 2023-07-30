use kubetailor::crd;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Git {
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub image: Option<String>,
    pub period: Option<String>,
}

impl Git {
    pub fn build(&self, repository: &str, branch: &str) -> Option<crd::Git> {
        Some(crd::Git {
            repository: Some(repository.into()),
            branch: Some(branch.into()),
            image: self.image.clone(),
            period: self.period.clone(),
        })
    }
}
