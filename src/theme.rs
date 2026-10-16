//! Aparência do aplicativo: paleta (claro/escuro), cantos arredondados,
//! espaçamento, tipografia e fontes.
//!
//! Tudo que depende de nomes da API de estilo do egui fica neste arquivo
//! (escrito para egui 0.31+). Se uma atualização do egui renomear algo, é aqui
//! que se corrige; o resto do aplicativo só usa `Palette`.
//!
//! Fontes: não há nenhuma embutida (nenhuma dependência nova). Em tempo de
//! execução o app procura, nesta ordem, `fonts/ui.ttf` (ao lado do executável),
//! `assets/fonts/ui.ttf` (diretório atual) e fontes do sistema (Segoe UI no
//! Windows). Para o texto monoespaçado, o mesmo com `mono.ttf` e Cascadia Mono /
//! Consolas. Se nada for encontrado, mantém as fontes padrão do egui.

use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle};
use std::path::PathBuf;
use std::sync::Arc;

/// Cores usadas pelo aplicativo nos dois temas.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface_alt: Color32,
    pub input_bg: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub accent_dark: Color32,
    pub on_accent: Color32,
    pub success: Color32,
    pub success_soft: Color32,
    pub danger: Color32,
    pub on_danger: Color32,
    pub console_bg: Color32,
    pub console_text: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            bg: rgb(0x0F172A),
            surface: rgb(0x1E293B),
            surface_alt: rgb(0x273449),
            input_bg: rgb(0x0B1220),
            border: rgb(0x334155),
            text: rgb(0xE2E8F0),
            muted: rgb(0x94A3B8),
            accent: rgb(0x818CF8),
            accent_soft: rgb(0x312E81),
            accent_dark: rgb(0xC7D2FE),
            on_accent: rgb(0x0F172A),
            success: rgb(0x4ADE80),
            success_soft: rgb(0x14532D),
            danger: rgb(0xF87171),
            on_danger: rgb(0x0F172A),
            console_bg: rgb(0x020617),
            console_text: rgb(0xE2E8F0),
        }
    } else {
        Palette {
            bg: rgb(0xF3F5FA),
            surface: rgb(0xFFFFFF),
            surface_alt: rgb(0xF1F5F9),
            input_bg: rgb(0xF8FAFC),
            border: rgb(0xE2E8F0),
            text: rgb(0x1E293B),
            muted: rgb(0x64748B),
            accent: rgb(0x4F46E5),
            accent_soft: rgb(0xE0E7FF),
            accent_dark: rgb(0x3730A3),
            on_accent: rgb(0xFFFFFF),
            success: rgb(0x15803D),
            success_soft: rgb(0xDCFCE7),
            danger: rgb(0xDC2626),
            on_danger: rgb(0xFFFFFF),
            console_bg: rgb(0x0F172A),
            console_text: rgb(0xE2E8F0),
        }
    }
}

/// Instala fontes e os dois estilos (claro e escuro); inicia no tema claro.
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    ctx.set_style_of(egui::Theme::Light, build_style(false));
    ctx.set_style_of(egui::Theme::Dark, build_style(true));
    set_dark(ctx, false);
}

pub fn set_dark(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark { egui::Theme::Dark } else { egui::Theme::Light });
}

fn build_style(dark: bool) -> egui::Style {
    let p = palette(dark);
    let mut style = egui::Style {
        visuals: if dark { egui::Visuals::dark() } else { egui::Visuals::light() },
        ..Default::default()
    };

    // Tipografia
    let proportional = |size: f32| FontId::new(size, FontFamily::Proportional);
    style.text_styles.insert(TextStyle::Heading, proportional(22.0));
    style.text_styles.insert(TextStyle::Body, proportional(14.5));
    style.text_styles.insert(TextStyle::Button, proportional(14.5));
    style.text_styles.insert(TextStyle::Small, proportional(12.0));
    style.text_styles.insert(TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace));

    // Espaçamento mais arejado
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.window_margin = Margin::same(14);
    style.spacing.menu_margin = Margin::same(8);

    // Cores e cantos
    let v = &mut style.visuals;
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = p.input_bg;
    v.faint_bg_color = p.surface_alt;
    v.code_bg_color = p.surface_alt;
    v.hyperlink_color = p.accent;
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0, p.border);
    let shadow = egui::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 120 } else { 36 }),
    };
    v.window_shadow = shadow;
    v.popup_shadow = shadow;

    // Texto selecionado e itens marcados (aba ativa, opção escolhida no combo)
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent_dark);

    let radius = CornerRadius::same(8);
    let w = &mut v.widgets;

    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = CornerRadius::same(10);

    w.inactive.bg_fill = p.surface_alt;
    w.inactive.weak_bg_fill = p.surface;
    w.inactive.bg_stroke = Stroke::new(1.0, p.border);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = p.accent_soft;
    w.hovered.weak_bg_fill = p.accent_soft;
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    w.hovered.fg_stroke = Stroke::new(1.5, p.accent_dark);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.5;

    w.active.bg_fill = p.accent;
    w.active.weak_bg_fill = p.accent;
    w.active.bg_stroke = Stroke::new(1.0, p.accent);
    w.active.fg_stroke = Stroke::new(1.5, p.on_accent);
    w.active.corner_radius = radius;

    w.open.bg_fill = p.accent_soft;
    w.open.weak_bg_fill = p.accent_soft;
    w.open.bg_stroke = Stroke::new(1.0, p.accent);
    w.open.fg_stroke = Stroke::new(1.0, p.accent_dark);
    w.open.corner_radius = radius;

    style
}

