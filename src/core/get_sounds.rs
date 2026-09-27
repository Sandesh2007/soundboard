use gpui_kit::Context;
use serde::Deserialize;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

use crate::app::SoundboardApp;

#[derive(Deserialize, Debug, Clone)]
pub struct InstantSound {
    pub id: String,
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

async fn fetch_audio_bytes(url: &str) -> Option<bytes::Bytes> {
    let response = HTTP_CLIENT
        .get(url)
        .header(reqwest::header::REFERER, "https://www.myinstants.com/")
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

    if let Some(ct) = response.headers().get(reqwest::header::CONTENT_TYPE) {
        if let Ok(ct) = ct.to_str() {
            if ct.starts_with("text/html") {
                eprintln!(
                    "soundboard: got HTML instead of audio from {} (content-type: {})",
                    url, ct
                );
                return None;
            }
        }
    }

    response.bytes().await.ok()
}

impl SoundboardApp {
    pub fn search_online_sounds(&mut self, cx: &mut Context<Self>) {
        let query = self.search_query.clone();
        self.is_searching = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let url = format!("https://myinstants-api.vercel.app/search?q={}", query);

            let (tx, rx) = futures::channel::oneshot::channel();
            HTTP_RUNTIME.spawn(async move {
                let result = async {
                    let response = reqwest::get(&url).await.ok()?;
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

    // download mp3/other and add to the soundboard
    pub fn download_and_add_sound(&mut self, sound: InstantSound, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let (tx, rx) = futures::channel::oneshot::channel();
            let mp3_url = sound.mp3.clone();

            HTTP_RUNTIME.spawn(async move {
                let result = async { fetch_audio_bytes(&mp3_url).await }.await;
                let _ = tx.send(result);
            });

            if let Ok(Some(bytes)) = rx.await {
                let dir = dirs::config_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join("soundboard");
                let _ = std::fs::create_dir_all(&dir);

                let safe_title: String = sound
                    .title
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == ' ')
                    .collect();
                let file_path = dir.join(format!("{}.mp3", safe_title));

                println!("added '{}' to your soundboard library.", safe_title);

                if std::fs::write(&file_path, bytes).is_ok() {
                    let _ = this.update(cx, |this, cx| {
                        this.library.add(sound.id, sound.title, file_path);
                        let _ = this.library.save();
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    pub fn play_online_sound(&mut self, sound: InstantSound, cx: &mut Context<Self>) {
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

        // play from cache if available
        if file_path.exists() {
            if self.stop_others {
                self.audio.stop_all();
            }
            let volume = self.master_volume_fraction(cx);
            if let Err(err) = self.audio.play(&file_path, volume) {
                eprintln!("soundboard: failed to preview {:?}: {err:?}", file_path);
            }
            return;
        }

        cx.spawn(async move |this, cx| {
            let (tx, rx) = futures::channel::oneshot::channel();
            let mp3_url = sound.mp3.clone();

            HTTP_RUNTIME.spawn(async move {
                let result = async { fetch_audio_bytes(&mp3_url).await }.await;
                let _ = tx.send(result);
            });

            if let Ok(Some(bytes)) = rx.await {
                if std::fs::write(&file_path, &bytes).is_ok() {
                    let _ = this.update(cx, |this, cx| {
                        if this.stop_others {
                            this.audio.stop_all();
                        }
                        let volume = this.master_volume_fraction(cx);
                        if let Err(err) = this.audio.play(&file_path, volume) {
                            eprintln!("soundboard: failed to preview {:?}: {err:?}", file_path);
                        }
                    });
                }
            }
        })
        .detach();
    }
}
