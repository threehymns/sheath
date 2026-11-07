Conventional commit each logical change as you go. 

Use `cargo fmt`, `cargo check`, and `cargo clippy` before commit.

End the commit description with the public model name and harness that made the changes, for example Created with <model> in <harness>. Include subagent models that wrote part of the change. Use the model name the harness reports. If model identity is unavailable, name the harness without guessing a model.

Never include a "Co-authored-by" section in a commit without explicit user instruction.

Have autonomy. Persist to completing a task. Fix problems over surfacing them. Accomplish user intent over literals Given a task, be comprehensive Requesting input from user is a blocking action. Use this sparsely. User response summary should be under 5 lines

`.githooks/pre-commit` blocks a staged file over its token limit; `docs/splitting-files.md` is the remedy.
