use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Stroke, Vec2};

use crate::config;
use crate::config::{Config, Keys};
use crate::harness::{self, build_command, expand_tilde};
use crate::runner::{Event, Runner};

const MAX_OUT_LINES: usize = 2000;

// ── palette ─────────────────────────────────────────────────────────────
const BG: Color32 = Color32::from_rgb(16, 16, 21);
const PANEL: Color32 = Color32::from_rgb(23, 23, 30);
const CARD: Color32 = Color32::from_rgb(28, 28, 37);
const CONSOLE_BG: Color32 = Color32::from_rgb(11, 11, 15);
const STROKE_SOFT: Color32 = Color32::from_rgb(46, 46, 60);
const TEXT_DIM: Color32 = Color32::from_rgb(162, 162, 178);
const TEXT_BRIGHT: Color32 = Color32::from_rgb(235, 235, 245);
const ACCENT: Color32 = Color32::from_rgb(122, 92, 255);
const OK_GREEN: Color32 = Color32::from_rgb(110, 200, 130);
const WARN: Color32 = Color32::from_rgb(235, 170, 90);
const ERR: Color32 = Color32::from_rgb(235, 105, 95);
const RUN_BLUE: Color32 = Color32::from_rgb(120, 170, 255);

/// Install the Greenroom theme (call once at startup).
pub fn install_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = CONSOLE_BG;
    v.faint_bg_color = Color32::from_rgb(31, 31, 41);
    v.window_corner_radius = CornerRadius::same(10);
    v.selection.bg_fill = ACCENT.gamma_multiply(0.55);
    v.selection.stroke = Stroke::new(1.0, Color32::from_rgb(190, 175, 255));
    v.hyperlink_color = Color32::from_rgb(150, 125, 255);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, STROKE_SOFT);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_BRIGHT);
    w.noninteractive.corner_radius = CornerRadius::same(8);
    w.inactive.bg_fill = Color32::from_rgb(36, 36, 48);
    w.inactive.weak_bg_fill = Color32::from_rgb(31, 31, 41);
    w.inactive.bg_stroke = Stroke::new(1.0, STROKE_SOFT);
    w.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(228, 228, 238));
    w.inactive.corner_radius = CornerRadius::same(8);
    w.hovered.bg_fill = Color32::from_rgb(48, 48, 64);
    w.hovered.weak_bg_fill = Color32::from_rgb(44, 44, 58);
    w.hovered.bg_stroke = Stroke::new(1.0, ACCENT.gamma_multiply(0.75));
    w.hovered.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    w.hovered.corner_radius = CornerRadius::same(8);
    w.active.bg_fill = ACCENT;
    w.active.weak_bg_fill = ACCENT;
    w.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(150, 125, 255));
    w.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    w.active.corner_radius = CornerRadius::same(8);
    w.open.bg_fill = Color32::from_rgb(42, 42, 56);
    w.open.weak_bg_fill = Color32::from_rgb(39, 39, 52);
    w.open.bg_stroke = Stroke::new(1.0, STROKE_SOFT);
    w.open.fg_stroke = Stroke::new(1.0, TEXT_BRIGHT);
    w.open.corner_radius = CornerRadius::same(8);
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(11.0, 5.0);
    style.spacing.interact_size.y = 26.0;
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(12.5));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.5));
    });
}

// ── keybindings ─────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Bind {
    cmd: bool,
    ctrl: bool,
    alt: bool,
    shift: bool,
    key: egui::Key,
}

fn parse_binding(spec: &str) -> Option<Bind> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    let mut b = Bind {
        cmd: false,
        ctrl: false,
        alt: false,
        shift: false,
        key: egui::Key::Enter,
    };
    let mut found_key = false;
    for tok in spec.split('+') {
        let t = tok.trim().to_lowercase();
        match t.as_str() {
            "cmd" | "command" | "meta" | "super" => b.cmd = true,
            "ctrl" | "control" => b.ctrl = true,
            "alt" | "opt" | "option" => b.alt = true,
            "shift" => b.shift = true,
            "" => {}
            other => {
                b.key = key_from_token(other)?;
                found_key = true;
            }
        }
    }
    if found_key {
        Some(b)
    } else {
        None
    }
}

