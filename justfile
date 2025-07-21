# Shell Sorter development tasks


# list the available tasks
default:
    just --list

# Run linting checks
lint:
    cargo clippy --quiet --all-targets
# uv run ruff check shell_sorter/ tests


# Run tests
test:
    cargo test

# Run all checks
check: fmt lint test esphome-check

# Format code
fmt:
    cargo fmt

# Run the Rust application
run:
    killall shell-sorter || true
    cargo run --quiet -- serve

run_debug:
    killall shell-sorter || true
    cargo run --quiet -- serve --debug

# Flash ESPHome configuration to device
esphome-flash:
    uvx esphome upload esphome-shell-sorter.yaml

# flash and monitor the device
esphome-monitor:
    esphome run esphome-shell-sorter.yaml

# Build ESPHome configuration for shell sorter
esphome-cam-build:
    uvx esphome compile esphome-esp32cam1.yaml

# Flash ESP32 Camera configuration to device
esphome-cam-flash: esphome-cam-build
    uvx esphome upload esphome-esp32cam1.yaml

# flash and monitor the ESP32 camera device
esphome-cam-monitor:
    uvx esphome run esphome-esp32cam1.yaml

esphome-check:
    yamllint esphome*.yaml
    uvx esphome config esphome-shell-sorter.yaml > /dev/null
    uvx esphome config esphome-esp32cam1.yaml > /dev/null

favicon_file := "2025-07-13-favicon-1-purple.png"
static_dir := "./shell_sorter/static"
favicon:
    magick assets/{{favicon_file}} -geometry 32x32 -background transparent -gravity center {{static_dir}}/favicon-32.png
    magick assets/{{favicon_file}} -geometry 32x32 -gravity center {{static_dir}}/favicon-32.jpg
    magick assets/{{favicon_file}} -geometry 32x32 -gravity center {{static_dir}}/favicon-32.ico
    magick assets/{{favicon_file}} -geometry 512x -background transparent -gravity center -quality 9 {{static_dir}}/favicon.png
    magick assets/{{favicon_file}} -geometry 180x -background transparent -gravity center -quality 9 {{static_dir}}/apple-touch-icon.png
    find shell_sorter/static/ -name '*.png' -exec pngcrush -ow '{}'  \;