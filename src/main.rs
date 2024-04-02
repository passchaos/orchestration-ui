use std::time::Instant;
use std::{sync::Arc, time::Duration};

use anyhow::Result;
use eframe::egui::{self, mutex::RwLock};
use graphviz_rust::cmd::Format;
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::Deserialize;
use tokio::runtime::{Builder, Runtime};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Deserialize)]
struct DotInfo {
    dot_info: String,
}

#[derive(Debug, Deserialize)]
struct Resp {
    data: Option<DotInfo>,
}

async fn get_task_info(client: Client) -> Result<Option<Vec<u8>>> {
    let Resp { data } = client
        .get("http://localhost:8888/task/bt/info")
        .timeout(Duration::from_millis(100))
        .send()
        .await?
        .json()
        .await?;

    let res = if let Some(DotInfo { dot_info }) = data {
        tracing::info!("get dot info: {dot_info:#?}");
        let data = graphviz_rust::exec_dot(dot_info, vec![Format::Png.into()])?;
        Some(data)
    } else {
        None
    };

    Ok(res)
}

async fn pull_task_info(image_path_ref: Arc<RwLock<Option<Vec<u8>>>>) {
    loop {
        let begin = Instant::now();
        match get_task_info(reqwest::Client::new()).await {
            Ok(res) => {
                let elapsed = begin.elapsed().as_millis();
                tracing::info!(
                    "get task data cost: has_task= {} {elapsed}ms",
                    res.is_some()
                );

                match res {
                    Some(data) => {
                        *image_path_ref.write() = Some(data);
                    }
                    None => {
                        // *image_path_ref.write() = None;
                    }
                }
            }
            Err(e) => {
                tracing::warn!("get task info meet failure: err= {e}");
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

static RUNTIME: Lazy<Runtime> =
    Lazy::new(|| Builder::new_multi_thread().enable_all().build().unwrap());

fn main() {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let native_options = eframe::NativeOptions::default();

    let app = MyApp::new();

    let image_path_ref = app.image_path.clone();

    RUNTIME.spawn(pull_task_info(image_path_ref));

    eframe::run_native(
        "My egui app",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Box::new(app)
        }),
    )
    .unwrap();
}

#[derive(Default)]
struct MyApp {
    image_path: Arc<RwLock<Option<Vec<u8>>>>,
}

impl MyApp {
    fn new() -> Self {
        Self::default()
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(image_path) = self.image_path.read().clone() {
            ctx.forget_all_images();

            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.add(egui::Image::from_bytes("bytes://demo.png", image_path))
                });
            });

            // ctx.request_repaint();
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("no task found");
            });
        }
    }
}
