((prog-mode . ((eval . (dprint-fmt-on-save-mode))))
 (rust-mode . ((rust-format-on-save . nil) (fill-column . 100)))
 (dart-mode
  .
  ((eval . (dart-format-on-save-mode))
   (fill-column . 100)
   (dprint-fmt-on-save-mode . nil)))
 (nil
  .
  ((compile-command
    . "cargo clippy --workspace --all-targets --all-features"))))
