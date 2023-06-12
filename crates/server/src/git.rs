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
    pub fn build(&self, git: &Git) -> Option<crd::Git> {
        Some(crd::Git {
            repository: git.repository.clone(),
            branch: git.branch.clone(),
            image: self.image.clone(),
            period: self.period.clone(),
        })
    }
}
