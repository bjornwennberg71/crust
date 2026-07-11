;;; crust-mode.el --- Major mode for the crust language

(define-derived-mode crust-mode c++-mode "Crust"
  "Major mode for editing crust source files (.cru, .crust).
Derived from c++-mode for Allman-style indentation and // comments."

  (font-lock-add-keywords
   nil
   `(;; keywords
     (,(regexp-opt
        '("function" "impl" "trait" "let" "auto" "in"
          "match" "default" "this" "public" "mutable"
          "use" "type" "unsafe" "extern"
          "async" "await")
        'words)
      . font-lock-keyword-face)
     ;; built-in generic types
     (,(regexp-opt
        '("string" "Vec" "Option" "Result"
          "HashMap" "HashSet" "Box" "Rc" "Arc"
          "box" "rc" "arc" "thread")
        'words)
      . font-lock-type-face)
     ;; built-in functions
     (,(regexp-opt
        '("spawn" "sleep_ms" "vec" "println" "print" "format")
        'words)
      . font-lock-builtin-face)
     ;; constants and special values
     (,(regexp-opt
        '("None" "Some" "Ok" "Err" "true" "false")
        'words)
      . font-lock-constant-face)
     ;; @derive(...)
     ("@[[:alpha:]][[:alnum:]_]*" . font-lock-preprocessor-face))))

(add-to-list 'auto-mode-alist '("\\.cru\\'"   . crust-mode))
(add-to-list 'auto-mode-alist '("\\.crust\\'" . crust-mode))

(provide 'crust-mode)
;;; crust-mode.el ends here
