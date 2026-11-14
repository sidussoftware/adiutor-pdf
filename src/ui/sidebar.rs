//! Left sidebar: app name, add button, document list, language and theme.

use super::*;
use crate::workspace::{DocId, DocSummary};
use egui::{Align, Layout, RichText, Sense, Stroke, vec2};
use style::palette;

pub(super) fn semibold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text)
        .family(egui::FontFamily::Name("semibold".into()))
        .size(size)
}

/// Dashed rectangle outline over a button.
fn dashed_border(ui: &egui::Ui, rect: egui::Rect, color: egui::Color32) {
    let pts = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    ui.painter().extend(egui::Shape::dashed_line(
        &pts,
        Stroke::new(1.0, color),
        5.0,
        4.0,
    ));
}

/// The document rows; the empty hint is shown when there are none.
/// Returns the id of the document whose × was clicked.
pub(super) fn doc_list(ui: &mut egui::Ui, tr: &Tr, docs: &[DocSummary]) -> Option<DocId> {
    let pal = palette(ui);
    let mut removed = None;
    if docs.is_empty() {
        ui.label(RichText::new(tr.t("sidebar.empty")).color(pal.text_secondary));
        return None;
    }
    for d in docs {
        let count = d.pages_in_grid.to_string();
        let key = if d.pages_in_grid == 1 {
            "sidebar.pages_one"
        } else {
            "sidebar.pages_many"
        };
        egui::Frame::new()
            .fill(pal.card)
            .stroke(Stroke::new(1.0, pal.border))
            .corner_radius(8)
            .inner_margin(10)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    // The letter identifies the document without relying on colour.
                    ui.label(semibold(&d.letter, 14.0).color(pal.text));
                    let (sw, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                    ui.painter()
                        .rect_filled(sw, 3.0, pal.doc_colors[d.id % pal.doc_colors.len()]);
                    // Leave room for the 40 px x button at the right end.
                    let text_w = ui.available_width() - 40.0 - ui.spacing().item_spacing.x;
                    ui.vertical(|ui| {
                        ui.set_width(text_w);
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.add(egui::Label::new(RichText::new(&d.name).color(pal.text)).truncate())
                            .on_hover_text(&d.name);
                        ui.label(
                            RichText::new(tr.tf(key, &[("count", &count)]))
                                .size(12.0)
                                .color(pal.text_secondary),
                        );
                    });
                    let name = tr.tf("sidebar.remove", &[("file", &d.name)]);
                    if icons::icon_button(ui, icons::close, &name).clicked() {
                        removed = Some(d.id);
                    }
                });
            });
        ui.add_space(4.0);
    }
    removed
}

