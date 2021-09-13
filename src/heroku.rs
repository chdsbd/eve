use reqwest::header::{ACCEPT, AUTHORIZATION, RANGE};
use serde::Deserialize;

#[derive(Debug)]
pub enum HerokuError {
    HttpError(reqwest::Error),
}

impl std::convert::From<reqwest::Error> for HerokuError {
    fn from(e: reqwest::Error) -> Self {
        Self::HttpError(e)
    }
}

#[derive(Deserialize)]
pub struct HerokuReleaseSlug {
    pub id: String,
}
#[derive(Deserialize)]
pub struct HerokuRelease {
    pub slug: HerokuReleaseSlug,
}

pub fn get_release(app: &str, release: i64, token: &str) -> Result<HerokuRelease, HerokuError> {
    let res = reqwest::blocking::Client::new()
        .get(&format!(
            "https://api.heroku.com/apps/{app}/releases/{release}",
            app = app,
            release = release
        ))
        .header("User-Agent", "chdsbd/eve")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(ACCEPT, "application/vnd.heroku+json; version=3")
        .header(RANGE, "version; order=desc")
        .send()?;
    res.error_for_status_ref()?;
    Ok(res.json::<HerokuRelease>()?)
}
#[derive(Deserialize)]
pub struct HerokuSlug {
    pub commit: String,
}

pub fn get_slug(app: &str, slug: &str, token: &str) -> Result<HerokuSlug, HerokuError> {
    let res = reqwest::blocking::Client::new()
        .get(&format!(
            "https://api.heroku.com/apps/{app}/slugs/{slug}",
            app = app,
            slug = slug
        ))
        .header("User-Agent", "chdsbd/eve")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(ACCEPT, "application/vnd.heroku+json; version=3")
        .send()?;
    res.error_for_status_ref()?;
    Ok(res.json::<HerokuSlug>()?)
}
