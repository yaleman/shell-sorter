# Shell Sorter development tasks


# list the available tasks
default:
    just --list

# Run linting checks
lint:
    cargo clippy --all-targets
# uv run ruff check shell_sorter/ tests

# Run type checking
mypy:
    uv run mypy --strict shell_sorter/

# Run tests
test:
    # uv run pytest -s -v
    cargo test

# Run all checks
check: fmt lint test esphome-check

# Format code
fmt:
    uv run ruff format --check shell_sorter/ tests
    cargo fmt

# Run the Rust application
run:
    killall shell-sorter || true
    cargo run -- serve

# Run the Python application (legacy)
python:
    killall shell-sorter || true
    uv run shell-sorter

rust:
    cargo run

# Flash ESPHome configuration to device
esphome-flash:
    esphome upload esphome-shell-sorter.yaml

# flash and monitor the device
esphome-monitor:
    esphome run esphome-shell-sorter.yaml

# Build ESPHome configuration for shell sorter
esphome-cam-build:
    esphome compile esphome-esp32cam1.yaml

# Flash ESP32 Camera configuration to device
esphome-cam-flash: esphome-cam-build
    esphome upload esphome-esp32cam1.yaml

# flash and monitor the ESP32 camera device
esphome-cam-monitor:
    esphome run esphome-esp32cam1.yaml

esphome-check:
    yamllint esphome*.yaml
    esphome config esphome-shell-sorter.yaml > /dev/null
    esphome config esphome-esp32cam1.yaml > /dev/null

favicon_file := "2025-07-13-favicon-1-purple.png"
static_dir := "./shell_sorter/static"
favicon:
    magick assets/{{favicon_file}} -geometry 32x32 -background transparent -gravity center {{static_dir}}/favicon-32.png
    magick assets/{{favicon_file}} -geometry 32x32 -gravity center {{static_dir}}/favicon-32.jpg
    magick assets/{{favicon_file}} -geometry 32x32 -gravity center {{static_dir}}/favicon-32.ico
    magick assets/{{favicon_file}} -geometry 512x -background transparent -gravity center -quality 9 {{static_dir}}/favicon.png
    magick assets/{{favicon_file}} -geometry 180x -background transparent -gravity center -quality 9 {{static_dir}}/apple-touch-icon.png
    find shell_sorter/static/ -name '*.png' -exec pngcrush -ow '{}'  \;