// ----------------------------------------------------------------------
// Fontes
// ----------------------------------------------------------------------

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut changed = false;

    if let Some(bytes) = first_readable(&ui_font_candidates()) {
        add_font(&mut fonts, "kubectlutil-ui", bytes, FontFamily::Proportional);
        changed = true;
    }
    if let Some(bytes) = first_readable(&mono_font_candidates()) {
        add_font(&mut fonts, "kubectlutil-mono", bytes, FontFamily::Monospace);
        changed = true;
    }

    if changed {
        ctx.set_fonts(fonts);
    }
}

/// A fonte nova vai para a frente da família; as fontes padrão do egui continuam
/// atrás e cobrem os símbolos que ela não tiver (setas, ◈, ●, emojis).
fn add_font(fonts: &mut egui::FontDefinitions, name: &str, bytes: Vec<u8>, family: FontFamily) {
    fonts.font_data.insert(name.to_owned(), Arc::new(egui::FontData::from_owned(bytes)));
    fonts.families.entry(family).or_default().insert(0, name.to_owned());
}

fn first_readable(paths: &[PathBuf]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| std::fs::read(path).ok())
}

/// `fonts/<file>` ao lado do executável e `assets/fonts/<file>` no diretório atual.
fn custom_font_paths(file: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.to_path_buf())) {
        paths.push(dir.join("fonts").join(file));
    }
    paths.push(PathBuf::from("assets").join("fonts").join(file));
    paths
}

fn windows_font(file: &str) -> Option<PathBuf> {
    std::env::var_os("WINDIR").map(|dir| PathBuf::from(dir).join("Fonts").join(file))
}

fn ui_font_candidates() -> Vec<PathBuf> {
    let mut paths = custom_font_paths("ui.ttf");
    paths.extend(windows_font("segoeui.ttf"));
    paths.extend(
        [
            "/usr/share/fonts/truetype/inter/Inter-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
        ]
        .map(PathBuf::from),
    );
    paths
}

fn mono_font_candidates() -> Vec<PathBuf> {
    let mut paths = custom_font_paths("mono.ttf");
    paths.extend(windows_font("CascadiaMono.ttf"));
    paths.extend(windows_font("consola.ttf"));
    paths.extend(
        [
            "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
        ]
        .map(PathBuf::from),
    );
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color32) -> f64 {
        let channel = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn both_palettes_keep_text_readable() {
        for dark in [false, true] {
            let p = palette(dark);
            let checks = [
                ("text/bg", p.text, p.bg, 4.5),
                ("text/surface", p.text, p.surface, 4.5),
                ("text/surface_alt", p.text, p.surface_alt, 4.5),
                ("text/input_bg", p.text, p.input_bg, 4.5),
                ("muted/surface", p.muted, p.surface, 4.0),
                ("muted/bg", p.muted, p.bg, 4.0),
                ("accent_dark/accent_soft", p.accent_dark, p.accent_soft, 4.5),
                ("on_accent/accent", p.on_accent, p.accent, 4.5),
                ("on_danger/danger", p.on_danger, p.danger, 4.5),
                ("success/success_soft", p.success, p.success_soft, 4.0),
                ("console_text/console_bg", p.console_text, p.console_bg, 7.0),
            ];
            for (name, fg, bg, minimum) in checks {
                let ratio = contrast(fg, bg);
                assert!(ratio >= minimum, "tema {} — {name}: contraste {ratio:.2} < {minimum}", if dark { "escuro" } else { "claro" });
            }
        }
    }

    #[test]
    fn rgb_helper_matches_from_rgb() {
        assert_eq!(rgb(0x4F46E5), Color32::from_rgb(0x4F, 0x46, 0xE5));
    }

    #[test]
    fn style_uses_palette_and_rounded_widgets() {
        let style = build_style(false);
        assert_eq!(style.visuals.panel_fill, palette(false).bg);
        assert_eq!(style.visuals.widgets.inactive.corner_radius, CornerRadius::same(8));
        assert!(build_style(true).visuals.dark_mode);
    }

    #[test]
    fn font_candidates_include_custom_paths_first() {
        let paths = ui_font_candidates();
        assert!(paths.iter().any(|p| p.ends_with("fonts/ui.ttf") || p.ends_with("fonts\\ui.ttf")));
        assert!(first_readable(&[PathBuf::from("/caminho/que/nao/existe.ttf")]).is_none());
    }
}