fn key_from_token(t: &str) -> Option<egui::Key> {
    use egui::Key as K;
    Some(match t {
        "enter" | "return" => K::Enter,
        "esc" | "escape" => K::Escape,
        "space" => K::Space,
        "tab" => K::Tab,
        "period" | "." => K::Period,
        "comma" | "," => K::Comma,
        "slash" | "/" => K::Slash,
        "minus" | "-" => K::Minus,
        "backspace" => K::Backspace,
        "delete" => K::Delete,
        "up" => K::ArrowUp,
        "down" => K::ArrowDown,
        "left" => K::ArrowLeft,
        "right" => K::ArrowRight,
        "home" => K::Home,
        "end" => K::End,
        "pageup" => K::PageUp,
        "pagedown" => K::PageDown,
        "f1" => K::F1,
        "f2" => K::F2,
        "f3" => K::F3,
        "f4" => K::F4,
        "f5" => K::F5,
        "f6" => K::F6,
        "f7" => K::F7,
        "f8" => K::F8,
        "f9" => K::F9,
        "f10" => K::F10,
        "f11" => K::F11,
        "f12" => K::F12,
        _ => key_char(t.chars().next()?)?,
    })
}

fn key_char(c: char) -> Option<egui::Key> {
    use egui::Key as K;
    Some(match c {
        'a' => K::A,
        'b' => K::B,
        'c' => K::C,
        'd' => K::D,
        'e' => K::E,
        'f' => K::F,
        'g' => K::G,
        'h' => K::H,
        'i' => K::I,
        'j' => K::J,
        'k' => K::K,
        'l' => K::L,
        'm' => K::M,
        'n' => K::N,
        'o' => K::O,
        'p' => K::P,
        'q' => K::Q,
        'r' => K::R,
        's' => K::S,
        't' => K::T,
        'u' => K::U,
        'v' => K::V,
        'w' => K::W,
        'x' => K::X,
        'y' => K::Y,
        'z' => K::Z,
        '0' => K::Num0,
        '1' => K::Num1,
        '2' => K::Num2,
        '3' => K::Num3,
        '4' => K::Num4,
        '5' => K::Num5,
        '6' => K::Num6,
        '7' => K::Num7,
        '8' => K::Num8,
        '9' => K::Num9,
        _ => return None,
    })
}

fn binding_pressed(ui: &egui::Ui, spec: &str) -> bool {
    let Some(b) = parse_binding(spec) else {
        return false;
    };
    ui.input(|i| {
        let m = i.modifiers;
        m.command == b.cmd
            && m.ctrl == b.ctrl
            && m.alt == b.alt
            && m.shift == b.shift
            && i.key_pressed(b.key)
    })
}

// ── config editor state ─────────────────────────────────────────────────

#[derive(PartialEq, Clone, Copy)]
enum View {
    Main,
    Config,
}

#[derive(PartialEq, Clone, Copy)]
enum ConfigTab {
    Form,
    Raw,
}

struct HarnessForm {
    name: String,
    bin: String,
    args: String,
    model_args: String,
    models: String,
    efforts: String,
    effort_args: String,
}

impl HarnessForm {
    fn from_harness(h: &config::Harness) -> Self {
        Self {
            name: h.name.clone(),
            bin: h.bin.clone(),
            args: h.args.join("\n"),
            model_args: h.model_args.join("\n"),
            models: h.models.join("\n"),
            efforts: h.efforts.join("\n"),
            effort_args: h.effort_args.join("\n"),
        }
    }

    fn to_harness(&self) -> config::Harness {
        config::Harness {
            name: self.name.trim().to_string(),
            bin: self.bin.trim().to_string(),
            args: split_lines(&self.args),
            model_args: split_lines(&self.model_args),
            models: split_lines(&self.models),
            efforts: split_lines(&self.efforts),
            effort_args: split_lines(&self.effort_args),
        }
    }
}

