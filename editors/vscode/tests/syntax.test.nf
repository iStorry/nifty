// SYNTAX TEST "source.nifty" "nifty grammar"

struct Todo {
// <------ keyword.declaration.struct.nifty
//     ^^^^ entity.name.type.struct.nifty
  title: string
//^^^^^ variable.parameter.label.nifty
//       ^^^^^^ support.type.primitive.nifty
}

state todos: [Todo] = []
// <----- keyword.declaration.state.nifty
//    ^^^^^ variable.other.state.nifty
//            ^^^^ entity.name.type.nifty

fn area(w: float) -> float { return w * 2.5 }
// <-- keyword.declaration.function.nifty
// ^^^^ entity.name.function.nifty
//                ^^ keyword.operator.arrow.nifty
//                           ^^^^^^ keyword.control.return.nifty
//                                      ^^^ constant.numeric.float.nifty

view Row(todo: Todo) {
// <---- keyword.declaration.view.nifty
//   ^^^ entity.name.type.view.nifty
  row(align: .center) {
//^^^ support.function.builtin.nifty
//           ^ punctuation.accessor.nifty
//            ^^^^^^ variable.other.enummember.nifty
    checkbox(checked: $todo.done)
//                    ^ keyword.operator.binding.nifty
//                          ^^^^ variable.other.property.nifty
    label("{todos.len()} \{x\} \q", style: .title) // trailing
//         ^ punctuation.section.interpolation.begin.nifty
//                ^^^ support.function.method.nifty
//                       ^^ constant.character.escape.nifty
//                             ^^ invalid.illegal.escape.nifty
//                                                 ^^^^^^^^^^^ comment.line.double-slash.nifty
  }
}

for todo in todos { if todo.done != true { add_todo() } }
// <--- keyword.control.loop.nifty
//  ^^^^ variable.other.constant.nifty
//       ^^ keyword.control.loop.nifty
//                  ^^ keyword.control.conditional.nifty
//                               ^^ keyword.operator.comparison.nifty
//                                  ^^^^ constant.language.boolean.nifty
//                                         ^^^^^^^^ entity.name.function.call.nifty

window("All") { column(spacing: 4) { text_field($d, lines: 3) } }
// <------ support.function.builtin.nifty
//              ^^^^^^ support.function.builtin.nifty
//                                   ^^^^^^^^^^ support.function.builtin.nifty

button("All", selected: filter == "all") { filter = "all" }
// <------ support.function.builtin.nifty

nav_item("Inbox", value: "inbox", selection: $s, icon: .inbox, badge: "3")
// <-------- support.function.builtin.nifty

button("Go") { download("u", to: "~", progress: $p, status: $s) }
// <------ support.function.builtin.nifty
//             ^^^^^^^^ support.function.builtin.nifty

pick_file($p, types: ["png"]) pick_files($ps) pick_folder($f) pick_save_path($p, name: "a")
// <--------- support.function.builtin.nifty
//                            ^^^^^^^^^^ support.function.builtin.nifty
//                                            ^^^^^^^^^^^ support.function.builtin.nifty
//                                                            ^^^^^^^^^^^^^^ support.function.builtin.nifty

switch(on: $w, title: "Wi-Fi") slider($v, min: -5, max: 5) picker($s, options: ["a"])
// <------ support.function.builtin.nifty
//                             ^^^^^^ support.function.builtin.nifty
//                                                         ^^^^^^ support.function.builtin.nifty

scroll(spacing: 4) { progress(value: item.percent) progress() }
// <------ support.function.builtin.nifty
//                   ^^^^^^^^ support.function.builtin.nifty

image("~/a.png", width: 64) text_field($q) { search() }
// <----- support.function.builtin.nifty

menu("File") { menu_item("New", shortcut: "n") { add() } }
// <---- support.function.builtin.nifty
//             ^^^^^^^^^ support.function.builtin.nifty

context_menu() { menu_item("Remove") { remove(item) } }
// <------------ support.function.builtin.nifty
//               ^^^^^^^^^ support.function.builtin.nifty

window settings("Settings", open: false) { close_window(settings) }
// <------ support.function.builtin.nifty
//     ^^^^^^^^ entity.name.window.nifty
//                                         ^^^^^^^^^^^^ support.function.builtin.nifty

drop_target($dropped) { open_window(downloads) }
// <----------- support.function.builtin.nifty
//                      ^^^^^^^^^^^ support.function.builtin.nifty

progress(value: item.percent) progress()
// <-------- support.function.builtin.nifty
//                            ^^^^^^^^ support.function.builtin.nifty

spacer() divider() items.push(1) items.remove(0) items.is_empty()
// <------ support.function.builtin.nifty
//       ^^^^^^^ support.function.builtin.nifty
//                       ^^^^ support.function.method.nifty
//                                     ^^^^^^ support.function.method.nifty
//                                                     ^^^^^^^^ support.function.method.nifty

label("\n\t\"\\")
//     ^^ constant.character.escape.nifty
//       ^^ constant.character.escape.nifty
//         ^^ constant.character.escape.nifty
//           ^^ constant.character.escape.nifty
