//! Modal dialogs.

use super::*;

/// Modal with a title, a text, a confirm button and Cancel (keys are i18n keys).
/// Returns Some(true) on confirm, Some(false) on Cancel, Esc or a click outside, None while
/// unanswered.
fn confirm_modal(
    ctx: &egui::Context,
    tr: &Tr,
    id: &str,
    title_key: &str,
    body: &str,
    confirm_key: &str,
) -> Option<bool> {
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::NULL.with(id)).show(ctx, |ui| {
        ui.heading(tr.t(title_key));
        ui.label(body);
        ui.horizontal(|ui| {
            if ui.button(tr.t(confirm_key)).clicked() {
                answer = Some(true);
            }
            if ui.button(tr.t("common.cancel")).clicked() {
                answer = Some(false);
            }
        });
    });
    answer.or_else(|| modal.should_close().then_some(false))
}

impl AdiutorApp {
    pub(super) fn password_modal(&mut self, ctx: &egui::Context) {
        let Some((path, wrong)) = self.passwords.front().cloned() else {
            return;
        };
        let (mut ok, mut cancel) = (false, false);
        let modal = egui::Modal::new(egui::Id::NULL.with("password")).show(ctx, |ui| {
            ui.heading(self.tr.t("password.title"));
            ui.label(
                self.tr
                    .tf("password.prompt", &[("file", &file_name(&path))]),
            );
            if wrong {
                ui.colored_label(ui.visuals().error_fg_color, self.tr.t("password.wrong"));
            }
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.password_text)
                    .password(true)
                    .align(egui::Align2::LEFT_CENTER),
            );
            ok = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            edit.request_focus();
            ui.horizontal(|ui| {
                ok |= ui.button(self.tr.t("password.ok")).clicked();
                cancel = ui.button(self.tr.t("common.cancel")).clicked();
            });
        });
        cancel |= !ok && modal.should_close();
        if ok || cancel {
            self.passwords.pop_front();
            let password = std::mem::take(&mut self.password_text);
            if ok {
                self.send(Command::Open {
                    path,
                    password: Some(password),
                });
            }
        }
    }

    pub(super) fn overwrite_modal(&mut self, ctx: &egui::Context) {
        let Some((_, dest)) = &self.pending_overwrite else {
            return;
        };
        let body = self
            .tr
            .tf("save.overwrite_body", &[("file", &file_name(dest))]);
        let answer = confirm_modal(
            ctx,
            &self.tr,
            "overwrite",
            "save.overwrite_title",
            &body,
            "save.overwrite_confirm",
        );
        match answer {
            Some(true) => {
                if let Some(job) = self.pending_overwrite.take() {
                    self.start_batch(vec![job], None);
                }
            }
            Some(false) => self.pending_overwrite = None,
            None => {}
        }
    }

    pub(super) fn blocked_modal(&mut self, ctx: &egui::Context) {
        if self.blocked_save.is_none() {
            return;
        }
        let body = self
            .tr
            .tf("save.blocked_body", &[("file", &self.blocked_file)]);
        let answer = confirm_modal(
            ctx,
            &self.tr,
            "blocked",
            "save.blocked_title",
            &body,
            "save.blocked_rename",
        );
        match answer {
            // Closed before reopening the dialog, which blocks until it is dismissed.
            Some(true) => {
                if let Some(selection_only) = self.blocked_save.take() {
                    self.save(selection_only);
                }
            }
            Some(false) => self.blocked_save = None,
            None => {}
        }
    }

    pub(super) fn close_modal(&mut self, ctx: &egui::Context) {
        if !self.close_prompt {
            return;
        }
        let answer = confirm_modal(
            ctx,
            &self.tr,
            "close",
            "close.unsaved_title",
            self.tr.t("close.unsaved_body"),
            "close.confirm",
        );
        match answer {
            Some(true) => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(false) => self.close_prompt = false,
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Lang;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// Shows the overwrite modal in a harness; the state keeps its first answer.
    fn overwrite_harness() -> Harness<'static, Option<bool>> {
        let tr = Tr::new(Lang::En);
        let mut harness = Harness::new_ui_state(
            move |ui, answer: &mut Option<bool>| {
                let a = confirm_modal(
                    ui.ctx(),
                    &tr,
                    "overwrite",
                    "save.overwrite_title",
                    "out.pdf already exists. Replace it?",
                    "save.overwrite_confirm",
                );
                *answer = answer.or(a);
            },
            None,
        );
        harness.run();
        assert_eq!(*harness.state(), None, "no answer before input");
        harness
    }

    #[test]
    fn overwrite_modal_replace() {
        let mut harness = overwrite_harness();
        harness.get_by_label("Replace").click();
        harness.run();
        assert_eq!(*harness.state(), Some(true));
    }

    #[test]
    fn overwrite_modal_cancel() {
        let mut harness = overwrite_harness();
        harness.get_by_label("Cancel").click();
        harness.run();
        assert_eq!(*harness.state(), Some(false));
    }

    #[test]
    fn overwrite_modal_click_outside_cancels() {
        let mut harness = overwrite_harness();
        let corner = egui::pos2(2.0, 2.0);
        harness.hover_at(corner);
        harness.drag_at(corner);
        harness.drop_at(corner);
        harness.run();
        assert_eq!(*harness.state(), Some(false));
    }

    #[test]
    fn overwrite_modal_escape_cancels() {
        let mut harness = overwrite_harness();
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert_eq!(*harness.state(), Some(false));
    }

    #[test]
    fn blocked_modal_buttons_and_escape() {
        let make = || {
            let tr = Tr::new(Lang::En);
            let mut harness = Harness::new_ui_state(
                move |ui, answer: &mut Option<bool>| {
                    let a = confirm_modal(
                        ui.ctx(),
                        &tr,
                        "blocked",
                        "save.blocked_title",
                        "a.pdf is one of the open files. The original is never modified.",
                        "save.blocked_rename",
                    );
                    *answer = answer.or(a);
                },
                None,
            );
            harness.run();
            harness
        };
        let mut h = make();
        h.get_by_label("Choose another name").click();
        h.run();
        assert_eq!(*h.state(), Some(true));
        let mut h = make();
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(*h.state(), Some(false));
        let mut h = make();
        h.key_press(egui::Key::Escape);
        h.run();
        assert_eq!(*h.state(), Some(false));
    }
}
