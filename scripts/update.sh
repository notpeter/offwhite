#!/usr/bin/env sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
templates_dir="$repo_root/templates"

curl -L \
  https://raw.githubusercontent.com/gcc-mirror/gcc/master/.editorconfig \
  -o "$templates_dir/gnu.editorconfig"

curl -L \
  https://raw.githubusercontent.com/rust-lang/rust/main/.editorconfig \
  -o "$templates_dir/rust.editorconfig"

curl -L \
  https://raw.githubusercontent.com/luarocks/luarocks/main/.editorconfig \
  -o "$templates_dir/lua.editorconfig"

curl -L \
  https://raw.githubusercontent.com/openjdk/jdk/master/.editorconfig \
  -o "$templates_dir/java.editorconfig"

cat > "$templates_dir/typescript.editorconfig" <<'EOF'
root = true

[*]
charset = utf-8

[*.{ts,js}]
end_of_line = lf
trim_trailing_whitespace = true
insert_final_newline = true
indent_style = space
indent_size = 4
EOF