impl AdiutorApp {
    pub(super) fn sidebar(&mut self, ui: &mut egui::Ui, docs: &[DocSummary]) {
        let pal = palette(ui);
        let frame = egui::Frame::new()
            .fill(pal.sidebar)
            .inner_margin(egui::Margin {
                left: 16,
                right: 16,
                top: 20,
                bottom: 20,
            });
        egui::Panel::left("sidebar")
            .exact_size(264.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    self.settings_combos(ui);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        self.sidebar_top(ui, docs)
                    });
                });
            });
    }

    fn sidebar_top(&mut self, ui: &mut egui::Ui, docs: &[DocSummary]) {
        let pal = palette(ui);
        ui.spacing_mut().item_spacing.y = 16.0;
        ui.label(semibold("adiutor-pdf", 15.0).color(pal.text));
        self.add_button(ui);
        ui.horizontal(|ui| {
            ui.label(
                semibold(self.tr.t("sidebar.documents").to_uppercase(), 12.0)
                    .color(pal.text_secondary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let enabled = !docs.is_empty() && self.batch.is_none();
                let clear = ui.add_enabled(enabled, egui::Button::new(self.tr.t("sidebar.clear")));
                if clear.clicked() {
                    if self.workspace.is_dirty() {
                        self.clear_prompt = true;
                    } else {
                        self.clear_all();
                    }
                }
            });
        });
        let mut removed = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                removed = doc_list(ui, &self.tr, docs);
            });
        if let Some(id) = removed {
            self.remove_doc(id, docs);
        }
    }

    fn add_button(&mut self, ui: &mut egui::Ui) {
        let pal = palette(ui);
        let text = self.tr.t("toolbar.add_pdf");
        let size = vec2(ui.available_width(), 44.0);
        let r = ui
            .allocate_ui_with_layout(
                size,
                Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    ui.add(
                        // No fixed fill, so the hovered state shows.
                        egui::Button::new((icons::glyph(icons::plus), text))
                            .stroke(Stroke::NONE)
                            .corner_radius(8),
                    )
                },
            )
            .inner;
        if r.has_focus() {
            ui.painter().rect_stroke(
                r.rect,
                8,
                Stroke::new(2.0, pal.text),
                egui::StrokeKind::Inside,
            );
        } else {
            let c = if r.hovered() {
                pal.text_secondary
            } else {
                pal.border_strong
            };
            dashed_border(ui, r.rect, c);
        }
        if r.clicked() {
            self.pick_pdfs();
        }
    }

    /// Opens the file picker and adds the chosen PDFs; shared by the sidebar and the empty state.
    pub(super) fn pick_pdfs(&mut self) {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .pick_files()
        {
            for path in files {
                self.open(path);
            }
        }
    }

    /// Language and theme selectors, pinned to the bottom.
    fn settings_combos(&mut self, ui: &mut egui::Ui) {
        let mut next = self.settings.clone();
        // Bottom-up: the theme row first, so Language ends up above it.
        ui.spacing_mut().item_spacing.y = 8.0;
        ui.horizontal(|ui| {
            let label = ui.add_sized(
                vec2(64.0, 40.0),
                egui::Label::new(self.tr.t("settings.theme")),
            );
            egui::ComboBox::from_id_salt("theme")
                .selected_text(self.theme_label(next.theme))
                .show_ui(ui, |ui| {
                    for p in [ThemePref::System, ThemePref::Light, ThemePref::Dark] {
                        ui.selectable_value(&mut next.theme, p, self.theme_label(p));
                    }
                })
                .response
                .labelled_by(label.id);
        });
        ui.horizontal(|ui| {
            let label = ui.add_sized(
                vec2(64.0, 40.0),
                egui::Label::new(self.tr.t("settings.language")),
            );
            egui::ComboBox::from_id_salt("language")
                .selected_text(self.lang_label(next.language))
                .show_ui(ui, |ui| {
                    for p in LanguagePref::ALL {
                        ui.selectable_value(&mut next.language, p, self.lang_label(p));
                    }
                })
                .response
                .labelled_by(label.id);
        });
        if next.language != self.settings.language {
            self.tr = Tr::new(next.language.resolve());
            let title = self.tr.t("app.title").to_string();
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        if next != self.settings {
            let ctx = ui.ctx().clone();
            self.set_settings(&ctx, next);
        }
    }

    fn theme_label(&self, p: ThemePref) -> &str {
        match p {
            ThemePref::System => self.tr.t("theme.system"),
            ThemePref::Light => self.tr.t("theme.light"),
            ThemePref::Dark => self.tr.t("theme.dark"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn sidebar_lists_documents_with_letters_and_counts() {
        let tr = Tr::new(Lang::En);
        let docs = vec![
            DocSummary {
                id: 0,
                letter: "A".into(),
                name: "contratto.pdf".into(),
                pages_in_grid: 8,
            },
            DocSummary {
                id: 1,
                letter: "B".into(),
                name: "allegato.pdf".into(),
                pages_in_grid: 1,
            },
        ];
        let mut h = Harness::builder()
            .with_context_setup(crate::install_fonts)
            .build_ui(move |ui| {
                doc_list(ui, &tr, &docs);
            });
        h.run();
        h.get_by_label("A");
        h.get_by_label("contratto.pdf");
        h.get_by_label("8 pages");
        h.get_by_label("B");
        h.get_by_label("1 page");
    }

    #[test]
    fn remove_button_has_accessible_name() {
        let tr = Tr::new(Lang::It);
        let docs = vec![DocSummary {
            id: 7,
            letter: "A".into(),
            name: "contratto.pdf".into(),
            pages_in_grid: 2,
        }];
        let mut removed = None;
        let mut h = Harness::builder()
            .with_context_setup(crate::install_fonts)
            .build_ui(|ui| {
                if let Some(id) = doc_list(ui, &tr, &docs) {
                    removed = Some(id);
                }
            });
        h.run();
        h.get_by_label("Togli contratto.pdf").click();
        h.run();
        drop(h);
        assert_eq!(removed, Some(7));
    }
}
