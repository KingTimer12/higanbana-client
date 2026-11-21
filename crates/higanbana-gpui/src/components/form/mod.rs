//! Componentes do grupo `form`: Input, Checkbox, Switch e Slider (shadcn/ui new-york).
//!
//! O estado mora na view; o componente recebe o valor atual e devolve mudanças por
//! callbacks (`on_change`). Todos viram `Element<V>` via `Component::render(cx)`.
mod checkbox;
mod input;
mod input_keys;
mod password;
mod slider;
mod slider_value;
mod style;
mod switch;

#[cfg(test)]
mod test_util;
#[cfg(test)]
mod tests_input;
#[cfg(test)]
mod tests_slider;
#[cfg(test)]
mod tests_toggle;

pub use checkbox::{CheckState, Checkbox};
pub use input::Input;
pub use slider::Slider;
pub use switch::Switch;
