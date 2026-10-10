use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};

use crate::config;
use crate::config::Config;
use crate::harness::{self, build_command, expand_tilde};
use crate::runner::{Event, Runner};

const MAX_OUT_LINES: usize = 1500;

pub struct GreenroomApp {
    cfg: Config,
    cfg_path: PathBuf,
    selected: usize,
    model: String,
    cwd: String,
    prompt: String,
    out: Vec<String>,
    runner: Option<Runner>,
    status: String,
    status_color: Color32,
    t_start: Option<Instant>,
    auto_scroll: bool,
}

impl GreenroomApp {
    pub fn new(cfg: Config, cfg_path: PathBuf, note: Option<String>) -> Self {
        let cwd = cfg.default_dir.clone();
        let (status, status_color) = match note {
            Some(n) => (n, Color32::from_rgb(230, 160, 110)),
            None => ("idle".to_string(), Color32::GRAY),
        };
        let selected = cfg
            .harness
            .iter()
            .position(|h| harness::bin_path(&h.bin).is_some())
            .unwrap_or(0);
        Self {
            cfg,
            cfg_path,
            selected,
            model: String::new(),
            cwd,
            prompt: String::new(),
            out: Vec::new(),
            runner: None,
            status,
            status_color,
            t_start: None,
            auto_scroll: true,
        }
    }

    fn harness(&self) -> Option<&config::Harness> {
        self.cfg.harness.get(self.selected)
    }

    fn running(&self) -> bool {
        self.runner.is_some()
    }

    fn start_run(&mut self, ctx: &egui::Context) {
        let Some(h) = self.harness().cloned() else { return };
        let model = self.model.trim().to_string();
        let mopt = if model.is_empty() { None } else { Some(model.as_str()) };
        let argv = build_command(&h, self.prompt.trim(), mopt);
        let cwd = expand_tilde(&self.cwd);
        let cwd_path = PathBuf::from(&cwd);
        if !cwd_path.is_dir() {
            self.status = format!("dir not found: {cwd}");
            self.status_color = Color32::from_rgb(230, 120, 90);
            return;
        }
        self.out.clear();
        self.out.push(format!("$ {}", argv.join(" ")));
        self.out.push(String::new());
        match Runner::start(&argv[0], &argv[1..], &cwd_path, ctx.clone()) {
            Ok(r) => {
                self.runner = Some(r);
                self.status = "running…".into();
                self.status_color = Color32::from_rgb(150, 200, 255);
                self.t_start = Some(Instant::now());
            }
            Err(e) => {
                self.status = e;
                self.status_color = Color32::from_rgb(230, 120, 90);
            }
        }
    }

    fn poll(&mut self) {
        let mut events = Vec::new();
        if let Some(r) = &self.runner {
            while let Ok(ev) = r.rx.try_recv() {
                events.push(ev);
            }
        }
        let mut done = None;
        for ev in events {
            match ev {
                Event::Line(l) => {
                    self.out.push(l);
                    if self.out.len() > MAX_OUT_LINES {
                        let excess = self.out.len() - MAX_OUT_LINES;
                        self.out.drain(0..excess);
                    }
                }
                Event::Done { code, secs } => done = Some((code, secs)),
            }
        }
        if let Some((code, secs)) = done {
            let cancelled = self
                .runner
                .as_ref()
                .map(|r| r.cancelled.load(Ordering::SeqCst))
                .unwrap_or(false);
            self.status = if cancelled {
                format!("cancelled · {secs:.1}s")
            } else {
                match code {
                    Some(0) => format!("exit 0 · {secs:.1}s"),
                    Some(c) => format!("exit {c} · {secs:.1}s"),
                    None => format!("killed · {secs:.1}s"),
                }
            };
            self.status_color = if code == Some(0) && !cancelled {
                Color32::from_rgb(140, 210, 140)
            } else {
                Color32::from_rgb(230, 160, 110)
            };
            self.runner = None;
            self.t_start = None;
        }
    }

    fn reload_config(&mut self) {
        let (cfg, path, note) = config::load();
        self.cfg = cfg;
        self.cfg_path = path;
        if self.selected >= self.cfg.harness.len() {
            self.selected = 0;
        }
        if let Some(n) = note {
            self.status = n;
            self.status_color = Color32::from_rgb(230, 160, 110);
        }
    }