fn split_lines(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

struct FormState {
    default_dir: String,
    keys: Keys,
    harness: Vec<HarnessForm>,
}

impl FormState {
    fn from_config(cfg: &Config) -> Self {
        Self {
            default_dir: cfg.default_dir.clone(),
            keys: cfg.keys.clone(),
            harness: cfg.harness.iter().map(HarnessForm::from_harness).collect(),
        }
    }

    fn to_config(&self) -> Config {
        let dir = if self.default_dir.trim().is_empty() {
            "~".to_string()
        } else {
            self.default_dir.trim().to_string()
        };
        Config {
            default_dir: dir,
            keys: self.keys.clone(),
            harness: self
                .harness
                .iter()
                .map(HarnessForm::to_harness)
                .filter(|h| !h.name.is_empty() && !h.bin.is_empty())
                .collect(),
        }
    }
}

// ── app ─────────────────────────────────────────────────────────────────

pub struct GreenroomApp {
    cfg: Config,
    cfg_path: PathBuf,
    selected: usize,
    model: String,
    effort: String,
    memo_model: Vec<String>,
    memo_effort: Vec<String>,
    cwd: String,
    prompt: String,
    out: Vec<String>,
    runner: Option<Runner>,
    status: String,
    status_color: Color32,
    t_start: Option<Instant>,
    auto_scroll: bool,
    view: View,
    cfg_tab: ConfigTab,
    raw_text: String,
    cfg_msg: Option<(String, Color32)>,
    form: FormState,
}

impl GreenroomApp {
    pub fn new(cfg: Config, cfg_path: PathBuf, note: Option<String>) -> Self {
        let cwd = cfg.default_dir.clone();
        let (status, status_color) = match note {
            Some(n) => (n, WARN),
            None => ("idle".to_string(), Color32::GRAY),
        };
        let selected = cfg
            .harness
            .iter()
            .position(|h| harness::bin_path(&h.bin).is_some())
            .unwrap_or(0);
        let form = FormState::from_config(&cfg);
        let n = cfg.harness.len();
        Self {
            cfg,
            cfg_path,
            selected,
            model: String::new(),
            effort: String::new(),
            memo_model: vec![String::new(); n],
            memo_effort: vec![String::new(); n],
            cwd,
            prompt: String::new(),
            out: Vec::new(),
            runner: None,
            status,
            status_color,
            t_start: None,
            auto_scroll: true,
            view: View::Main,
            cfg_tab: ConfigTab::Form,
            raw_text: String::new(),
            cfg_msg: None,
            form,
        }
    }

    fn harness(&self) -> Option<&config::Harness> {
        self.cfg.harness.get(self.selected)
    }

    fn running(&self) -> bool {
        self.runner.is_some()
    }

    fn can_run(&self) -> bool {
        !self.running()
            && !self.prompt.trim().is_empty()
            && self
                .harness()
                .map(|h| harness::bin_path(&h.bin).is_some())
                .unwrap_or(false)
    }

    fn switch_harness(&mut self, i: usize) {
        if i == self.selected || i >= self.cfg.harness.len() {
            return;
        }
        let len = self.cfg.harness.len();
        self.memo_model.resize(len, String::new());
        self.memo_effort.resize(len, String::new());
        self.memo_model[self.selected] = self.model.clone();
        self.memo_effort[self.selected] = self.effort.clone();
        self.selected = i;
        self.model = self.memo_model[i].clone();
        self.effort = self.memo_effort[i].clone();
    }

    fn start_run(&mut self, ctx: &egui::Context) {
        let Some(h) = self.harness().cloned() else {
            return;
        };
        let model = self.model.trim().to_string();
        let effort = self.effort.trim().to_string();
        let mopt = if model.is_empty() {
            None
        } else {
            Some(model.as_str())
        };
        let eopt = if effort.is_empty() {
            None
        } else {
            Some(effort.as_str())
        };
        let argv = build_command(&h, self.prompt.trim(), mopt, eopt);
        let cwd = expand_tilde(&self.cwd);
        let cwd_path = PathBuf::from(&cwd);
        if !cwd_path.is_dir() {
            self.status = format!("dir not found: {cwd}");
            self.status_color = ERR;
            return;
        }
        self.out.clear();
        self.out.push(format!("$ {}", argv.join(" ")));
        self.out.push(String::new());
        match Runner::start(&argv[0], &argv[1..], &cwd_path, ctx.clone()) {
            Ok(r) => {
                self.runner = Some(r);
                self.status = "running…".into();
                self.status_color = RUN_BLUE;
                self.t_start = Some(Instant::now());
            }
            Err(e) => {
                self.status = e;
                self.status_color = ERR;
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
                OK_GREEN
            } else {
                WARN
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
        let len = self.cfg.harness.len();
        self.memo_model.resize(len, String::new());
        self.memo_effort.resize(len, String::new());
        if let Some(n) = note {
            self.status = n;
            self.status_color = WARN;
        }
    }

    fn open_config(&mut self) {
        self.raw_text = std::fs::read_to_string(&self.cfg_path)
            .unwrap_or_else(|_| config::DEFAULT_CONFIG.to_string());
        self.form = FormState::from_config(&self.cfg);
        self.cfg_msg = None;
        self.view = View::Config;
    }

    // ── main view ───────────────────────────────────────────────────────

    fn ui_main(&mut self, ui: &mut egui::Ui) {
        let keys = self.cfg.keys.clone();
        let mut do_reload = false;
        let mut do_open_cfg = false;

        // header card
        egui::Frame::default()
            .fill(PANEL)
            .inner_margin(Margin::same(12))
            .corner_radius(CornerRadius::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Greenroom")
                            .size(20.0)
                            .strong()
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        RichText::new("call an agent onto the stage")
                            .color(TEXT_DIM)
                            .size(12.0),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button("Reload")
                            .on_hover_text(format!("reload config ({})", keys.reload))
                            .clicked()
                        {
                            do_reload = true;
                        }
                        if ui
                            .button("Config")
                            .on_hover_text(format!(
                                "edit {} ({})",
                                self.cfg_path.display(),
                                keys.config
                            ))
                            .clicked()
                        {
                            do_open_cfg = true;
                        }
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(self.status.as_str())
                                .color(self.status_color)
                                .size(12.5),
                        );
                        status_dot(ui, self.status_color);
                    });
                });
            });
        ui.add_space(8.0);

        // harness pills
        let names: Vec<(String, String)> = self
            .cfg
            .harness
            .iter()
            .map(|h| (h.name.clone(), h.bin.clone()))
            .collect();
        let mut switch_to: Option<usize> = None;
        ui.horizontal_wrapped(|ui| {
            for (i, (name, bin)) in names.iter().enumerate() {
                let installed = harness::bin_path(bin).is_some();
                let mut label = RichText::new(name.as_str());
                if !installed {
                    label = label.weak();
                }
                let resp = ui
                    .selectable_label(self.selected == i, label)
                    .on_hover_text(if installed {
                        bin.clone()
                    } else {
                        format!("{bin} — not found on PATH")
                    });
                if resp.clicked() {
                    switch_to = Some(i);
                }
            }
        });
        if let Some(i) = switch_to {
            if !self.running() {
                self.switch_harness(i);
            }
        }
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        // controls strip
        let models = self.harness().map(|h| h.models.clone()).unwrap_or_default();
        let efforts = self
            .harness()
            .map(|h| h.efforts.clone())
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.add_sized(
                [46.0, 18.0],
                egui::Label::new(RichText::new("model").color(TEXT_DIM).size(12.0)),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.model)
                    .desired_width(190.0)
                    .hint_text("harness default")
                    .font(egui::TextStyle::Monospace),
            );
            egui::ComboBox::from_id_salt("model_pick")
                .selected_text("")
                .width(14.0)
                .show_ui(ui, |ui| {
                    if models.is_empty() {
                        ui.label(RichText::new("no presets in config").weak());
                    }
                    for m in &models {
                        if ui.selectable_label(self.model == *m, m.as_str()).clicked() {
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
            ui.add_space(10.0);
            ui.add_sized(
                [46.0, 18.0],
                egui::Label::new(RichText::new("effort").color(TEXT_DIM).size(12.0)),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.effort)
                    .desired_width(90.0)
                    .hint_text("—")
                    .font(egui::TextStyle::Monospace),
            );
            egui::ComboBox::from_id_salt("effort_pick")
                .selected_text("")
                .width(14.0)
                .show_ui(ui, |ui| {
                    if efforts.is_empty() {
                        ui.label(RichText::new("no effort presets for this harness").weak());
                    }
                    for e in &efforts {
                        if ui.selectable_label(self.effort == *e, e.as_str()).clicked() {
                            self.effort = e.clone();
                        }
                    }
                    if !efforts.is_empty() {
                        ui.separator();
                        if ui.button("clear").clicked() {
                            self.effort.clear();
                        }
                    }
                });
            ui.add_space(10.0);
            ui.add_sized(
                [46.0, 18.0],
                egui::Label::new(RichText::new("dir").color(TEXT_DIM).size(12.0)),
            );
            ui.add(egui::TextEdit::singleline(&mut self.cwd).desired_width(230.0));
            if ui.button("Browse").clicked() {
                let start = expand_tilde(&self.cwd);
                if let Some(dir) = rfd::FileDialog::new().set_directory(&start).pick_folder() {
                    self.cwd = dir.display().to_string();
                }
            }
        });
        ui.add_space(8.0);

        // prompt card
        egui::Frame::default()
            .fill(CARD)
            .stroke(Stroke::new(1.0, STROKE_SOFT))
            .inner_margin(Margin::same(10))
            .corner_radius(CornerRadius::same(12))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.prompt)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY)
                        .hint_text("What should the agent do?")
                        .frame(egui::Frame::default()),
                );
            });
        ui.add_space(7.0);

        // actions row
        let mut do_run = false;
        let mut do_cancel = false;
        let mut do_clear = false;
        ui.horizontal(|ui| {
            let can_run = self.can_run();
            let run_btn = egui::Button::new(RichText::new("Run").strong().size(14.0))
                .min_size(Vec2::new(92.0, 30.0))
                .fill(ACCENT);
            if ui
                .add_enabled(can_run, run_btn)
                .on_disabled_hover_text("pick an installed harness and write a prompt first")
                .on_hover_text(format!("run ({})", keys.run))
                .clicked()
            {
                do_run = true;
            }
            if self.running() {
                let stop_btn = egui::Button::new(RichText::new("Stop").strong())
                    .min_size(Vec2::new(76.0, 30.0))
                    .fill(Color32::from_rgb(82, 45, 52));
                if ui
                    .add(stop_btn)
                    .on_hover_text(format!("stop the agent ({})", keys.cancel))
                    .clicked()
                {
                    do_cancel = true;
                }
                ui.spinner();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut self.auto_scroll, "auto-scroll");
                if ui
                    .button("Clear")
                    .on_hover_text(format!("clear output ({})", keys.clear))
                    .clicked()
                {
                    do_clear = true;
                }
                ui.label(
                    RichText::new(format!("{} lines", self.out.len()))
                        .color(TEXT_DIM)
                        .size(11.5),
                );
            });
        });

        // command preview
        if let Some(h) = self.harness().cloned() {
            let model = self.model.trim();
            let effort = self.effort.trim();
            let mopt = if model.is_empty() { None } else { Some(model) };
            let eopt = if effort.is_empty() { None } else { Some(effort) };
            let argv = build_command(&h, "<prompt>", mopt, eopt);
            let mut prev = argv.join(" ");
            if prev.chars().count() > 170 {
                prev = format!("{}…", prev.chars().take(169).collect::<String>());
            }
            ui.label(RichText::new(prev).monospace().color(TEXT_DIM).size(11.5));
        }
        ui.add_space(6.0);

        // console
        egui::Frame::default()
            .fill(CONSOLE_BG)
            .stroke(Stroke::new(1.0, STROKE_SOFT))
            .inner_margin(Margin::same(10))
            .corner_radius(CornerRadius::same(12))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(self.auto_scroll)
                    .show(ui, |ui| {
                        if self.out.is_empty() {
                            ui.label(
                                RichText::new(
                                    "output streams here — pick a harness, type a prompt, hit Run",
                                )
                                .color(TEXT_DIM)
                                .monospace()
                                .size(12.0),
                            );
                        } else {
                            let text = self.out.join("\n");
                            ui.add(
                                egui::Label::new(RichText::new(text).monospace().size(12.0))
                                    .selectable(true),
                            );
                        }
                    });
            });

        if do_run && self.can_run() {
            let ctx = ui.ctx().clone();
            self.start_run(&ctx);
        }
        if do_cancel && self.running() {
            if let Some(r) = &self.runner {
                r.cancel();
            }
            self.status = "cancelling…".into();
            self.status_color = WARN;
        }
        if do_clear {
            self.out.clear();
        }
        if do_reload {
            self.reload_config();
        }
        if do_open_cfg {
            self.open_config();
        }
    }

    // ── config view ─────────────────────────────────────────────────────

    fn ui_config(&mut self, ui: &mut egui::Ui) {
        let mut back = false;
        egui::Frame::default()
            .fill(PANEL)
            .inner_margin(Margin::same(12))
            .corner_radius(CornerRadius::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Configuration")
                            .size(20.0)
                            .strong()
                            .color(TEXT_BRIGHT),
                    );
                    ui.label(
                        RichText::new(self.cfg_path.display().to_string())
                            .color(TEXT_DIM)
                            .size(11.5)
                            .monospace(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Back").clicked() {
                            back = true;
                        }
                        if ui.button("Reveal in Finder").clicked() {
                            let _ = StdCommand::new("open")
                                .arg("-R")
                                .arg(&self.cfg_path)
                                .spawn();
                        }
                    });
                });
            });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.cfg_tab, ConfigTab::Form, "Form");
            ui.selectable_value(&mut self.cfg_tab, ConfigTab::Raw, "Raw TOML");
            if let Some((msg, col)) = &self.cfg_msg {
                ui.add_space(8.0);
                ui.label(RichText::new(msg.as_str()).color(*col).size(12.0));
            }
        });
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(6.0);

        match self.cfg_tab {
            ConfigTab::Form => self.ui_config_form(ui),
            ConfigTab::Raw => self.ui_config_raw(ui),
        }

        if back || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.view = View::Main;
        }
    }

    fn ui_config_form(&mut self, ui: &mut egui::Ui) {
        let mut do_save = false;
        let mut do_revert = false;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(RichText::new("General").strong().size(14.0));
                ui.add_space(4.0);
                field_line(ui, "default dir", &mut self.form.default_dir, 260.0);

                ui.add_space(10.0);
                ui.label(RichText::new("Keybindings").strong().size(14.0));
                ui.label(
                    RichText::new(
                        "spec: cmd/ctrl/alt/shift + key — e.g. cmd+enter, cmd+., esc, cmd+k. empty = off",
                    )
                    .color(TEXT_DIM)
                    .size(11.0),
                );
                ui.add_space(2.0);
                {
                    let k = &mut self.form.keys;
                    field_line(ui, "run", &mut k.run, 200.0);
                    field_line(ui, "cancel", &mut k.cancel, 200.0);
                    field_line(ui, "clear", &mut k.clear, 200.0);
                    field_line(ui, "reload", &mut k.reload, 200.0);
                    field_line(ui, "config", &mut k.config, 200.0);
                }

                ui.add_space(10.0);
                ui.label(RichText::new("Harnesses").strong().size(14.0));
                ui.add_space(2.0);
                for (idx, hf) in self.form.harness.iter_mut().enumerate() {
                    egui::CollapsingHeader::new(RichText::new(hf.name.clone()).strong())
                        .id_salt(format!("hform_{idx}"))
                        .show(ui, |ui| {
                            field_line(ui, "name", &mut hf.name, 200.0);
                            field_line(ui, "bin", &mut hf.bin, 200.0);
                            field_block(
                                ui,
                                "args  (argv template; {prompt} = your prompt)",
                                &mut hf.args,
                                3,
                            );
                            field_block(
                                ui,
                                "model_args  (appended when a model is set)",
                                &mut hf.model_args,
                                2,
                            );
                            field_block(
                                ui,
                                "models  (picker presets, one per line)",
                                &mut hf.models,
                                3,
                            );
                            field_block(
                                ui,
                                "efforts  (effort presets, one per line)",
                                &mut hf.efforts,
                                2,
                            );
                            field_block(
                                ui,
                                "effort_args  (appended when an effort is set)",
                                &mut hf.effort_args,
                                2,
                            );
                        });
                }
                ui.add_space(10.0);
                ui.label(
                    RichText::new(
                        "Saving from the form rewrites the file; comments in the raw file are lost.",
                    )
                    .color(TEXT_DIM)
                    .size(11.0),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let save_btn = egui::Button::new(RichText::new("Save & Reload").strong())
                        .fill(ACCENT)
                        .min_size(Vec2::new(130.0, 28.0));
                    if ui.add(save_btn).clicked() {
                        do_save = true;
                    }
                    if ui.button("Revert form").clicked() {
                        do_revert = true;
                    }
                });
            });

        if do_save {
            self.save_form();
        }
        if do_revert {
            self.form = FormState::from_config(&self.cfg);
        }
    }

    fn ui_config_raw(&mut self, ui: &mut egui::Ui) {
        let mut do_save = false;
        let mut do_reload_file = false;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.raw_text)
                        .code_editor()
                        .desired_rows(28)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let save_btn = egui::Button::new(RichText::new("Save & Reload").strong())
                        .fill(ACCENT)
                        .min_size(Vec2::new(130.0, 28.0));
                    if ui.add(save_btn).clicked() {
                        do_save = true;
                    }
                    if ui.button("Reload from disk").clicked() {
                        do_reload_file = true;
                    }
                });
            });
        if do_save {
            self.save_raw();
        }
        if do_reload_file {
            self.raw_text = std::fs::read_to_string(&self.cfg_path)
                .unwrap_or_else(|_| config::DEFAULT_CONFIG.to_string());
            self.cfg_msg = Some(("reloaded file into editor".into(), TEXT_DIM));
        }
    }

    fn save_form(&mut self) {
        let cfg = self.form.to_config();
        match config::serialize(&cfg) {
            Ok(text) => match std::fs::write(&self.cfg_path, text) {
                Ok(_) => {
                    self.reload_config();
                    self.form = FormState::from_config(&self.cfg);
                    self.cfg_msg = Some(("saved & reloaded".into(), OK_GREEN));
                }
                Err(e) => self.cfg_msg = Some((format!("write failed: {e}"), ERR)),
            },
            Err(e) => self.cfg_msg = Some((format!("serialize failed: {e}"), ERR)),
        }
    }

    fn save_raw(&mut self) {
        match toml::from_str::<Config>(&self.raw_text) {
            Ok(_) => {
                if let Some(dir) = self.cfg_path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                match std::fs::write(&self.cfg_path, &self.raw_text) {
                    Ok(_) => {
                        self.reload_config();
                        self.form = FormState::from_config(&self.cfg);
                        self.cfg_msg = Some(("saved & reloaded".into(), OK_GREEN));
                    }
                    Err(e) => self.cfg_msg = Some((format!("write failed: {e}"), ERR)),
                }
            }
            Err(e) => self.cfg_msg = Some((format!("TOML error — not saved: {e}"), ERR)),
        }
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

        egui::Frame::default()
            .inner_margin(Margin::symmetric(14, 10))
            .show(ui, |ui| {
        match self.view {
            View::Main => {
                let keys = self.cfg.keys.clone();
                let want_run = binding_pressed(ui, &keys.run);
                let want_cancel = binding_pressed(ui, &keys.cancel);
                let want_clear = binding_pressed(ui, &keys.clear);
                let want_reload = binding_pressed(ui, &keys.reload);
                let want_config = binding_pressed(ui, &keys.config);

                self.ui_main(ui);

                if want_config {
                    self.open_config();
                } else {
                    if want_reload {
                        self.reload_config();
                    }
                    if want_clear {
                        self.out.clear();
                    }
                    if want_cancel && self.running() {
                        if let Some(r) = &self.runner {
                            r.cancel();
                        }
                        self.status = "cancelling…".into();
                        self.status_color = WARN;
                    }
                    if want_run && self.can_run() {
                        self.start_run(&ctx);
                    }
                }
            }
            View::Config => {
                self.ui_config(ui);
                if binding_pressed(ui, &self.cfg.keys.config) {
                    self.view = View::Main;
                }
            }
        }
        });
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

// ── small ui helpers ────────────────────────────────────────────────────

fn status_dot(ui: &mut egui::Ui, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(9.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.5, color);
}

fn field_line(ui: &mut egui::Ui, caption: &str, value: &mut String, width: f32) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [92.0, 18.0],
            egui::Label::new(RichText::new(caption).color(TEXT_DIM).size(12.0)),
        );
        ui.add(egui::TextEdit::singleline(value).desired_width(width));
    });
}

fn field_block(ui: &mut egui::Ui, caption: &str, value: &mut String, rows: usize) {
    ui.label(RichText::new(caption).color(TEXT_DIM).size(11.5));
    ui.add(
        egui::TextEdit::multiline(value)
            .code_editor()
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
}
