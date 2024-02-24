use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

use crate::entry::short_name;

use common::constants::ALLIUM_GAMES_DIR;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LazyImage {
    Unknown(PathBuf),
    Found(PathBuf),
    NotFound,
}

impl LazyImage {
    pub fn from_path(path: &Path, image: Option<PathBuf>) -> Self {
        match image {
            Some(image) => Self::Found(image),
            None => Self::Unknown(path.to_path_buf()),
        }
    }

    pub fn image(&mut self) -> Option<&Path> {
        let path = match self {
            Self::Unknown(path) => path,
            Self::Found(path) => return Some(path.as_path()),
            Self::NotFound => return None,
        };

        const IMAGE_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "gif"];

        let mut parent = path.clone();
        let mut image = None;
        let file_name = path.file_name().unwrap();
        'image: while parent.pop() {
            let mut image_path = parent.join("Imgs");
            if image_path.is_dir() {
                image_path.push(file_name);
                for ext in &IMAGE_EXTENSIONS {
                    image_path.set_extension(ext);
                    if image_path.is_file() {
                        image = Some(image_path);
                        break 'image;
                    }
                }
                image_path.pop();
                image_path.extend(path.strip_prefix(&parent).unwrap());
                for ext in &IMAGE_EXTENSIONS {
                    image_path.set_extension(ext);
                    if image_path.is_file() {
                        image = Some(image_path);
                        break 'image;
                    }
                }
            }
            if parent.to_str() == ALLIUM_GAMES_DIR.to_str() {
                break;
            }
        }

        if image.is_none() {
            if let Some(ext) = path.extension().and_then(std::ffi::OsStr::to_str) {
                if IMAGE_EXTENSIONS.contains(&ext) {
                    image = Some(path.clone());
                }
            }
        }

        if image.is_none() {
            let scraper_keyword = path.file_stem().unwrap().to_string_lossy();
            let config_path = parent.join("Imgs").with_file_name("scraper_config.json");
            if let Some(url) = scrape_image(&scraper_keyword, Some(&config_path)) {
                if let Some(file_name) = download_image(&url, &scraper_keyword) {
                    image = Some(file_name);
                }
            }
        }

        *self = match image {
            Some(image) => Self::Found(image),
            None => Self::NotFound,
        };

        match self {
            Self::Found(path) => Some(path.as_path()),
            _ => None,
        }
    }

    pub fn try_image(&self) -> Option<&Path> {
        match self {
            Self::Found(path) => Some(path.as_path()),
            _ => None,
        }
    }
}

fn scrape_image(keyword: &str, config_path: Option<&Path>) -> Option<String> {
    let config: ScraperConfig = config_path
        .and_then(|path| {
            File::open(path)
                .ok()
                .and_then(|file| serde_json::from_reader(file).ok())
        })
        .unwrap_or_else(|| {
            let default_config_path = "scraper_config.json";
            File::open(default_config_path)
                .ok()
                .and_then(|file| serde_json::from_reader(file).ok())
        })?;

    let url = config.url_template.replace("{}", &short_name(keyword));
    let client = Client::new();
    let response = client.get(&url).send().ok()?;
    let body = response.text().ok()?;
    let document = Html::parse_document(&body);

    let box_art_url = document
        .select(&Selector::parse(&config.img_selector).unwrap())
        .next()
        .and_then(|img| img.value().attr("src"))
        .map(|src| {
            let re = Regex::new(&config.regex_pattern).unwrap();
            let replaced_src = re.replace_all(src, &config.replacement_string);
            replaced_src.into_owned()
        });

    box_art_url
}

fn download_image(url: &str, scraper_keyword: &str) -> Option<PathBuf> {
    let client = Client::new();
    let response = client.get(url).send().ok()?;
    let mut file = File::create(scraper_keyword).ok()?;
    let bytes = response.bytes().ok()?;
    file.write_all(&bytes).ok()?;
    Some(PathBuf::from(scraper_keyword))
}

#[derive(Deserialize)]
struct ScraperConfig {
    url_template: String,
    img_selector: String,
    regex_pattern: String,
    replacement_string: String,
}
