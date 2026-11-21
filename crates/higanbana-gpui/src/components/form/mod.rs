//! Componentes do grupo `form`: Input, Checkbox, Switch e Slider (shadcn/ui new-york).
//!
//! O estado mora na view; o componente recebe o valor atual e devolve mudanças por
//! callbacks (`on_change`). Todos viram `Element<V>` via `Component::render(cx)`.
mod checkbox;
mod input;
mod input_keys;
mod password;
mod radio;
mod slider;
mod slider_value;
mod style;
mod switch;

#[cfg(test)]
pub(crate) mod test_util;
#[cfg(test)]
mod tests_input;
#[cfg(test)]
mod tests_slider;
#[cfg(test)]
mod tests_toggle;
#[cfg(test)]
mod tests_radio;

pub use checkbox::{CheckState, Checkbox};
pub use input::Input;
pub use radio::RadioGroup;
pub use slider::Slider;
pub use switch::Switch;
