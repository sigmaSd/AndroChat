# AndroChat
<img src="./androchat.png" width="200%" height="60%">
Communicate with https://github.com/lemunozm/termchat/


# How To Build
- Install godot, rust
- In rust directory compile to the desiered target `cargo b --target my_target` (see android instructions for example https://godot-rust.github.io/book/exporting/android.html)
- Copy or symlink the compiled lib to tcp folder
- Specify the built library path for `control.tres`
- Run
