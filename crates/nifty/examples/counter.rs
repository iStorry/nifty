use nifty::{Align, Justify, Signal, Window, button, column, label, row};

fn main() {
    let count = Signal::new(0);

    let counter_label = label({
        let count = count.clone();
        move || format!("Clicked {} times", count.get())
    });
    let decrement = button("−", {
        let count = count.clone();
        move || count.update(|n| *n -= 1)
    });
    let increment = button("+", move || count.update(|n| *n += 1));

    let content = column()
        .spacing(12.0)
        .padding(24.0)
        .align(Align::Center)
        .justify(Justify::Center)
        .child(counter_label)
        .child(row().spacing(8.0).child(decrement).child(increment));

    nifty::run(Window::new("Counter", content).size(320.0, 200.0));
}
