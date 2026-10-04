use futures::channel::{mpsc, oneshot};
use futures::StreamExt;
use gpui_kit::component::notification::NotificationType;
use gpui_kit::{Anchor, Context};
use reqwest::header::{CONTENT_TYPE, REFERER};
use serde::Deserialize;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

use crate::app::{SoundboardApp, QUERY_URL};

#[derive(Deserialize, Debug, Clone)]
pub struct InstantSound {
    pub _id: Option<String>,
    pub title: String,
    pub mp3: String,
}

#[derive(Deserialize, Debug)]
pub struct InstantApiResponse {
    pub data: Vec<InstantSound>,
}

// we will use seperate thread for http for reqwest
static HTTP_RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("failed to start background HTTP runtime"));

// make a request client
static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36")
            .build()
            .expect("failed to build reqwest client")
});

/// Fetches a URL fully into memory with no progress reporting, used for
/// previews, where we don't show a progress bar :3.
async fn fetch_audio_bytes(url: &str) -> Option<Vec<u8>> {
    let response = HTTP_CLIENT
        .get(url)
        .header(REFERER, "https://www.myinstants.com/")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        eprintln!(
            "soundboard: fetch failed ({}) for {}",
            response.status(),
            url
        );
        return None;
    }

    if let Some(ct) = response.headers().get(CONTENT_TYPE) {
        if let Ok(ct) = ct.to_str() {
            if ct.starts_with("text/html") {
                eprintln!("soundboard: got HTML instead of audio from {}", url);
                return None;
            }
        }
    }

    response.bytes().await.ok().map(|b| b.to_vec())
}

/// Fetches a URL in chunks, sending (downloaded_bytes, total_bytes) over
/// `progress_tx` as data arrives. `total` is `None` if the server didn't
/// send Content-Length, in which case the download is indeterminate.
async fn fetch_audio_with_progress(
    url: &str,
    progress_tx: mpsc::UnboundedSender<(u64, Option<u64>)>,
) -> Option<Vec<u8>> {
    let response = HTTP_CLIENT
        .get(url)
        .header(REFERER, "https://www.myinstants.com/")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        eprintln!(
            "soundboard: fetch failed ({}) for {}",
            response.status(),
            url
        );
        return None;
    }

    if let Some(ct) = response.headers().get(CONTENT_TYPE) {
        if let Ok(ct) = ct.to_str() {
            if ct.starts_with("text/html") {
                eprintln!("soundboard: got HTML instead of audio from {}", url);
                return None;
            }
        }
    }

    let total = response.content_length();
    let mut downloaded: u64 = 0;
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.ok()?;
        downloaded += chunk.len() as u64;
        bytes.extend_from_slice(&chunk);
        let _ = progress_tx.unbounded_send((downloaded, total));
    }

    Some(bytes)
}

impl SoundboardApp {
    pub fn search_online_sounds(&mut self, cx: &mut Context<Self>) {
        let query = self.search_query.clone();
        self.is_searching = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let url = format!("{QUERY_URL}{query}");

            let (tx, rx) = oneshot::channel();
            HTTP_RUNTIME.spawn(async move {
                let result = async {
                    let response = HTTP_CLIENT.get(&url).send().await.ok()?;
                    let api_response = response.json::<InstantApiResponse>().await.ok()?;
                    Some(api_response.data)
                }
                .await;
                let _ = tx.send(result);
            });

            let results = rx.await.ok().flatten().unwrap_or_default();

            let _ = this.update(cx, |this, cx| {
                this.search_results = results;
                this.is_searching = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn play_online_sound(&mut self, sound: InstantSound, cx: &mut Context<Self>) {
        let preview_id = format!("preview-{}", sound.mp3);
        let dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("soundboard")
            .join("previews");
        let _ = std::fs::create_dir_all(&dir);

        let safe_title: String = sound
            .title
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ')
            .collect();
        let file_path = dir.join(format!("{}.mp3", safe_title));

        if file_path.exists() {
            self.play_cached_path(preview_id, file_path, cx);
            return;
        }

        cx.spawn(async move |this, cx| {
            let (tx, rx) = oneshot::channel();
            let mp3_url = sound.mp3.clone();

            HTTP_RUNTIME.spawn(async move {
                let result = fetch_audio_bytes(&mp3_url).await;
                let _ = tx.send(result);
            });

            if let Ok(Some(bytes)) = rx.await {
                if std::fs::write(&file_path, &bytes).is_ok() {
                    let _ = this.update(cx, |this, cx| {
                        this.play_cached_path(preview_id, file_path, cx);
                    });
                }
            }
        })
        .detach();
    }

    // download mp3/other and add to the soundboard
    pub fn download_and_add_sound(&mut self, sound: InstantSound, cx: &mut Context<Self>) {
        if self.is_busy {
            return;
        }
        self.is_busy = true;
        self.download_progress = Some(0.0);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let (progress_tx, mut progress_rx) = mpsc::unbounded::<(u64, Option<u64>)>();
            let (result_tx, result_rx) = oneshot::channel();
            let mp3_url = sound.mp3.clone();

            HTTP_RUNTIME.spawn(async move {
                let bytes = fetch_audio_with_progress(&mp3_url, progress_tx).await;
                let _ = result_tx.send(bytes);
            });

            while let Some((downloaded, total)) = progress_rx.next().await {
                let _ = this.update(cx, |this, cx| {
                    let fraction = total
                        .filter(|t| *t > 0)
                        .map(|t| (downloaded as f32 / t as f32).min(1.0));
                    this.download_progress = Some(fraction.unwrap_or(0.0));
                    cx.notify();
                });
            }

            let bytes = result_rx.await.ok().flatten();

            let Some(bytes) = bytes else {
                let title = sound.title.clone();
                let _ = this.update(cx, |this, cx| {
                    this.is_busy = false;
                    this.download_progress = None;
                    this.show_toast(
                        format!("Failed to download '{}'", title),
                        NotificationType::Error,
                        Anchor::BottomRight,
                        cx,
                    );
                });
                return;
            };

            let dir = dirs::cache_dir()
                .unwrap_or_else(std::env::temp_dir)
                .join("soundboard");
            let _ = std::fs::create_dir_all(&dir);

            let safe_title: String = sound
                .title
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == ' ')
                .collect();
            let file_path = dir.join(format!("{}.mp3", safe_title));
            let write_ok = std::fs::write(&file_path, &bytes).is_ok();

            let _ = this.update(cx, |this, cx| {
                if write_ok {
                    let id = format!(
                        "{}-{}",
                        safe_title.to_lowercase().replace(' ', "-"),
                        gpui_kit::accesskit::Uuid::new_v4()
                    );
                    this.library.add(id, sound.title.clone(), file_path);
                    let _ = this.library.save();
                }
                this.is_busy = false;
                this.download_progress = None;
                this.show_toast(
                    format!("Added '{}' to your library", safe_title),
                    NotificationType::Success,
                    Anchor::BottomRight,
                    cx,
                );
                cx.notify();
            });
        })
        .detach();
    }
}
