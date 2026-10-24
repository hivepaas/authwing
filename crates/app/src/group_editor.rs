//! The group editor: a group's name and color, as shellwing's edits its
//! hosts' groups. Renaming a group takes its codes along; deleting one,
//! after a question, keeps its codes, under no group.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Render,
    SharedString, Window, div, prelude::*, px, rgb,
};

use authwing_i18n::{tr, tr_n};

use crate::colors::colors;
use crate::groups::{self, COLORS, Group};
use crate::input::{Changed, TextInput, said};
use crate::look::ui;
use crate::widgets::{outline_button, primary_button};

pub enum GroupEvent {
    Close,
    /// Save a new group (`old` None), or `old` as `name` in `color`.
    Save {
        old: Option<Group>,
        name: String,
        color: String,
    },
    Delete(Group),
}

pub struct GroupEditor {
    /// The group as opened; None for a new one.
    old: Option<Group>,
    /// The other groups' names, which this one may not take.
    taken: Vec<String>,
    name: Entity<TextInput>,
    color: String,
    problem: Option<String>,
    asking_delete: bool,
    focus: FocusHandle,
}

impl EventEmitter<GroupEvent> for GroupEditor {}

impl Focusable for GroupEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl GroupEditor {
    /// Edits `old`, or makes a new group; `all` are the groups there are.
    pub fn new(old: Option<Group>, all: &[Group], cx: &mut Context<Self>) -> Self {
        let name = said(cx, || tr!("Work, Money, Family…"));
        if let Some(g) = &old {
            name.update(cx, |f, cx| f.set_text(&g.name, 0..g.name.len(), cx));
        }
        cx.subscribe(&name, |this: &mut GroupEditor, _, _: &Changed, cx| {
            this.problem = None;
            cx.notify();
        })
        .detach();
        let color = match &old {
            Some(g) => g.color.clone(),
            // The first color no group has yet.
            None => COLORS
                .iter()
                .map(|(n, _)| *n)
                .find(|n| !all.iter().any(|g| g.color == *n))
                .unwrap_or(COLORS[0].0)
                .to_string(),
        };
        let taken = all
            .iter()
            .filter(|g| old.as_ref().is_none_or(|o| o.name != g.name))
            .map(|g| g.name.to_lowercase())
            .collect();
        GroupEditor {
            old,
            taken,
            name,
            color,
            problem: None,
            asking_delete: false,
            focus: cx.focus_handle(),
        }
    }

    /// The name field's focus, where the cursor goes first.
    pub fn name_focus(&self, cx: &App) -> FocusHandle {
        self.name.focus_handle(cx)
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let name = self.name.read(cx).text().trim().to_string();
        if name.is_empty() {
            self.problem = Some(tr!("Give the group a name."));
        } else if self.taken.contains(&name.to_lowercase()) {
            self.problem = Some(tr!("There is a group of that name already."));
        } else {
            cx.emit(GroupEvent::Save {
                old: self.old.clone(),
                name,
                color: self.color.clone(),
            });
            return;
        }
        cx.notify();
    }

    fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match ev.keystroke.key.as_str() {
            "enter" if self.asking_delete => {
                if let Some(g) = self.old.clone() {
                    cx.emit(GroupEvent::Delete(g));
                }
            }
            "enter" => self.submit(cx),
            "escape" if self.asking_delete => {
                self.asking_delete = false;
                cx.notify();
            }
            "escape" => cx.emit(GroupEvent::Close),
            _ => return,
        }
        cx.stop_propagation();
    }
}

impl Render for GroupEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = colors();
        let mut swatches = div().flex().flex_wrap().gap(px(8.));
        for (name, rgb_value) in COLORS {
            let on = self.color == *name;
            swatches = swatches.child(
                div()
                    .id(SharedString::from(format!("group-color-{name}")))
                    .size(px(26.))
                    .rounded(px(7.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .border_2()
                    .border_color(rgb(if on { c.text } else { c.surface }))
                    .hover(|s| s.border_color(rgb(c.soft)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.color = name.to_string();
                        cx.notify();
                    }))
                    .child(div().size(px(16.)).rounded(px(4.)).bg(rgb(*rgb_value))),
            );
        }
        let title = if self.old.is_some() {
            tr!("Edit the group")
        } else {
            tr!("New group")
        };
        let count = self.old.as_ref().map_or(0, |g| g.count);
        // The question has its answers alone; the saving buttons otherwise.
        let buttons =
            if self.asking_delete {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .text_size(ui(12.))
                            .text_color(rgb(c.soft))
                            .child(tr_n!(
                                count,
                                "Delete the group? Its code stays, under no group.",
                                "Delete the group? Its {n} codes stay, under no group."
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.))
                            .child(outline_button("group-keep", &tr!("Cancel")).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.asking_delete = false;
                                    cx.notify();
                                }),
                            ))
                            .child(primary_button("group-delete-yes", &tr!("Delete")).on_click(
                                cx.listener(|this, _, _, cx| {
                                    if let Some(g) = this.old.clone() {
                                        cx.emit(GroupEvent::Delete(g));
                                    }
                                }),
                            )),
                    )
            } else {
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .when(self.old.is_some(), |d| {
                        d.child(outline_button("group-delete", &tr!("Delete")).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.asking_delete = true;
                                cx.notify();
                            }),
                        ))
                    })
                    .child(div().flex_1())
                    .child(
                        outline_button("group-cancel", &tr!("Cancel"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(GroupEvent::Close))),
                    )
                    .child(
                        primary_button("group-save", &tr!("Save"))
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    )
            };
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .w(px(420.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .p(px(20.))
            .rounded(px(12.))
            .bg(rgb(c.surface))
            .border_1()
            .border_color(rgb(c.edge))
            .text_color(rgb(c.text))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .size(px(12.))
                            .rounded(px(3.))
                            .bg(rgb(groups::rgb(&self.color))),
                    )
                    .child(
                        div()
                            .text_size(ui(16.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .child(
                        div()
                            .text_size(ui(12.))
                            .text_color(rgb(c.label))
                            .child(tr!("Name")),
                    )
                    .child(
                        div()
                            .h(px(32.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .bg(rgb(c.field))
                            .border_1()
                            .border_color(rgb(c.field_edge))
                            .child(self.name.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(ui(12.))
                            .text_color(rgb(c.label))
                            .child(tr!("Color")),
                    )
                    .child(swatches),
            )
            .when_some(self.problem.clone(), |d, p| {
                d.child(div().text_size(ui(12.)).text_color(rgb(c.bad)).child(p))
            })
            .when(self.old.as_ref().is_some_and(|g| g.count > 0), |d| {
                d.child(
                    div()
                        .text_size(ui(12.))
                        .text_color(rgb(c.muted))
                        .child(tr!("Renamed, its codes go along.")),
                )
            })
            .child(buttons)
    }
}
