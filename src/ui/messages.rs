//! Messages and the bottom bar.

use super::*;

/// An i18n key and its placeholder values, translated when drawn so a language switch applies.
pub(super) type Message = (&'static str, Vec<(&'static str, String)>);

pub(super) fn message_text(tr: &Tr, (key, args): &Message) -> String {
    let args: Vec<(&str, &str)> = args.iter().map(|(n, v)| (*n, v.as_str())).collect();
    tr.tf(key, &args)
}

impl AdiutorApp {
    pub(super) fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        if let Some(b) = &self.batch {
            let (done, total) = b.total_progress();
            let text = self.tr.tf(
                "save.progress",
                &[("done", &done.to_string()), ("total", &total.to_string())],
            );
            let frac = done as f32 / total.max(1) as f32;
            ui.add(egui::ProgressBar::new(frac).text(text));
        }
        let mut dismiss = None;
        for (i, message) in self.messages.iter().enumerate() {
            ui.horizontal(|ui| {
                if icons::button(ui, true, icons::close, self.tr.t("common.close")).clicked() {
                    dismiss = Some(i);
                }
                ui.label(message_text(&self.tr, message));
            });
        }
        if let Some(i) = dismiss {
            self.messages.remove(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;

    #[test]
    fn messages_are_translated_when_drawn() {
        let m: Message = ("save.done", vec![("file", "a.pdf".into())]);
        assert_eq!(message_text(&Tr::new(Lang::En), &m), "Saved a.pdf");
        assert_eq!(message_text(&Tr::new(Lang::It), &m), "Salvato a.pdf");
    }
}
