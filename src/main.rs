#![cfg_attr(windows, windows_subsystem = "windows")]

mod audio;
mod auto_input;
mod config;
mod eliza;
mod speech_to_text;
mod vrchat;

use audio::AudioRecorder;
use config::Config;
use eframe::egui;
use speech_to_text::SpeechToTextClient;
use std::sync::mpsc::{channel, Receiver};
use tokio::sync::mpsc::UnboundedReceiver;

fn main() -> eframe::Result<()> {
    let config = Config::load();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([380.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "VRC Companion",
        options,
        Box::new(move |cc| {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "japanese".to_owned(),
                egui::FontData::from_static(include_bytes!("../fonts/NotoSansJP-Regular.ttf")),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "japanese".to_owned());
            cc.egui_ctx.set_fonts(fonts);

            Ok(Box::new(App::new(config)))
        }),
    )
}

const USER_PREFIX: &str = "> ";
const ELIZA_PREFIX: &str = "AI> ";

pub enum TranscriptionMessage {
    Partial(String),
    Success(String),
    Error(String),
}

struct App {
    config: Config,
    is_recording: bool,
    is_transcribing: bool,
    audio_recorder: Option<AudioRecorder>,
    transcribed_text: String,
    eliza_response: String,
    status_message: String,

    transcription_receiver: Option<UnboundedReceiver<TranscriptionMessage>>,
    tokio_runtime: Option<tokio::runtime::Runtime>,
    eliza_response_receiver: Option<Receiver<Result<String, String>>>,

    show_settings: bool,
    settings_xai_api_key: String,
    settings_eliza_url: String,

    available_devices: Vec<String>,
    selected_device_index: usize,
}

impl App {
    fn new(config: Config) -> Self {
        let mut available_devices = audio::get_input_devices().unwrap_or_else(|e| {
            eprintln!("Failed to get input devices: {}", e);
            vec![]
        });
        available_devices.insert(0, "デフォルト".to_string());

        let selected_device_index = config
            .input_device_name
            .as_ref()
            .and_then(|name| available_devices.iter().position(|d| d == name))
            .unwrap_or(0);

        Self {
            settings_xai_api_key: config.xai_api_key.clone(),
            settings_eliza_url: config.eliza_url.clone(),
            config,
            is_recording: false,
            is_transcribing: false,
            audio_recorder: None,
            transcribed_text: String::new(),
            eliza_response: String::new(),
            status_message: String::new(),
            transcription_receiver: None,
            tokio_runtime: None,
            eliza_response_receiver: None,
            show_settings: false,
            available_devices,
            selected_device_index,
        }
    }

