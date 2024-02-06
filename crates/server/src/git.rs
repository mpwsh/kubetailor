use kubetailor::crd;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Git {
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub image: Option<String>,
    pub period: Option<String>,
    pub username: Option<String>,
    pub token: Option<String>,
}

impl Git {
    pub fn build(
        &self,
        repository: Option<String>,
        branch: Option<String>,
        username: Option<String>,
        token: Option<String>,
    ) -> Option<crd::Git> {
        Some(crd::Git {
            repository,
            branch,
            image: self.image.clone(),
            period: self.period.clone(),
            username,
            token,
        })
    }
}
