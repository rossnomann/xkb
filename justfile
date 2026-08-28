default: compile_all
compile target:
    xkbcli compile-keymap --include ./xkb --include-defaults --test --layout {{target}} && echo "ok" || echo "err"
compile_all: (compile "custom_lat") (compile "custom_cyr")
test target:
    xkbcli compile-keymap --include ./xkb --include-defaults --layout {{target}} | xkbcli interactive
preview:
    ./preview/generate.py