    fn on_start_recording(&mut self) {
        self.status_message = "Recording... Speak now!".to_string();

        let mut recorder = AudioRecorder::new(self.config.silence_threshold);
        let (chunk_tx, chunk_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<f32>>();
        let device_name = self.config.input_device_name.as_deref();

        match recorder.start_recording(device_name, chunk_tx) {
            Ok(_) => {
                let sample_rate = recorder.get_sample_rate();
                self.audio_recorder = Some(recorder);

                if self.config.xai_api_key.is_empty() {
                    self.status_message =
                        "Recording... (Set xAI API key in Settings to enable transcription)"
                            .to_string();
                } else {
                    self.start_streaming_transcription(sample_rate, chunk_rx);
                }
            }
            Err(e) => {
                self.status_message = format!("Error: {}", e);
                self.is_recording = false;
            }
        }
    }

    fn on_stop_recording(&mut self) {
        if let Some(mut recorder) = self.audio_recorder.take() {
            recorder.stop_recording();
        }
        self.status_message = if self.is_transcribing {
            "Transcribing...".to_string()
        } else {
            "Recording stopped.".to_string()
        };
    }

    fn start_streaming_transcription(
        &mut self,
        sample_rate: u32,
        chunk_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<f32>>,
    ) {
        let (msg_tx, msg_rx) = tokio::sync::mpsc::unbounded_channel::<TranscriptionMessage>();
        self.transcription_receiver = Some(msg_rx);
        self.is_transcribing = true;

        let api_key = self.config.xai_api_key.clone();
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime");

        rt.spawn(async move {
            let client = SpeechToTextClient::new(api_key);
            if let Err(e) = client
                .stream_transcribe(sample_rate, chunk_rx, msg_tx.clone())
                .await
            {
                let _ = msg_tx.send(TranscriptionMessage::Error(e.to_string()));
            }
        });

        self.tokio_runtime = Some(rt);
    }

    fn on_transcription_success(&mut self, text: String) {
        self.transcribed_text = text.clone();

        if self.config.clipboard_enabled {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(&text);
            }
        }

        if self.config.vrchat_enabled && !text.is_empty() {
            let client = vrchat::VRChatClient::new();
            if let Err(e) = client.send_message(&format!("{}{}", USER_PREFIX, text)) {
                eprintln!("VRChat OSC error: {}", e);
            }
        }

        if self.config.eliza_enabled && !text.is_empty() {
            let eliza_url = self.config.eliza_url.clone();
            let eliza_text = text.clone();
            let (tx, rx) = channel::<Result<String, String>>();
            std::thread::spawn(move || {
                let client = eliza::ElizaClient::new(eliza_url);
                let _ = tx.send(client.send_chat(&eliza_text));
            });
            self.eliza_response_receiver = Some(rx);
        }

        if self.config.auto_input_enabled {
            let result = match (self.config.clipboard_enabled, self.config.auto_input_send_enter)
            {
                (true, true) => auto_input::send_ctrl_v_with_enter(),
                (true, false) => auto_input::send_ctrl_v(),
                (false, true) => auto_input::type_text_with_enter(&text),
                (false, false) => auto_input::type_text(&text),
            };
            if let Err(e) = result {
                eprintln!("Auto-input error: {}", e);
            }
        }

        self.status_message = "Transcription completed!".to_string();
        self.is_transcribing = false;
        self.transcription_receiver = None;
        if let Some(rt) = self.tokio_runtime.take() {
            rt.shutdown_background();
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(ref receiver) = self.eliza_response_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    Ok(response) => {
                        self.eliza_response = response.clone();
                        if self.config.vrchat_enabled {
                            let client = vrchat::VRChatClient::new();
                            if let Err(e) =
                                client.send_message(&format!("{}{}", ELIZA_PREFIX, response))
                            {
                                eprintln!("Failed to send Eliza response to VRChat: {}", e);
                            }
                        }
                    }
                    Err(e) => eprintln!("Eliza error: {}", e),
                }
                self.eliza_response_receiver = None;
            }
        }

        if let Some(receiver) = &mut self.transcription_receiver {
            if let Ok(message) = receiver.try_recv() {
                match message {
                    TranscriptionMessage::Partial(text) => self.transcribed_text = text,
                    TranscriptionMessage::Success(text) => self.on_transcription_success(text),
                    TranscriptionMessage::Error(error) => {
                        self.status_message = format!("Transcription failed: {}", error);
                        self.is_transcribing = false;
                        self.transcription_receiver = None;
                        if let Some(rt) = self.tokio_runtime.take() {
                            rt.shutdown_background();
                        }
                    }
                }
            }
        }

        if self.show_settings {
            egui::Window::new("Settings")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("xAI API Key:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.settings_xai_api_key)
                            .password(true)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(10.0);
                    ui.label("Eliza Agent URL:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.settings_eliza_url)
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(10.0);
                    ui.label("Input Device:");
                    egui::ComboBox::from_id_salt("input_device_combo")
                        .selected_text(
                            self.available_devices
                                .get(self.selected_device_index)
                                .cloned()
                                .unwrap_or_default(),
                        )
                        .show_ui(ui, |ui| {
                            for (idx, device_name) in self.available_devices.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_device_index, idx, device_name);
                            }
                        });
                    ui.add_space(10.0);
                    ui.label("Silence Duration (seconds):");
                    ui.add(egui::Slider::new(
                        &mut self.config.silence_duration_secs,
                        0.5..=10.0,
                    ));
                    ui.label("Silence Threshold:");
                    ui.add(
                        egui::Slider::new(&mut self.config.silence_threshold, 0.001..=0.3)
                            .logarithmic(true),
                    );

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            self.config.xai_api_key = self.settings_xai_api_key.trim().to_string();
                            self.config.eliza_url = self.settings_eliza_url.trim().to_string();
                            self.config.input_device_name = if self.selected_device_index == 0 {
                                None
                            } else {
                                self.available_devices.get(self.selected_device_index).cloned()
                            };
                            if let Err(e) = self.config.save() {
                                eprintln!("Failed to save config: {}", e);
                            }
                            self.show_settings = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.settings_xai_api_key = self.config.xai_api_key.clone();
                            self.settings_eliza_url = self.config.eliza_url.clone();
                            self.selected_device_index = self
                                .config
                                .input_device_name
                                .as_ref()
                                .and_then(|name| self.available_devices.iter().position(|d| d == name))
                                .unwrap_or(0);
                            self.show_settings = false;
                        }
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("⚙ Settings").clicked() {
                            self.show_settings = true;
                        }
                    });
                });

                if self.config.xai_api_key.is_empty() {
                    ui.colored_label(
                        egui::Color32::RED,
                        "⚠ xAI API key not set. Please configure in Settings.",
                    );
                }
                if !self.status_message.is_empty() {
                    ui.label(&self.status_message);
                }
                if self.is_recording {
                    if let Some(recorder) = &self.audio_recorder {
                        ui.label(format!(
                            "Recording: {:.1}s | Silence: {:.1}s/{:.1}s",
                            recorder.get_recording_duration(),
                            recorder.get_silence_duration().as_secs_f32(),
                            self.config.silence_duration_secs,
                        ));
                    }
                }

                ui.add_space(10.0);

                let button_text = if self.is_recording { "⏹ Stop" } else { "⏺ Start" };
                let button_size = egui::vec2(300.0, 60.0);

                let silence_progress = if self.is_recording {
                    self.audio_recorder
                        .as_ref()
                        .map(|recorder| {
                            (recorder.get_silence_duration().as_secs_f32()
                                / self.config.silence_duration_secs)
                                .min(1.0)
                        })
                        .unwrap_or(0.0)
                } else {
                    0.0
                };

                let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());
                let visuals = ui.style().interact(&response);

                ui.painter()
                    .rect_filled(rect, visuals.rounding, visuals.bg_fill);

                if self.is_recording {
                    let fill_height = rect.height() * (1.0 - silence_progress);
                    if fill_height > 0.0 {
                        let progress_rect = egui::Rect::from_min_size(
                            egui::pos2(rect.min.x, rect.max.y - fill_height),
                            egui::vec2(rect.width(), fill_height),
                        );
                        ui.painter().rect_filled(
                            progress_rect,
                            visuals.rounding,
                            egui::Color32::from_rgb(100, 200, 255),
                        );
                    }
                }

                ui.painter()
                    .rect_stroke(rect, visuals.rounding, visuals.bg_stroke);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    button_text,
                    egui::FontId::proportional(24.0),
                    visuals.text_color(),
                );

                if response.clicked() {
                    if self.is_recording {
                        self.is_recording = false;
                        self.on_stop_recording();
                    } else {
                        self.is_recording = true;
                        self.on_start_recording();
                    }
                }

                ui.add_space(10.0);

                if self.is_recording {
                    if let Some(recorder) = &self.audio_recorder {
                        let max_amplitude = recorder.get_max_amplitude();
                        let bar_width = 300.0;
                        let bar_height = 10.0;
                        let clipped_amplitude = max_amplitude.min(1.2);
                        let bar_fill_width = (clipped_amplitude / 1.2) * bar_width;

                        let (bar_rect, _) = ui.allocate_exact_size(
                            egui::vec2(bar_width, bar_height),
                            egui::Sense::hover(),
                        );

                        ui.painter()
                            .rect_filled(bar_rect, 2.0, egui::Color32::from_rgb(50, 50, 50));

                        if bar_fill_width > 0.0 {
                            let fill_rect = egui::Rect::from_min_size(
                                bar_rect.min,
                                egui::vec2(bar_fill_width, bar_height),
                            );
                            let color = if max_amplitude < self.config.silence_threshold {
                                egui::Color32::from_rgb(150, 150, 150)
                            } else if max_amplitude < 1.0 {
                                egui::Color32::from_rgb(0, 200, 0)
                            } else {
                                egui::Color32::from_rgb(255, 0, 0)
                            };
                            ui.painter().rect_filled(fill_rect, 2.0, color);
                        }

                        ui.painter().rect_stroke(
                            bar_rect,
                            2.0,
                            egui::Stroke::new(1.0, egui::Color32::from_rgb(100, 100, 100)),
                        );

                        ui.label(format!("Level: {:.3}", max_amplitude));
                    }

                    ui.add_space(10.0);
                }
                ui.label("Transcribed:");
                ui.add_sized(
                    [300.0, 60.0],
                    egui::TextEdit::multiline(&mut self.transcribed_text).interactive(false),
                );

                if self.config.eliza_enabled && !self.eliza_response.is_empty() {
                    ui.add_space(5.0);
                    ui.label("Eliza:");
                    ui.add_sized(
                        [300.0, 60.0],
                        egui::TextEdit::multiline(&mut self.eliza_response).interactive(false),
                    );
                }

                ui.add_space(10.0);

                let clipboard_changed = ui
                    .checkbox(&mut self.config.clipboard_enabled, "Auto-copy to clipboard")
                    .changed();

                let auto_input_changed = ui
                    .checkbox(
                        &mut self.config.auto_input_enabled,
                        "Auto-input to active window",
                    )
                    .changed();
                if auto_input_changed && self.config.auto_input_enabled {
                    self.config.enable_auto_input_exclusive();
                }

                let send_enter_changed = ui
                    .add_enabled(
                        self.config.auto_input_enabled,
                        egui::Checkbox::new(
                            &mut self.config.auto_input_send_enter,
                            "Send Enter after input",
                        ),
                    )
                    .changed();

                let vrchat_changed = ui
                    .checkbox(&mut self.config.vrchat_enabled, "Send to VRChat")
                    .changed();
                if vrchat_changed && self.config.vrchat_enabled {
                    self.config.enable_vrchat_exclusive();
                }

                let eliza_changed = ui
                    .checkbox(&mut self.config.eliza_enabled, "Send to Eliza")
                    .changed();

                if ui.button("📝 call QvPen").clicked() {
                    if let Err(e) = auto_input::call_qvpen() {
                        eprintln!("call_qvpen error: {}", e);
                    }
                }

                if clipboard_changed
                    || auto_input_changed
                    || send_enter_changed
                    || vrchat_changed
                    || eliza_changed
                {
                    if let Err(e) = self.config.save() {
                        eprintln!("Failed to save config: {}", e);
                    }
                }
            });
        });

        if self.is_recording {
            if let Some(recorder) = &self.audio_recorder {
                if recorder.is_silent(self.config.silence_duration_secs) {
                    self.is_recording = false;
                    self.on_stop_recording();
                }
            }
            ctx.request_repaint();
        }
        if self.is_transcribing {
            ctx.request_repaint();
        }
    }
}