    fn ui_top(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("🎭 Greenroom");
            ui.label(RichText::new("— call an agent onto the stage").weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("↻ reload config")
                    .on_hover_text(format!("{}", self.cfg_path.display()))
                    .clicked()
                {
                    self.reload_config();
                }
                ui.label(
                    RichText::new(
                        self.cfg_path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    )
                    .weak()
                    .small(),
                );
            });
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for i in 0..self.cfg.harness.len() {
                let h = self.cfg.harness[i].clone();
                let installed = harness::bin_path(&h.bin).is_some();
                let text = format!("{}{}", if installed { "● " } else { "○ " }, h.name);
                let mut label = RichText::new(text);
                if !installed {
                    label = label.weak();
                }
                let resp = ui
                    .selectable_label(self.selected == i, label)
                    .on_hover_text(format!("{} {}", h.bin, h.args.join(" ")));
                if resp.clicked() && !self.running() {
                    self.selected = i;
                    self.model.clear();
                }
            }
        });
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label("model:");
            ui.add(
                egui::TextEdit::singleline(&mut self.model)
                    .desired_width(240.0)
                    .hint_text("(harness default)")
                    .font(egui::TextStyle::Monospace),
            );
            let models = self.harness().map(|h| h.models.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("model_pick")
                .selected_text("▾")
                .width(16.0)
                .show_ui(ui, |ui| {
                    if models.is_empty() {
                        ui.label(
                            RichText::new("no presets — type an id above or add models in config")
                                .weak(),
                        );
                    }
                    for m in &models {
                        if ui.selectable_label(self.model == *m, m).clicked() {
                            self.model = m.clone();
                        }
                    }
                    if !models.is_empty() {
                        ui.separator();
                        if ui.button("clear").clicked() {
                            self.model.clear();
                        }
                    }
                });
            ui.separator();
            ui.label("dir:");
            ui.add(egui::TextEdit::singleline(&mut self.cwd).desired_width(260.0));
            if ui.button("📁").on_hover_text("pick working directory").clicked() {
                let start = expand_tilde(&self.cwd);
                if let Some(dir) = rfd::FileDialog::new().set_directory(&start).pick_folder() {
                    self.cwd = dir.display().to_string();
                }
            }
        });
        ui.add_space(4.0);
        ui.separator();
    }

    fn ui_main(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.add(
            egui::TextEdit::multiline(&mut self.prompt)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text("What should the agent do?  (Cmd+Enter to run)"),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let sel_ok = self
                .harness()
                .map(|h| harness::bin_path(&h.bin).is_some())
                .unwrap_or(false);
            let can_run = !self.running() && sel_ok && !self.prompt.trim().is_empty();
            if ui
                .add_enabled(can_run, egui::Button::new(RichText::new("▶  Run").strong()))
                .on_disabled_hover_text(if sel_ok {
                    "write a prompt first"
                } else {
                    "selected harness binary not found on PATH"
                })
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.start_run(&ctx);
            }
            if self.running() {
                if ui.button("✕  Cancel").clicked() {
                    if let Some(r) = &self.runner {
                        r.cancel();
                    }
                    self.status = "cancelling…".into();
                    self.status_color = Color32::from_rgb(230, 160, 110);
                }
                ui.spinner();
            }
            ui.label(RichText::new(&self.status).color(self.status_color));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut self.auto_scroll, "auto-scroll");
                if ui.small_button("clear").clicked() {
                    self.out.clear();
                }
            });
        });
        if let Some(h) = self.harness().cloned() {
            let model = self.model.trim();
            let mopt = if model.is_empty() { None } else { Some(model) };
            let argv = build_command(&h, "<prompt>", mopt);
            let mut prev = argv.join(" ");
            if prev.chars().count() > 160 {
                prev = format!("{}…", prev.chars().take(159).collect::<String>());
            }
            ui.label(RichText::new(prev).monospace().weak().small());
        }
        ui.add_space(2.0);
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(self.auto_scroll)
            .show(ui, |ui| {
                let text = self.out.join("\n");
                ui.add(
                    egui::Label::new(RichText::new(text).monospace().size(12.0)).selectable(true),
                );
            });
    }
}

impl eframe::App for GreenroomApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll();
        let ctx = ui.ctx().clone();
        if self.running() {
            if let Some(t) = self.t_start {
                self.status = format!("running… {:.0}s", t.elapsed().as_secs_f32());
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        let run_requested = ui.input(|i| {
            i.modifiers.command && i.key_pressed(egui::Key::Enter) && !self.prompt.trim().is_empty()
        });
        self.ui_top(ui);
        self.ui_main(ui);
        if run_requested && !self.running() {
            self.start_run(&ctx);
        }
    }

    fn on_exit(&mut self) {
        if let Some(r) = &self.runner {
            r.cancel();
        }
    }
}

impl Drop for GreenroomApp {
    fn drop(&mut self) {
        if let Some(r) = &self.runner {
            r.cancel();
        }
    }
}
